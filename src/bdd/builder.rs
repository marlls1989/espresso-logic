//! The generic BDD builder.
//!
//! A builder owns one BDD manager (an independent node table, unique table and caches) and hands out
//! [`Bdd`] handles branded to it. [`BddBuilder`] is parameterised by two orthogonal type parameters:
//!
//! - a [`Brand`] `B`, a sealed marker that names this builder's namespace for uniqueness;
//! - a [`ManagerCell`] `C`, the storage backend — [`LocalCell`](crate::expression::manager_cell::LocalCell)
//!   for a single-threaded builder (`!Send`/`!Sync`), or
//!   [`SyncCell`](crate::expression::manager_cell::SyncCell) for a thread-safe one (`Send + Sync`).
//!
//! The brand and the cell are independent: any brand pairs with either cell, and the body of every method
//! is written once over `<B, C>`. There is no process-global manager and no default brand; each builder is
//! independent.
//!
//! The ergonomic `bdd_builder!` / `sync_bdd_builder!` macros mint a fresh brand per call paired with the
//! [`LocalCell`](crate::expression::manager_cell::LocalCell) or
//! [`SyncCell`](crate::expression::manager_cell::SyncCell) backend respectively; in-crate tests declare
//! their own brand types (the brand seal permits in-crate impls).

use std::marker::PhantomData;

use super::brand::Brand;
use super::handle::Bdd;
use super::scope::{Scope, ScopedBdd};
use crate::cover::{Anonymous, Cover, StringLabel};
use crate::error::MinimizationError;
use crate::expression::manager::{BddManager, FALSE_NODE, TRUE_NODE};
use crate::expression::manager_cell::ManagerCell;
use crate::expression::{BoolExpr, ParseBoolExprError};
use crate::Symbol;

/// An owned BDD namespace over a brand `B` and a storage backend `C`.
///
/// Owns a fresh `C`-backed manager and hands out [`Bdd`] handles branded to it. The storage backend
/// determines thread-safety: a [`LocalCell`](crate::expression::manager_cell::LocalCell)-backed builder is
/// single-threaded (`!Send`/`!Sync`) and pays no synchronisation cost, while a
/// [`SyncCell`](crate::expression::manager_cell::SyncCell)-backed one is `Send + Sync` and can be moved to,
/// or shared by reference across, threads (lock poisoning propagates).
///
/// A handle holds its own refcounted clone of the manager, so it can outlive the builder.
///
/// # Send/Sync follows the cell, not the brand
///
/// The brand selects no behaviour; thread-safety follows the storage cell alone. A builder over
/// [`LocalCell`](crate::expression::manager_cell::LocalCell) is `!Send` whatever its brand, and one over
/// [`SyncCell`](crate::expression::manager_cell::SyncCell) is `Send + Sync` whatever its brand. This is
/// asserted at compile time in this module's tests.
pub struct BddBuilder<B: Brand, C: ManagerCell> {
    cell: C,
    _brand: PhantomData<fn() -> B>,
}

impl<B: Brand, C: ManagerCell> BddBuilder<B, C> {
    /// Create a new builder owning a fresh, empty BDD manager (seeded with the two terminals).
    #[must_use]
    pub fn new() -> Self {
        BddBuilder {
            cell: C::new_empty(),
            _brand: PhantomData,
        }
    }

    /// Build a builder onto an existing manager cell (a refcount bump). The recovered builder shares
    /// the originating manager, so its handles interoperate with handles already minted from that
    /// manager. Mirrors [`Bdd::from_root`](super::handle::Bdd) and backs
    /// [`Bdd::builder`](crate::bdd::Bdd::builder).
    pub(super) fn from_cell(cell: &C) -> Self {
        BddBuilder {
            cell: cell.clone(),
            _brand: PhantomData,
        }
    }

    /// This builder's storage cell. Crate-internal: the scoped builder ([`scope`](Self::scope)) reads it
    /// to mint by-reference [`ScopedBdd`] handles.
    pub(super) fn cell(&self) -> &C {
        &self.cell
    }

    /// A handle for the single variable `name`, creating it in this builder's variable ordering on first
    /// use.
    #[must_use]
    pub fn var<S: AsRef<str>>(&self, name: S) -> Bdd<B, C> {
        let id = BddManager::make_var(&self.cell, name.as_ref());
        let root = BddManager::make_node(&self.cell, id, FALSE_NODE, TRUE_NODE);
        Bdd::from_root(&self.cell, root)
    }

    /// A handle for a constant: `true` or `false`.
    #[must_use]
    pub fn constant(&self, value: bool) -> Bdd<B, C> {
        let root = if value { TRUE_NODE } else { FALSE_NODE };
        Bdd::from_root(&self.cell, root)
    }

