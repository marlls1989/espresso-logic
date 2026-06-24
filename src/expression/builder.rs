//! Low-level BDD construction: the [`BoolExpr::build`] closure builder.
//!
//! [`BoolExpr::build`] calls a closure with a [`BddBuilder`] and returns the [`BoolExpr`] for the handle
//! the closure returns. The builder's methods build [`Bdd`] node handles in the global manager.
//! [`graft`](BddBuilder::graft) turns an existing [`BoolExpr`] into a handle.
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
//! closure or smuggled between two `build` calls — misuse is a compile error. (The brand guards
//! *cross-builder* handle hygiene; a [`graft`](BddBuilder::graft)ed expression is tied to the manager by
//! the single global manager, see `graft`.)
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

use super::manager::{BddManager, NodeId, FALSE_NODE, TRUE_NODE};
use super::BoolExpr;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};

/// A handle to a node being built inside a [`BoolExpr::build`] closure.
///
/// `Copy` (it is a node id). The `'b` lifetime is an **invariant brand** tying the handle
/// to one [`BddBuilder`] activation, so the borrow checker rejects any attempt to move a handle out of
/// its closure or use it with a different builder. Handles are opaque: combine them only through the
/// [`BddBuilder`] methods.
#[derive(Clone, Copy)]
pub struct Bdd<'b> {
    node: NodeId,
    // Invariant in 'b (neither co- nor contravariant), so the brand cannot be widened or narrowed.
    _brand: PhantomData<fn(&'b ()) -> &'b ()>,
}

/// The builder handed to a [`BoolExpr::build`] closure: the BDD manager's node-construction operations.
///
/// Methods take `&self`, so handles nest in one expression — `b.and(b.var("a"), b.not(b.var("b")))`. Each
/// method hash-conses through the manager — a read lock for the lookup, the write lock to append a new
/// node — and returns a canonical [`Bdd`] handle.
pub struct BddBuilder<'b> {
    manager: Arc<RwLock<BddManager>>,
    /// Identity of the manager, used only to debug-assert that a [`graft`](Self::graft)ed expression
    /// belongs to it (it always does — there is one global manager). Equal to `Arc::as_ptr(&manager)`.
    manager_ptr: *const RwLock<BddManager>,
    /// Invariant brand (see [`Bdd`]); the builder owns the `Arc`, so `'b` is carried only as a marker.
    _brand: PhantomData<fn(&'b ()) -> &'b ()>,
}

