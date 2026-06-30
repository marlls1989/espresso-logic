//! The two concrete BDD contexts.
//!
//! A context owns one BDD manager (an independent node table, unique table and caches) and hands out
//! [`Bdd`] handles branded to it. Two flavours, distinguished purely by the cell their brand selects:
//!
//! - [`BddBuilder`] owns a [`LocalCell`](crate::expression::manager_cell::LocalCell), so it is
//!   single-threaded (`!Send`/`!Sync`) and pays no synchronisation cost.
//! - [`SyncBddBuilder`] owns a [`SyncCell`](crate::expression::manager_cell::SyncCell), so it is
//!   `Send + Sync` and can be shared across threads.
//!
//! Both are generic over a [`Brand`]; the brand's [`Cell`](Brand::Cell) associated type selects the
//! concrete cell, and the body of every method is shared. There is no process-global manager and no
//! default brand: each context is independent.
//!
//! The ergonomic `bdd_builder!` / `sync_bdd_builder!` macros that mint a fresh brand per call arrive
//! with the 5.0 breaking cut; until then, in-crate tests declare their own brand types (the brand seal
//! permits in-crate impls) to construct contexts.

use std::marker::PhantomData;

use super::brand::Brand;
use super::handle::Bdd;
use crate::cover::{Anonymous, Cover, StringLabel};
use crate::error::MinimizationError;
use crate::expression::manager::{BddManager, FALSE_NODE, TRUE_NODE};
use crate::expression::manager_cell::ManagerCell;
use crate::expression::rpn::Token;
use crate::expression::{BoolExpr, ParseBoolExprError};
use crate::Symbol;

/// A single-threaded, owned BDD namespace.
///
/// Owns a fresh [`LocalCell`](crate::expression::manager_cell::LocalCell)-backed manager (when its
/// brand selects that cell) and hands out [`Bdd`] handles branded to it. `!Send`/`!Sync`: use
/// [`SyncBddBuilder`] to share a context across threads.
///
/// The context must outlive every handle it produces (handles borrow it); this is enforced at compile
/// time by the borrow checker.
///
/// # Not thread-safe
///
/// A `BddBuilder` whose brand selects the single-threaded
/// [`LocalCell`](crate::expression::manager_cell::LocalCell) is not `Send`/`Sync`, so it cannot be moved
/// into or shared across threads — use [`SyncBddBuilder`] for that. The asymmetry (a `LocalCell`-branded
/// `BddBuilder` is `!Send` while a `SyncCell`-branded `SyncBddBuilder` is `Send + Sync`) is asserted at
/// compile time in this module's tests.
pub struct BddBuilder<B: Brand> {
    cell: B::Cell,
    _brand: PhantomData<fn() -> B>,
}

/// A thread-safe, owned BDD namespace.
///
/// Owns a fresh [`SyncCell`](crate::expression::manager_cell::SyncCell)-backed manager (when its brand
/// selects that cell) and hands out [`Bdd`] handles branded to it. `Send + Sync`: the context can be
/// moved to, or shared by reference across, threads. Lock poisoning propagates — a panic while the
/// manager is borrowed poisons it for every subsequent access.
///
/// The context must outlive every handle it produces (handles borrow it); this is enforced at compile
/// time by the borrow checker.
pub struct SyncBddBuilder<B: Brand> {
    cell: B::Cell,
    _brand: PhantomData<fn() -> B>,
}

