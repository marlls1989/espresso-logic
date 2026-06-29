//! Low-level BDD construction: the [`BoolExpr::build`] closure builder.
//!
//! [`BoolExpr::build`] calls a closure with a [`BddBuilder`] and returns the [`BoolExpr`] for the handle
//! the closure returns. The builder's methods build [`Bdd`] node handles in the manager.
//! [`graft`](BddBuilder::graft) turns an existing [`BoolExpr`] into a handle. A scoped
//! [`BddContext`](crate::BddContext) offers the same surface via
//! [`BddContext::build`](crate::BddContext::build).
//!
//! ```
//! use espresso_logic::BoolExpr;
//!
//! // (a ^ b) & !c, composed from node handles.
//! let expr = BoolExpr::build(|b| {
//!     let a = b.var("a");
//!     let bb = b.var("b");
//!     let c = b.var("c");
//!     b.and(b.xor(a, bb), b.not(c))
//! });
//!
//! let manual = BoolExpr::variable("a")
//!     .xor(&BoolExpr::variable("b"))
//!     .and(&BoolExpr::variable("c").not());
//! assert_eq!(expr, manual);
//! ```
//!
//! # Splicing in existing expressions
//!
//! [`graft`](BddBuilder::graft) lifts an existing [`BoolExpr`] into the build as a handle (its root node):
//!
//! ```
//! use espresso_logic::BoolExpr;
//!
//! let sub = BoolExpr::parse("a * b").unwrap();
//! let expr = BoolExpr::build(|b| {
//!     let other = BoolExpr::parse("c + d").unwrap();
//!     b.or(b.graft(&sub), b.graft(&other))
//! });
//! # let _ = expr;
//! ```
//!
//! # Handles cannot escape their builder
//!
//! A [`Bdd`] handle is branded with the builder's invariant lifetime, so it cannot be stored outside the
//! closure or smuggled between two `build` calls — misuse is a compile error.
//!
//! ```compile_fail
//! use espresso_logic::BoolExpr;
//!
//! let mut escaped = None;
//! BoolExpr::build(|b| {
//!     let a = b.var("a");
//!     escaped = Some(a); // error: the handle's lifetime cannot outlive the closure
//!     a
//! });
//! ```

use super::context::{Brand, Global};
use super::manager::{BddManager, NodeId, Store, FALSE_NODE, TRUE_NODE};
use super::manager_cell::{ManagerCell, SyncCell};
use super::BoolExpr;
use std::marker::PhantomData;

/// Marker that is **invariant** in both the builder lifetime `'b` and the brand `B`, so neither can be
/// widened or narrowed. Carried by [`Bdd`] and [`BddBuilder`] to brand handles to one builder activation
/// and one BDD namespace.
type BuilderBrand<'b, B> = PhantomData<(fn(&'b ()) -> &'b (), fn() -> B)>;

/// A handle to a node being built inside a [`BoolExpr::build`] / [`BddContext::build`] closure.
///
/// `Copy` (it is a node id). The `'b` lifetime is an **invariant brand** tying the handle to one
/// builder activation, so the borrow checker rejects any attempt to move a handle out of its closure
/// or use it with a different builder; the `B` type parameter additionally brands it to its BDD
/// namespace. Handles are opaque: combine them only through the [`BddBuilder`] methods.
///
/// [`BddContext::build`]: crate::BddContext::build
pub struct Bdd<'b, B: Brand = Global> {
    node: NodeId,
    // Invariant in 'b (neither co- nor contravariant) and in B, so neither brand can be widened.
    _marker: BuilderBrand<'b, B>,
}

impl<B: Brand> Clone for Bdd<'_, B> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<B: Brand> Copy for Bdd<'_, B> {}

/// The builder handed to a [`BoolExpr::build`] / [`BddContext::build`] closure: the BDD manager's
/// node-construction operations.
///
/// Methods take `&self`, so handles nest in one expression — `b.and(b.var("a"), b.not(b.var("b")))`.
/// Each method hash-conses through the manager and returns a canonical [`Bdd`] handle.
///
/// [`BddContext::build`]: crate::BddContext::build
pub struct BddBuilder<'b, B: Brand = Global> {
    cell: SyncCell,
    /// Invariant brand (see [`Bdd`]); the builder owns the cell, so `'b` is carried only as a marker.
    _marker: BuilderBrand<'b, B>,
}