impl<'b> BddBuilder<'b> {
    #[inline]
    fn wrap(node: NodeId) -> Bdd<'b> {
        Bdd {
            node,
            _brand: PhantomData,
        }
    }

    /// A variable by name (creating it in the manager's ordering on first use).
    pub fn var<S: AsRef<str>>(&self, name: S) -> Bdd<'b> {
        let var_id = BddManager::make_var(&self.manager, name.as_ref());
        Self::wrap(BddManager::make_node(
            &self.manager,
            var_id,
            FALSE_NODE,
            TRUE_NODE,
        ))
    }

    /// A constant `true`/`false`.
    #[must_use]
    pub fn constant(&self, value: bool) -> Bdd<'b> {
        Self::wrap(if value { TRUE_NODE } else { FALSE_NODE })
    }

    /// Splice an existing [`BoolExpr`] into the build as a handle (its root node).
    ///
    /// This is how an in-scope `BoolExpr` is grafted into a larger expression — the lowering target of
    /// the `expr!` macro's variable operands. The expression must belong to the same (global) manager;
    /// checked with a `debug_assert!`, as it always holds while the operand is alive.
    pub fn graft(&self, expr: &BoolExpr) -> Bdd<'b> {
        // Soundness rests on the *operand's* `Arc<RwLock<BddManager>>`, not on the assert below: while any
        // `BoolExpr` is alive the global manager cannot be freed/replaced, so `expr.root` indexes the same
        // node table the builder writes. The `debug_assert!` is a debug-only sanity check (compiled out in
        // release). This is why `graft` takes `&BoolExpr` (which owns that `Arc`) and never a bare node id.
        debug_assert!(
            std::ptr::eq(Arc::as_ptr(&expr.manager), self.manager_ptr),
            "grafted BoolExpr must share the builder's BDD manager"
        );
        Self::wrap(expr.root)
    }

    /// Logical NOT: `ite(a, false, true)`.
    pub fn not(&self, a: Bdd<'b>) -> Bdd<'b> {
        Self::wrap(BddManager::ite(
            &self.manager,
            a.node,
            FALSE_NODE,
            TRUE_NODE,
        ))
    }

    /// Logical AND: `ite(a, b, false)`.
    pub fn and(&self, a: Bdd<'b>, b: Bdd<'b>) -> Bdd<'b> {
        Self::wrap(BddManager::ite(&self.manager, a.node, b.node, FALSE_NODE))
    }

    /// Logical OR: `ite(a, true, b)`.
    pub fn or(&self, a: Bdd<'b>, b: Bdd<'b>) -> Bdd<'b> {
        Self::wrap(BddManager::ite(&self.manager, a.node, TRUE_NODE, b.node))
    }

    /// Logical XOR: `ite(a, ¬b, b)`.
    pub fn xor(&self, a: Bdd<'b>, b: Bdd<'b>) -> Bdd<'b> {
        Self::wrap(BddManager::xor(&self.manager, a.node, b.node))
    }

    /// If-then-else: `ite(f, g, h)` — the primitive all the others are built from.
    pub fn ite(&self, f: Bdd<'b>, g: Bdd<'b>, h: Bdd<'b>) -> Bdd<'b> {
        Self::wrap(BddManager::ite(&self.manager, f.node, g.node, h.node))
    }
}

impl BoolExpr {
    /// Build an expression by composing [`Bdd`] handles inside a closure.
    ///
    /// The closure receives a [`BddBuilder`] and returns the handle for the root of the expression.
    /// [`graft`](BddBuilder::graft) splices an existing [`BoolExpr`] into the build as a handle.
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
    pub fn build<F>(f: F) -> BoolExpr
    where
        F: for<'b> FnOnce(&BddBuilder<'b>) -> Bdd<'b>,
    {
        // Resolve the singleton manager once for the whole build; the builder holds an `Arc` clone and
        // takes the write lock per node-creating step.
        let manager = BddManager::get_or_create();
        let manager_ptr = Arc::as_ptr(&manager);
        let root = {
            let builder = BddBuilder {
                manager: Arc::clone(&manager),
                manager_ptr,
                _brand: PhantomData,
            };
            f(&builder).node
        };
        BoolExpr::from_root(manager, root)
    }

    /// If-then-else: `if self then g else h`.
    ///
    /// A convenience wrapper over [`build`](Self::build) for the common ternary shape, equivalent to
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
    pub fn ite(&self, g: &BoolExpr, h: &BoolExpr) -> BoolExpr {
        BoolExpr::build(|b| {
            let f = b.graft(self);
            let g = b.graft(g);
            let h = b.graft(h);
            b.ite(f, g, h)
        })
    }
}

/// One step of a postfix (reverse-Polish) expression program. The lalrpop string grammar emits a
/// `Vec<Op>` bottom-up, which [`build_postfix`] realises through a single [`BoolExpr::build`].
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

/// Realise a postfix [`Op`] program as a [`BoolExpr`], locking the manager per node-creating step.
///
/// Evaluated **iteratively** with an explicit value stack (no recursion), so an arbitrarily deep parse
/// — a long operator chain or deep nesting — cannot overflow the call stack, matching the no-recursion
/// discipline used elsewhere for deep expression trees. The program is well-formed by construction (the
/// grammar only ever emits balanced postfix), so the stack neither underflows nor ends non-singleton.
pub(crate) fn build_postfix(program: Vec<Op>) -> BoolExpr {
    BoolExpr::build(|b| {
        let mut stack: Vec<Bdd<'_>> = Vec::with_capacity(program.len());
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