macro_rules! context_impl {
    ($ctx:ident) => {
        impl<B: Brand> $ctx<B> {
            /// Create a new context owning a fresh, empty BDD manager (seeded with the two terminals).
            #[must_use]
            pub fn new() -> Self {
                $ctx {
                    cell: <B::Cell as ManagerCell>::new_empty(),
                    _brand: PhantomData,
                }
            }

            /// A handle for the single variable `name`, creating it in this context's variable ordering
            /// on first use.
            #[must_use]
            pub fn var<S: AsRef<str>>(&self, name: S) -> Bdd<'_, B> {
                let id = BddManager::make_var(&self.cell, name.as_ref());
                let root = BddManager::make_node(&self.cell, id, FALSE_NODE, TRUE_NODE);
                Bdd::from_root(&self.cell, root)
            }

            /// A handle for a constant: `true` or `false`.
            #[must_use]
            pub fn constant(&self, value: bool) -> Bdd<'_, B> {
                let root = if value { TRUE_NODE } else { FALSE_NODE };
                Bdd::from_root(&self.cell, root)
            }

            /// Build the BDD of a [`Cover`]'s ON-set: the OR of each cube's product term.
            ///
            /// Each input column resolves to a variable by its label's string form (mirroring the
            /// existing expression builder), each cube becomes the AND of its fixed literals (a fully
            /// don't-care cube is the constant `true`), and the cubes are OR-ed together; an empty cover
            /// is `false`. The input label type is therefore a [`StringLabel`] (`Symbol`/`String`/… —
            /// anything with a `&str` view); positional [`Anonymous`](crate::Anonymous) inputs have no
            /// name to resolve and are not accepted.
            ///
            /// **Multi-output covers** are treated as the **ON-set characteristic function over the
            /// inputs**: a cube contributes its input product term whenever it is an ON-set cube,
            /// regardless of which output columns it asserts, so per-output structure is collapsed. For a
            /// per-output function, project the cover to one output before building. Only `F` (ON-set)
            /// cubes contribute; `D`/`R` cubes are ignored, so the result is the ON-set of the cover.
            #[must_use]
            pub fn build_cover<I: StringLabel, O>(&self, cover: &Cover<I, O>) -> Bdd<'_, B> {
                use crate::cover::CubeType;
                let mut acc = self.constant(false);
                for cube in cover.cubes() {
                    if cube.cube_type() != CubeType::F {
                        continue;
                    }
                    // Product term: AND of this cube's fixed literals (don't-cares skipped). A fully
                    // don't-care cube is the constant `true`.
                    let mut term = self.constant(true);
                    let labels = cube.inputs().vars();
                    for (label, value) in labels.iter().zip(cube.inputs().iter()) {
                        match value {
                            Some(true) => {
                                term = term & self.var(label.as_ref());
                            }
                            Some(false) => {
                                term = term & !self.var(label.as_ref());
                            }
                            None => {}
                        }
                    }
                    acc = acc | term;
                }
                acc
            }

            /// Build a [`Bdd`] handle from an owned, syntactic [`BoolExpr`].
            ///
            /// Interprets the expression's reverse-Polish token stream into canonical BDD nodes through
            /// this context's engine: a variable becomes [`var`](Self::var), a constant becomes
            /// [`constant`](Self::constant), and the operators fold through the handle's `&`/`|`/`^`/`!`.
            /// Evaluated iteratively with an explicit value stack (no recursion), so an arbitrarily deep
            /// expression cannot overflow the call stack. The result is canonical, so two expressions
            /// denoting the same function build to the *same* handle.
            #[must_use]
            pub fn build(&self, expr: &BoolExpr) -> Bdd<'_, B> {
                let mut stack: Vec<Bdd<'_, B>> = Vec::with_capacity(expr.tokens().len());
                for token in expr.tokens() {
                    let node = match token {
                        Token::Var(name) => self.var(name.as_str()),
                        Token::Const(value) => self.constant(*value),
                        Token::Not => {
                            let a = stack.pop().expect("build: postfix underflow on NOT");
                            a.complement()
                        }
                        Token::And => {
                            let r = stack.pop().expect("build: postfix underflow on AND");
                            let l = stack.pop().expect("build: postfix underflow on AND");
                            l.and(r)
                        }
                        Token::Or => {
                            let r = stack.pop().expect("build: postfix underflow on OR");
                            let l = stack.pop().expect("build: postfix underflow on OR");
                            l.or(r)
                        }
                        Token::Xor => {
                            let r = stack.pop().expect("build: postfix underflow on XOR");
                            let l = stack.pop().expect("build: postfix underflow on XOR");
                            l.xor(r)
                        }
                    };
                    stack.push(node);
                }
                stack.pop().expect("build: empty expression token stream")
            }

            /// Parse a Boolean expression from a string and build it into a [`Bdd`] in this context.
            ///
            /// A convenience for `self.build(&BoolExpr::parse(input)?)`.
            ///
            /// # Errors
            ///
            /// Propagates a [`ParseBoolExprError`] if the text does not parse.
            pub fn parse<S: AsRef<str>>(
                &self,
                input: S,
            ) -> Result<Bdd<'_, B>, ParseBoolExprError> {
                Ok(self.build(&BoolExpr::parse(input)?))
            }

            /// Minimise a [`BoolExpr`]'s ON-set with Espresso, returning the minimised single-output
            /// [`Cover`].
            ///
            /// A convenience for `self.build(expr).minimize()`; the cover is the characteristic function
            /// over the expression's support variables (see [`Bdd::to_cubes`]).
            ///
            /// # Errors
            ///
            /// Propagates any [`MinimizationError`] from the Espresso engine.
            pub fn minimize(
                &self,
                expr: &BoolExpr,
            ) -> Result<Cover<Symbol, Anonymous>, MinimizationError> {
                self.build(expr).minimize()
            }
        }

        impl<B: Brand> Default for $ctx<B> {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

context_impl!(BddBuilder);
context_impl!(SyncBddBuilder);