impl<'b, B: Brand> BddBuilder<'b, B> {
    #[inline]
    fn wrap(node: NodeId) -> Bdd<'b, B> {
        Bdd {
            node,
            _marker: PhantomData,
        }
    }

    /// A variable by name (creating it in the manager's ordering on first use).
    pub fn var<S: AsRef<str>>(&self, name: S) -> Bdd<'b, B> {
        let var_id = BddManager::make_var(&self.cell, name.as_ref());
        Self::wrap(BddManager::make_node(
            &self.cell,
            var_id,
            FALSE_NODE,
            TRUE_NODE,
        ))
    }

    /// A constant `true`/`false`.
    #[must_use]
    pub fn constant(&self, value: bool) -> Bdd<'b, B> {
        Self::wrap(if value { TRUE_NODE } else { FALSE_NODE })
    }

    /// Splice an existing [`BoolExpr`] into the build as a handle (its root node).
    ///
    /// This is how an in-scope `BoolExpr` is grafted into a larger expression — the lowering target of
    /// the `expr!` macro's variable operands. The expression must belong to the same manager as the
    /// builder, which always holds: [`Brand`] is sealed, so a brand always maps to exactly one manager
    /// ([`Global`] is process-global; each scoped brand is minted by [`bdd_context!`](crate::bdd_context)
    /// for one context), and the invariant brand type parameter rules out splicing across contexts at
    /// compile time. The `debug_assert!` therefore only guards this internal invariant.
    pub fn graft(&self, expr: &BoolExpr<B>) -> Bdd<'b, B> {
        debug_assert!(
            expr.store_ident() == self.cell.as_ptr(),
            "grafted BoolExpr must share the builder's BDD manager"
        );
        Self::wrap(expr.root_node())
    }

    /// Logical NOT: `ite(a, false, true)`.
    pub fn not(&self, a: Bdd<'b, B>) -> Bdd<'b, B> {
        Self::wrap(BddManager::ite(&self.cell, a.node, FALSE_NODE, TRUE_NODE))
    }

    /// Logical AND: `ite(a, b, false)`.
    pub fn and(&self, a: Bdd<'b, B>, b: Bdd<'b, B>) -> Bdd<'b, B> {
        Self::wrap(BddManager::ite(&self.cell, a.node, b.node, FALSE_NODE))
    }

    /// Logical OR: `ite(a, true, b)`.
    pub fn or(&self, a: Bdd<'b, B>, b: Bdd<'b, B>) -> Bdd<'b, B> {
        Self::wrap(BddManager::ite(&self.cell, a.node, TRUE_NODE, b.node))
    }

    /// Logical XOR: `ite(a, ¬b, b)`.
    pub fn xor(&self, a: Bdd<'b, B>, b: Bdd<'b, B>) -> Bdd<'b, B> {
        Self::wrap(BddManager::xor(&self.cell, a.node, b.node))
    }

    /// If-then-else: `ite(f, g, h)` — the primitive all the others are built from.
    pub fn ite(&self, f: Bdd<'b, B>, g: Bdd<'b, B>, h: Bdd<'b, B>) -> Bdd<'b, B> {
        Self::wrap(BddManager::ite(&self.cell, f.node, g.node, h.node))
    }
}

/// Realise a `build` closure against an explicit `store`, the single construction primitive shared by
/// [`BoolExpr::build`], [`BddContext::build`](crate::BddContext::build), the operators, and the parser.
pub(crate) fn build_in<B, F>(store: Store, f: F) -> BoolExpr<B>
where
    B: Brand,
    F: for<'b> FnOnce(&BddBuilder<'b, B>) -> Bdd<'b, B>,
{
    let cell = SyncCell::from_arc(store);
    let root = {
        let builder = BddBuilder {
            cell: cell.clone(),
            _marker: PhantomData,
        };
        f(&builder).node
    };
    BoolExpr::from_store(cell.into_arc(), root)
}

