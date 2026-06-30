//! The scoped, by-reference BDD builder behind [`BddBuilder::scope`].
//!
//! An owned [`Bdd`] holds a refcounted clone of its manager's cell, so each composition bumps a
//! reference count and the handle is not [`Copy`]. Inside a [`BddBuilder::scope`] closure that cost is
//! removed: a [`ScopedBdd`] is a node id plus a *borrow* of the builder's cell, so it is [`Copy`] and the
//! operators (`&`, `|`, `^`, `!`) compose handles in place — no `.clone()` at the call site, and an
//! operand can be named more than once for free.
//!
//! [`BddBuilder::scope`] hands a closure a [`Scope`] over the builder and expects back the [`ScopedBdd`]
//! for the root; it then materialises the owned [`Bdd`] for that node. The scoped handle is branded with
//! the closure's lifetime, so it cannot escape the closure or be mixed with another scope — both are
//! compile errors — while the owned [`Bdd`] that leaves carries the builder's brand and manager and so
//! interoperates with every other handle the builder mints.
//!
//! Unlike the [`ExprBuilder`](crate::expression::ExprBuilder)'s `graft` (which copies an existing
//! expression's tokens verbatim), [`Scope::lift`] splices an existing owned [`Bdd`] in at no cost: the
//! node is already canonical in this manager, so lifting it is a re-view as a borrowed handle.

use std::marker::PhantomData;
use std::ops::{BitAnd, BitOr, BitXor, Not};

use super::brand::Brand;
use super::builder::BddBuilder;
use super::handle::Bdd;
use crate::expression::manager::{BddManager, NodeId, FALSE_NODE, TRUE_NODE};
use crate::expression::manager_cell::ManagerCell;
use crate::expression::rpn;
use crate::expression::{BoolExpr, ParseBoolExprError};

/// A [`Copy`], by-reference handle into a [`BddBuilder::scope`] closure.
///
/// A node id plus a borrow of the builder's manager cell, carrying the builder's brand `B`. Because it
/// borrows rather than refcount-clones the cell, it is [`Copy`]: operands compose without `.clone()` and
/// may be named repeatedly. The lifetime `'s` brands the handle to one scope — it is invariant, so a
/// handle cannot outlive the closure or be combined with a handle from another scope.
///
/// A handle cannot escape its closure:
///
/// ```compile_fail
/// use espresso_logic::bdd_builder;
///
/// let builder = bdd_builder!();
/// let mut stash = None;
/// builder.scope(|s| {
///     let a = s.var("a");
///     stash = Some(a); // error: `a` does not live long enough — the brand confines it
///     a
/// });
/// ```
pub struct ScopedBdd<'s, B: Brand, C: ManagerCell> {
    root: NodeId,
    cell: &'s C,
    /// Invariant in `'s` (it brands the scope) and carries the builder's brand `B`.
    _brand: ScopeBrand<'s, B>,
}

/// Type-level brand for a [`ScopedBdd`]: the `fn(&'s ()) -> &'s ()` half makes the scope lifetime `'s`
/// invariant (so a handle can neither escape its scope nor unify with another scope), and the `fn() -> B`
/// half carries the builder's brand `B` (mirroring the owned [`Bdd`]'s `PhantomData<fn() -> B>`).
type ScopeBrand<'s, B> = PhantomData<(fn(&'s ()) -> &'s (), fn() -> B)>;

impl<B: Brand, C: ManagerCell> Clone for ScopedBdd<'_, B, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: Brand, C: ManagerCell> Copy for ScopedBdd<'_, B, C> {}

impl<'s, B: Brand, C: ManagerCell> ScopedBdd<'s, B, C> {
    /// Wrap a raw root node into a scoped handle borrowing `cell`.
    fn from_root(cell: &'s C, root: NodeId) -> Self {
        ScopedBdd {
            root,
            cell,
            _brand: PhantomData,
        }
    }

    /// The canonical root node this handle denotes. Crate-internal: [`BddBuilder::scope`] reads it to
    /// materialise the owned [`Bdd`].
    pub(super) fn root(self) -> NodeId {
        self.root
    }

    /// Assert two handles share one manager. The shared `'s` already ties both to one scope; this guards
    /// against a future change that lets two scopes' lifetimes coincide (mirrors the owned handle's
    /// runtime backstop against a brand clash).
    fn assert_same(self, other: Self) {
        debug_assert!(
            self.cell.as_ptr() == other.cell.as_ptr(),
            "ScopedBdd handles from different builders"
        );
    }

    /// Logical AND: `self ∧ other`. Equivalent to the `&` operator.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        self.assert_same(other);
        Self::from_root(
            self.cell,
            BddManager::ite(self.cell, self.root, other.root, FALSE_NODE),
        )
    }

    /// Logical OR: `self ∨ other`. Equivalent to the `|` operator.
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        self.assert_same(other);
        Self::from_root(
            self.cell,
            BddManager::ite(self.cell, self.root, TRUE_NODE, other.root),
        )
    }

    /// Logical XOR: `self ⊕ other`. Equivalent to the `^` operator.
    #[must_use]
    pub fn xor(self, other: Self) -> Self {
        self.assert_same(other);
        Self::from_root(self.cell, BddManager::xor(self.cell, self.root, other.root))
    }

    /// Logical NOT: `¬self`. Equivalent to the unary `!` operator.
    #[must_use]
    pub fn complement(self) -> Self {
        Self::from_root(
            self.cell,
            BddManager::ite(self.cell, self.root, FALSE_NODE, TRUE_NODE),
        )
    }
}