    /// Build the BDD of a [`Cover`]'s ON-set: the OR of each cube's product term.
    ///
    /// Each input column resolves to a variable by its label's string form (mirroring the existing
    /// expression builder), each cube becomes the AND of its fixed literals (a fully don't-care cube is
    /// the constant `true`), and the cubes are OR-ed together; an empty cover is `false`. The input label
    /// type is therefore a [`StringLabel`] (`Symbol`/`String`/… — anything with a `&str` view); positional
    /// [`Anonymous`](crate::Anonymous) inputs have no name to resolve and are not accepted.
    ///
    /// **Multi-output covers** are treated as the **ON-set characteristic function over the inputs**: a
    /// cube contributes its input product term whenever it is an ON-set cube, regardless of which output
    /// columns it asserts, so per-output structure is collapsed. For a per-output function, project the
    /// cover to one output before building. Only `F` (ON-set) cubes contribute; `D`/`R` cubes are ignored,
    /// so the result is the ON-set of the cover.
    #[must_use]
    pub fn build_cover<I: StringLabel, O>(&self, cover: &Cover<I, O>) -> Bdd<B, C> {
        use crate::cover::CubeType;
        // Composed inside a `scope`: the OR-of-products fold runs on `Copy`, by-reference handles, so the
        // doubly-nested loop pays no per-operation refcount bump — only the returned root is materialised.
        self.scope(|s| {
            let mut acc = s.constant(false);
            for cube in cover.cubes() {
                if cube.cube_type() != CubeType::F {
                    continue;
                }
                // Product term: AND of this cube's fixed literals (don't-cares skipped). A fully
                // don't-care cube is the constant `true`.
                let mut term = s.constant(true);
                let labels = cube.inputs().vars();
                for (label, value) in labels.iter().zip(cube.inputs().iter()) {
                    match value {
                        Some(true) => {
                            term = term & s.var(label.as_ref());
                        }
                        Some(false) => {
                            term = term & !s.var(label.as_ref());
                        }
                        None => {}
                    }
                }
                acc = acc | term;
            }
            acc
        })
    }

    /// Build a [`Bdd`] handle from an owned, syntactic [`BoolExpr`].
    ///
    /// Interprets the expression's reverse-Polish token stream into canonical BDD nodes through this
    /// builder's engine: a variable becomes [`var`](Self::var), a constant becomes
    /// [`constant`](Self::constant), and the operators fold through the handle's `&`/`|`/`^`/`!`. Evaluated
    /// iteratively with an explicit value stack (no recursion), so an arbitrarily deep expression cannot
    /// overflow the call stack. The result is canonical, so two expressions denoting the same function
    /// build to the *same* handle.
    ///
    /// Composed inside a [`scope`](Self::scope) so the fold runs on `Copy`, by-reference handles (one
    /// refcount bump for the returned root, not one per node); [`Scope::build`] does the postfix fold.
    #[must_use]
    pub fn build(&self, expr: &BoolExpr) -> Bdd<B, C> {
        self.scope(|s| s.build(expr))
    }

    /// Compose a [`Bdd`] through a [`Scope`] of `Copy`, by-reference handles, returning the owned root.
    ///
    /// The closure receives a [`Scope`] over this builder and returns the [`ScopedBdd`] for the result.
    /// A [`ScopedBdd`] is a [`Copy`] handle — a node id plus a borrow of this builder's manager — so the
    /// operators (`&`, `|`, `^`, `!`) compose handles in place with no `.clone()` at the call site, and an
    /// operand can be named more than once without cloning. The handle is confined to the closure by an
    /// invariant lifetime brand (it cannot escape or be mixed with another scope); only the owned
    /// [`Bdd`] for the returned root leaves, carrying this builder's brand and manager so it interoperates
    /// with every other handle the builder mints.
    ///
    /// This complements [`build`](Self::build): both produce a canonical handle in this builder, but
    /// `scope` trades the owned `Bdd`'s refcount-bumped composition for cheaper, allocation-free
    /// composition inside the closure. An existing owned [`Bdd`] is spliced in with [`Scope::lift`].
    ///
    /// ```
    /// use espresso_logic::bdd_builder;
    ///
    /// let builder = bdd_builder!();
    /// // (a ^ b) & !c, composed from Copy handles — no `.clone()`.
    /// let f = builder.scope(|s| (s.var("a") ^ s.var("b")) & !s.var("c"));
    /// assert!(f.equivalent_to(&builder.parse("(a ^ b) & !c").unwrap()));
    /// ```
    #[must_use]
    pub fn scope<F>(&self, f: F) -> Bdd<B, C>
    where
        F: for<'s> FnOnce(Scope<'s, B, C>) -> ScopedBdd<'s, B, C>,
    {
        let root = f(Scope::new(self)).root();
        Bdd::from_root(&self.cell, root)
    }

    /// Parse a Boolean expression from a string and build it into a [`Bdd`] in this builder.
    ///
    /// A convenience for `self.build(&BoolExpr::parse(input)?)`.
    ///
    /// # Errors
    ///
    /// Propagates a [`ParseBoolExprError`] if the text does not parse.
    pub fn parse<S: AsRef<str>>(&self, input: S) -> Result<Bdd<B, C>, ParseBoolExprError> {
        Ok(self.build(&BoolExpr::parse(input)?))
    }

    /// Minimise a [`BoolExpr`]'s ON-set with Espresso, returning the minimised single-output [`Cover`].
    ///
    /// A convenience for `self.build(expr).minimize()`; the cover is the characteristic function over the
    /// expression's support variables (see [`Bdd::cover`]).
    ///
    /// # Errors
    ///
    /// Propagates any [`MinimizationError`] from the Espresso engine.
    pub fn minimize(&self, expr: &BoolExpr) -> Result<Cover<Symbol, Anonymous>, MinimizationError> {
        self.build(expr).minimize()
    }
}

impl<B: Brand, C: ManagerCell> Default for BddBuilder<B, C> {
    fn default() -> Self {
        Self::new()
    }
}