impl BoolExpr<Global> {
    /// Build an expression by composing [`Bdd`] handles inside a closure.
    ///
    /// The closure receives a [`BddBuilder`] and returns the handle for the root of the expression.
    /// [`graft`](BddBuilder::graft) splices an existing [`BoolExpr`] into the build as a handle. This is
    /// the [`Global`]-brand entry point; a scoped [`BddContext`](crate::BddContext) offers the same via
    /// [`BddContext::build`](crate::BddContext::build).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    ///
    /// // Majority of three, built imperatively.
    /// let majority = BoolExpr::build(|b| {
    ///     let a = b.var("a");
    ///     let bb = b.var("b");
    ///     let c = b.var("c");
    ///     b.or(b.or(b.and(a, bb), b.and(bb, c)), b.and(a, c))
    /// });
    ///
    /// let manual = {
    ///     let a = BoolExpr::variable("a");
    ///     let bb = BoolExpr::variable("b");
    ///     let c = BoolExpr::variable("c");
    ///     a.and(&bb).or(&bb.and(&c)).or(&a.and(&c))
    /// };
    /// assert_eq!(majority, manual);
    /// ```
    #[must_use]
    pub fn build<F>(f: F) -> BoolExpr<Global>
    where
        F: for<'b> FnOnce(&BddBuilder<'b, Global>) -> Bdd<'b, Global>,
    {
        build_in(BddManager::get_or_create(), f)
    }
}

impl<B: Brand> BoolExpr<B> {
    /// If-then-else: `if self then g else h`.
    ///
    /// A convenience wrapper over the builder for the common ternary shape, equivalent to
    /// `self*g + ¬self*h` but built directly from the BDD primitive.
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let c = BoolExpr::variable("c");
    ///
    /// // a ? b : c
    /// let mux = a.ite(&b, &c);
    /// let manual = a.and(&b).or(&a.not().and(&c));
    /// assert_eq!(mux, manual);
    /// ```
    #[must_use]
    pub fn ite(&self, g: &BoolExpr<B>, h: &BoolExpr<B>) -> BoolExpr<B> {
        build_in(self.store_cloned(), |b| {
            let f = b.graft(self);
            let g = b.graft(g);
            let h = b.graft(h);
            b.ite(f, g, h)
        })
    }
}

/// One step of a postfix (reverse-Polish) expression program. The lalrpop string grammar emits a
/// `Vec<Op>` bottom-up, which [`build_postfix`] realises through a single builder activation.
pub(crate) enum Op {
    /// Push a variable by name.
    Var(String),
    /// Push a constant.
    Const(bool),
    /// Pop one operand, push its negation.
    Not,
    /// Pop two operands, push their conjunction.
    And,
    /// Pop two operands, push their disjunction.
    Or,
    /// Pop two operands, push their exclusive-or.
    Xor,
}

/// Realise a postfix [`Op`] program as a [`BoolExpr`] against `store`.
///
/// Evaluated **iteratively** with an explicit value stack (no recursion), so an arbitrarily deep parse
/// — a long operator chain or deep nesting — cannot overflow the call stack. The program is well-formed
/// by construction (the grammar only ever emits balanced postfix), so the stack neither underflows nor
/// ends non-singleton.
pub(crate) fn build_postfix<B: Brand>(store: Store, program: Vec<Op>) -> BoolExpr<B> {
    build_in(store, |b| {
        let mut stack: Vec<Bdd<'_, B>> = Vec::with_capacity(program.len());
        for op in program {
            let node = match op {
                Op::Var(name) => b.var(&name),
                Op::Const(value) => b.constant(value),
                Op::Not => {
                    let a = stack.pop().expect("postfix underflow on NOT");
                    b.not(a)
                }
                Op::And => {
                    let r = stack.pop().expect("postfix underflow on AND");
                    let l = stack.pop().expect("postfix underflow on AND");
                    b.and(l, r)
                }
                Op::Or => {
                    let r = stack.pop().expect("postfix underflow on OR");
                    let l = stack.pop().expect("postfix underflow on OR");
                    b.or(l, r)
                }
                Op::Xor => {
                    let r = stack.pop().expect("postfix underflow on XOR");
                    let l = stack.pop().expect("postfix underflow on XOR");
                    b.xor(l, r)
                }
            };
            stack.push(node);
        }
        stack.pop().expect("postfix program produced no result")
    })
}