impl<B: Brand, C: ManagerCell> BitAnd for ScopedBdd<'_, B, C> {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        self.and(rhs)
    }
}

impl<B: Brand, C: ManagerCell> BitOr for ScopedBdd<'_, B, C> {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.or(rhs)
    }
}

impl<B: Brand, C: ManagerCell> BitXor for ScopedBdd<'_, B, C> {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        self.xor(rhs)
    }
}

impl<B: Brand, C: ManagerCell> Not for ScopedBdd<'_, B, C> {
    type Output = Self;
    fn not(self) -> Self {
        self.complement()
    }
}

/// The handle source a [`BddBuilder::scope`] closure composes with: a borrow of the builder that mints
/// [`Copy`], by-reference [`ScopedBdd`] handles into it.
///
/// Created only by [`BddBuilder::scope`], which hands the closure one of these. Its leaves
/// ([`var`](Self::var), [`constant`](Self::constant)), expression builders ([`build`](Self::build),
/// [`parse`](Self::parse)), and the splice [`lift`](Self::lift) all return [`ScopedBdd`] handles branded
/// to this scope.
pub struct Scope<'s, B: Brand, C: ManagerCell> {
    builder: &'s BddBuilder<B, C>,
}

impl<'s, B: Brand, C: ManagerCell> Scope<'s, B, C> {
    /// Open a scope over `builder`. Crate-internal: only [`BddBuilder::scope`] mints one.
    pub(super) fn new(builder: &'s BddBuilder<B, C>) -> Self {
        Scope { builder }
    }

    /// A handle for the single variable `name`, creating it in the builder's variable ordering on first
    /// use.
    #[must_use]
    pub fn var<S: AsRef<str>>(&self, name: S) -> ScopedBdd<'s, B, C> {
        let cell = self.builder.cell();
        let id = BddManager::make_var(cell, name.as_ref());
        let root = BddManager::make_node(cell, id, FALSE_NODE, TRUE_NODE);
        ScopedBdd::from_root(cell, root)
    }

    /// A handle for a constant: `true` or `false`.
    #[must_use]
    pub fn constant(&self, value: bool) -> ScopedBdd<'s, B, C> {
        let root = if value { TRUE_NODE } else { FALSE_NODE };
        ScopedBdd::from_root(self.builder.cell(), root)
    }

    /// Splice an existing owned [`Bdd`] into this scope as a [`Copy`] handle.
    ///
    /// The node is already canonical in this builder's manager, so lifting it is a re-view at no cost — not
    /// a copy. (Contrast the [`ExprBuilder`](crate::expression::ExprBuilder)'s `graft`, which copies an
    /// expression's tokens.)
    ///
    /// A [`Bdd`] from a *different* builder carries a different [`Brand`](crate::bdd::Brand), so it cannot
    /// be lifted — a compile error, the same guarantee the operators give:
    ///
    /// ```compile_fail
    /// use espresso_logic::bdd_builder;
    ///
    /// let one = bdd_builder!();
    /// let two = bdd_builder!();
    /// let foreign = two.var("a");
    /// // error: `foreign`'s brand differs from `one`'s, so it is not a `Bdd` of this scope.
    /// let _ = one.scope(|s| s.lift(&foreign));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `bdd` belongs to a different manager (only possible under a brand clash — two builders
    /// sharing a brand type), mirroring the owned operators' same-manager backstop.
    #[must_use]
    pub fn lift(&self, bdd: &Bdd<B, C>) -> ScopedBdd<'s, B, C> {
        let cell = self.builder.cell();
        assert!(
            bdd.cell().as_ptr() == cell.as_ptr(),
            "Bdd lifted from a different manager (a brand clash); mixing them is a bug"
        );
        ScopedBdd::from_root(cell, bdd.root())
    }

    /// Build a [`ScopedBdd`] from an owned, syntactic [`BoolExpr`] (the scoped analogue of
    /// [`BddBuilder::build`]).
    ///
    /// Interprets the expression's reverse-Polish token stream into canonical nodes through this scope's
    /// handles, iteratively (no recursion), so an arbitrarily deep expression cannot overflow the stack.
    #[must_use]
    pub fn build(&self, expr: &BoolExpr) -> ScopedBdd<'s, B, C> {
        rpn::fold_postfix(
            expr.tokens(),
            |name| self.var(name.as_str()),
            |value| self.constant(value),
            |a| !a,
            |l, r| l & r,
            |l, r| l | r,
            |l, r| l ^ r,
        )
    }

    /// Parse a Boolean expression from a string and build it into a [`ScopedBdd`] (the scoped analogue of
    /// [`BddBuilder::parse`]).
    ///
    /// # Errors
    ///
    /// Propagates a [`ParseBoolExprError`] if the text does not parse.
    pub fn parse<S: AsRef<str>>(
        &self,
        input: S,
    ) -> Result<ScopedBdd<'s, B, C>, ParseBoolExprError> {
        Ok(self.build(&BoolExpr::parse(input)?))
    }
}
