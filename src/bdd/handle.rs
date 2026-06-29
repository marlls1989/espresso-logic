//! The borrowed, `Copy` BDD handle.
//!
//! A [`Bdd`] is a lightweight handle into a [`BddContext`](super::BddContext) /
//! [`SyncBddContext`](super::SyncBddContext): a shared reference to the context's cell plus the
//! canonical root node it denotes. Because it borrows the cell (`&'ctx B::Cell`) rather than owning it,
//! it is `Copy` — `a & b`, `&a & b`, etc. all work without clones or deref gymnastics.
//!
//! Two handles can be combined only when they share the same lifetime `'ctx` **and** the same brand
//! `B` — i.e. when they came from the same context. Mixing handles from two different contexts is a
//! compile error, enforced by the invariant brand parameter; there is no runtime check.

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::Arc;

use super::brand::Brand;
use crate::cover::{Anonymous, Cover, CoverType, Cube, CubeType, Minterm, OutputSet, Symbols};
use crate::expression::manager::{BddManager, BddNode, NodeId, FALSE_NODE, TRUE_NODE};
use crate::expression::manager_cell::ManagerCell;
use crate::expression::BoolExpr;
use crate::Symbol;

/// A borrowed, `Copy` handle to a canonical BDD root within one context.
///
/// `mgr` borrows the owning context's cell, so the handle cannot outlive the context (a compile error,
/// not a runtime one). `root` is the canonical node id; the brand `B` is carried as an invariant
/// type-level marker (`PhantomData<fn() -> B>`) so handles of different contexts never unify.
///
/// # Canonicity
///
/// Within one context every Boolean function has exactly one root node, so two handles denote the same
/// function **iff** their roots are equal — making [`equivalent_to`](Self::equivalent_to) an O(1) id
/// comparison. The brand+lifetime pairing guarantees both handles share the same manager before such a
/// comparison is even type-correct.
///
/// # Lifetime soundness
///
/// A handle borrows its context, so it cannot escape:
///
/// ```compile_fail
/// use espresso_logic::bdd::{Bdd, BddContext, Brand};
/// fn escape<B: Brand>() {
///     let f;
///     {
///         let ctx: BddContext<B> = BddContext::new();
///         f = ctx.var("a"); // borrows ctx
///     } // ctx dropped here
///     let _ = f.node_count(); // error: `ctx` does not live long enough
/// }
/// ```
///
/// # Cross-context mixing is a compile error
///
/// Handles of two different contexts carry distinct brands, which never unify, so an operator over them
/// fails to type-check:
///
/// ```compile_fail
/// use espresso_logic::bdd::{BddContext, Brand};
/// fn mix<B1: Brand, B2: Brand>(c1: &BddContext<B1>, c2: &BddContext<B2>) {
///     let a = c1.var("a");
///     let b = c2.var("b");
///     let _ = a & b; // error: distinct brands `B1` and `B2` do not unify
/// }
/// ```
pub struct Bdd<'ctx, B: Brand> {
    mgr: &'ctx B::Cell,
    root: NodeId,
    _brand: PhantomData<fn() -> B>,
}

impl<B: Brand> Clone for Bdd<'_, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: Brand> Copy for Bdd<'_, B> {}

impl<'ctx, B: Brand> Bdd<'ctx, B> {
    /// Wrap a raw root node into a handle bound to `mgr`. Crate-internal: only the context and the
    /// operator impls mint handles, so every `Bdd` is guaranteed to denote a node in `mgr`.
    pub(super) fn from_root(mgr: &'ctx B::Cell, root: NodeId) -> Self {
        Bdd {
            mgr,
            root,
            _brand: PhantomData,
        }
    }

    // ---- Boolean operations -------------------------------------------------------------------

    /// Logical AND of two handles: `self ∧ other`. Equivalent to the `&` operator.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        let root = BddManager::ite(self.mgr, self.root, other.root, FALSE_NODE);
        Self::from_root(self.mgr, root)
    }

    /// Logical OR of two handles: `self ∨ other`. Equivalent to the `|` operator.
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        let root = BddManager::ite(self.mgr, self.root, TRUE_NODE, other.root);
        Self::from_root(self.mgr, root)
    }

    /// Logical XOR of two handles: `self ⊕ other`. Equivalent to the `^` operator.
    #[must_use]
    pub fn xor(self, other: Self) -> Self {
        let root = BddManager::xor(self.mgr, self.root, other.root);
        Self::from_root(self.mgr, root)
    }

    /// Logical NOT: `¬self`. Equivalent to the unary `!` operator (which delegates here).
    #[must_use]
    pub fn complement(self) -> Self {
        let root = BddManager::ite(self.mgr, self.root, FALSE_NODE, TRUE_NODE);
        Self::from_root(self.mgr, root)
    }

    /// If-then-else: `if self then g else h`. The fundamental BDD operation the others derive from.
    #[must_use]
    pub fn ite(self, g: Self, h: Self) -> Self {
        let root = BddManager::ite(self.mgr, self.root, g.root, h.root);
        Self::from_root(self.mgr, root)
    }

    // ---- Shannon cofactor / quantification (requirement 1) ------------------------------------

    /// Shannon cofactor by assignment (a.k.a. *restrict*): `self|var=value`.
    ///
    /// Substitutes the variable named `var` with the constant `value` and returns the canonical reduced
    /// result. A name that is **not** a variable of this function leaves it unchanged (a no-op);
    /// restricting every support variable collapses the function to a constant.
    #[must_use]
    pub fn restrict<S: AsRef<str>>(self, var: S, value: bool) -> Self {
        // Resolve the name without creating it: an absent variable yields `None` → no-op.
        let var_id = self.mgr.read().var_id(var.as_ref());
        match var_id {
            None => self,
            Some(id) => {
                let root = BddManager::restrict(self.mgr, self.root, id, value);
                Self::from_root(self.mgr, root)
            }
        }
    }

    /// Shannon cofactor by assignment — an alias of [`restrict`](Self::restrict).
    ///
    /// In this BDD modelling the cofactor *by a single assignment* and `restrict` are the same
    /// operation, so `cofactor` is provided as the conventional name; both substitute `var := value` and
    /// reduce. For a multi-variable cofactor, chain `restrict`/`cofactor` calls (or use
    /// [`forall`](Self::forall) / [`exists`](Self::exists) to quantify).
    #[must_use]
    pub fn cofactor<S: AsRef<str>>(self, var: S, value: bool) -> Self {
        self.restrict(var, value)
    }

    /// Universal quantification over `vars`: `∀v∈vars. self`.
    ///
    /// Folds `restrict(v, true) & restrict(v, false)` across each variable in `vars`. A name absent from
    /// the function contributes a no-op cofactor (`self & self == self`). Quantifying over no variables
    /// returns `self`.
    #[must_use]
    pub fn forall<S: AsRef<str>>(self, vars: &[S]) -> Self {
        let mut acc = self;
        for v in vars {
            let lo = acc.restrict(v.as_ref(), false);
            let hi = acc.restrict(v.as_ref(), true);
            acc = hi.and(lo);
        }
        acc
    }

    /// Existential quantification over `vars`: `∃v∈vars. self`.
    ///
    /// Folds `restrict(v, true) | restrict(v, false)` across each variable in `vars`. A name absent from
    /// the function contributes a no-op cofactor (`self | self == self`). Quantifying over no variables
    /// returns `self`.
    #[must_use]
    pub fn exists<S: AsRef<str>>(self, vars: &[S]) -> Self {
        let mut acc = self;
        for v in vars {
            let lo = acc.restrict(v.as_ref(), false);
            let hi = acc.restrict(v.as_ref(), true);
            acc = hi.or(lo);
        }
        acc
    }

    // ---- Tautology / contradiction (requirement 4) --------------------------------------------

    /// Whether this function is the constant `true` (a tautology). O(1): the canonical root is the TRUE
    /// terminal.
    #[must_use]
    pub fn is_tautology(self) -> bool {
        self.root == TRUE_NODE
    }

    /// Whether this function is the constant `false` (a contradiction). O(1): the canonical root is the
    /// FALSE terminal.
    #[must_use]
    pub fn is_contradiction(self) -> bool {
        self.root == FALSE_NODE
    }

    // ---- Canonical equivalence ----------------------------------------------------------------

    /// Whether `self` and `other` denote the same Boolean function. O(1).
    ///
    /// Sharing the lifetime `'ctx` and brand `B` means both handles came from the same context, hence
    /// the same canonical manager, so equal functions have equal roots and this reduces to a root-id
    /// comparison. (A `debug_assert!` confirms the two cells are physically the same.)
    #[must_use]
    pub fn equivalent_to(self, other: Self) -> bool {
        debug_assert_eq!(
            self.mgr.as_ptr(),
            other.mgr.as_ptr(),
            "handles of the same brand+lifetime must share one manager"
        );
        self.root == other.root
    }

    // ---- Introspection ------------------------------------------------------------------------

    /// The variables this function depends on, sorted alphabetically by name.
    #[must_use]
    pub fn collect_variables(self) -> Vec<Symbol> {
        let mut ids = std::collections::HashSet::new();
        self.collect_var_ids(&mut ids);
        let mgr = self.mgr.read();
        let mut names: Vec<Symbol> = ids
            .into_iter()
            .filter_map(|id| mgr.var_name(id).cloned())
            .collect();
        names.sort();
        names
    }

    /// Number of distinct, reachable nodes in this BDD (including reached terminals).
    #[must_use]
    pub fn node_count(self) -> usize {
        let mgr = self.mgr.read();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![self.root];
        let mut count = 0;
        while let Some(node) = stack.pop() {
            if !visited.insert(node) {
                continue;
            }
            count += 1;
            match mgr.get_node(node) {
                Some(BddNode::Terminal(_)) => {}
                Some(BddNode::Decision { low, high, .. }) => {
                    stack.push(*low);
                    stack.push(*high);
                }
                None => panic!(
                    "Invalid node ID {node} encountered during node counting - this indicates a bug in the BDD implementation"
                ),
            }
        }
        count
    }

    /// Number of distinct variables this function depends on.
    #[must_use]
    pub fn var_count(self) -> usize {
        let mut ids = std::collections::HashSet::new();
        self.collect_var_ids(&mut ids);
        ids.len()
    }

    /// Collect every variable id reachable from the root (iterative DFS, deduplicated on nodes so a
    /// variable labelling several nodes is never missed). One read guard for the whole walk (NodeIds are
    /// stable).
    fn collect_var_ids(self, vars: &mut std::collections::HashSet<usize>) {
        let mgr = self.mgr.read();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![self.root];
        while let Some(node) = stack.pop() {
            if !visited.insert(node) {
                continue;
            }
            match mgr.get_node(node) {
                Some(BddNode::Terminal(_)) => {}
                Some(BddNode::Decision { var, low, high }) => {
                    vars.insert(*var);
                    stack.push(*low);
                    stack.push(*high);
                }
                None => panic!(
                    "Invalid node ID {node} encountered during variable collection - this indicates a bug in the BDD implementation"
                ),
            }
        }
    }

    // ---- Cover / minterm materialisation ------------------------------------------------------

    /// Enumerate the paths to TRUE as a single-output sum-of-products [`Cover`].
    ///
    /// Each root→TRUE path becomes one input cube: a variable on the path is fixed `Some(true)` /
    /// `Some(false)`, a variable off the path is a don't-care (`None`). Variables are carried by name
    /// (`Symbol`); the output side is a single [`Anonymous`] column, asserted by every cube — i.e. the
    /// cover is the **characteristic function** of this BDD over its support variables. The returned
    /// cover is an `F` (ON-set) cover.
    #[must_use]
    pub fn to_cubes(self) -> Cover<Symbol, Anonymous> {
        // Canonical, alphabetically sorted header shared by every extracted cube.
        let vars: Arc<[Symbol]> = self.collect_variables().into_iter().collect();
        let index: std::collections::HashMap<Symbol, usize> = vars
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, v)| (v, i))
            .collect();
        let symbols = Symbols::new(vars);
        // One asserted Anonymous output column, shared by every cube.
        let output_symbols = Symbols::<Anonymous>::anonymous(1);

        // Iterative DFS enumerating every root→TRUE path. `SetPath` items replay the recursive
        // "fix this header slot on descent, restore on backtrack" around the two child visits; LIFO
        // order makes them fire exactly as the recursion would. One read guard for the whole walk.
        enum Work {
            Node(NodeId),
            SetPath(usize, Option<bool>),
        }

        let mut cubes: Vec<Cube<Symbol, Anonymous>> = Vec::new();
        let mut path: Vec<Option<bool>> = vec![None; symbols.arity()];

        let mgr = self.mgr.read();
        let mut stack = vec![Work::Node(self.root)];
        while let Some(work) = stack.pop() {
            match work {
                Work::SetPath(i, value) => path[i] = value,
                Work::Node(node) => match mgr.get_node(node) {
                    Some(BddNode::Terminal(true)) => {
                        let inputs =
                            Minterm::from_symbols(Arc::clone(&symbols), path.iter().copied());
                        let outputs = OutputSet::from_symbols(
                            Arc::clone(&output_symbols),
                            std::iter::once(true),
                        );
                        cubes.push(Cube::new(inputs, outputs, CubeType::F));
                    }
                    Some(BddNode::Terminal(false)) => {}
                    Some(BddNode::Decision { var, low, high }) => {
                        let var_name = mgr.var_name(*var).expect(
                            "Invalid variable ID encountered during cube extraction - this indicates a bug in the BDD implementation",
                        );
                        let i = *index.get(var_name).expect(
                            "BDD variable absent from the collected header - this indicates a bug in the BDD implementation",
                        );
                        stack.push(Work::SetPath(i, None));
                        stack.push(Work::Node(*high));
                        stack.push(Work::SetPath(i, Some(true)));
                        stack.push(Work::Node(*low));
                        stack.push(Work::SetPath(i, Some(false)));
                    }
                    None => panic!(
                        "Invalid node ID {node} encountered during cube extraction - this indicates a bug in the BDD implementation"
                    ),
                },
            }
        }
        drop(mgr);

        Cover::from_cubes(CoverType::F, cubes)
    }

    /// Fully-expanded canonical minterms over the explicit, widenable variable set `vars`
    /// (requirement 2).
    ///
    /// Every returned [`Minterm`] assigns **every** variable in `vars` (no don't-cares left). `vars` MAY
    /// be a superset of this function's support: a variable in `vars` absent from the function is split
    /// into both polarities. A variable of the function omitted from `vars` is dropped — for the inverse
    /// of minimisation, pass at least the function's support. All returned minterms share one canonical,
    /// identity-aligned header (so they stay on the fast-comparison path and are usable in
    /// `BTreeSet`/`HashMap`). The result is deduplicated and returned in a deterministic (sorted) order;
    /// expanding the same function over the same `vars` is idempotent.
    ///
    /// Built from [`to_cubes`](Self::to_cubes) via [`Cover::maximize`].
    #[must_use]
    pub fn to_minterms<S: AsRef<str>>(self, vars: &[S]) -> Vec<Minterm<Symbol>> {
        let header: Vec<Symbol> = vars.iter().map(|s| Symbol::new(s.as_ref())).collect();
        let maximal = self.to_cubes().maximize(&header);
        let mut minterms: Vec<Minterm<Symbol>> =
            maximal.cubes().map(|c| c.inputs().clone()).collect();
        minterms.sort();
        minterms.dedup();
        minterms
    }

    // ---- Minimisation -------------------------------------------------------------------------

    /// Minimise this function's ON-set with Espresso, returning the minimised single-output
    /// [`Cover`].
    ///
    /// Equivalent to `self.to_cubes().minimize()`; the cover is the characteristic function over the
    /// support variables (see [`to_cubes`](Self::to_cubes)).
    ///
    /// # Errors
    ///
    /// Propagates any [`MinimizationError`](crate::error::MinimizationError) from the Espresso engine.
    pub fn minimize(self) -> Result<Cover<Symbol, Anonymous>, crate::error::MinimizationError> {
        use crate::Minimizable;
        self.to_cubes().minimize()
    }

    // ---- Lowering back to a syntactic expression ----------------------------------------------

    /// Lower this function to an owned, factored [`BoolExpr`].
    ///
    /// Enumerates the function's ON-set with [`to_cubes`](Self::to_cubes), then applies algebraic
    /// **direct factorisation** to that cube set (the same path as [`Cover::to_expr`]) to produce a
    /// compact multi-level expression — it does **not** re-canonicalise through the BDD. The resulting
    /// `BoolExpr` is syntactic; building it back here yields the same canonical handle, but its token
    /// structure reflects the factored cubes, not this BDD's node graph.
    #[must_use]
    pub fn to_expr(self) -> BoolExpr {
        self.to_cubes()
            .to_expr_by_index(0)
            .expect("to_cubes yields a single-output cover, so output index 0 is in bounds")
    }
}

/// Canonical equality: two handles are equal iff they denote the same function (same root within the
/// shared manager). Only handles of the same brand+lifetime are comparable, which guarantees they share
/// a manager.
impl<B: Brand> PartialEq for Bdd<'_, B> {
    fn eq(&self, other: &Self) -> bool {
        debug_assert_eq!(
            self.mgr.as_ptr(),
            other.mgr.as_ptr(),
            "handles of the same brand+lifetime must share one manager"
        );
        self.root == other.root
    }
}

impl<B: Brand> Eq for Bdd<'_, B> {}

/// Shows the canonical root id (the function's identity within its context) and the manager pointer, so
/// two handles that are `==` print equal roots. The decoded function is not rendered — use
/// [`to_cubes`](Bdd::to_cubes) for that.
impl<B: Brand> std::fmt::Debug for Bdd<'_, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bdd")
            .field("root", &self.root)
            .field("mgr", &self.mgr.as_ptr())
            .finish()
    }
}

// ---- Operator overloading -------------------------------------------------------------------------
//
// Each operator is implemented for every owned/borrowed combination so `a & b`, `&a & b`, `a & &b`, and
// `&a & &b` all type-check. Handles are `Copy`, so a `&Bdd` is dereferenced cheaply. Mixing two
// different contexts is impossible: the operands must share `'ctx` and `B`, and a mismatch is a compile
// error (see the `Bdd` type docs for a `compile_fail` example).

macro_rules! bin_op {
    ($trait:ident, $method:ident, $call:ident) => {
        impl<'ctx, B: Brand> std::ops::$trait for Bdd<'ctx, B> {
            type Output = Bdd<'ctx, B>;
            fn $method(self, rhs: Bdd<'ctx, B>) -> Bdd<'ctx, B> {
                Bdd::$call(self, rhs)
            }
        }
        impl<'ctx, B: Brand> std::ops::$trait<&Bdd<'ctx, B>> for Bdd<'ctx, B> {
            type Output = Bdd<'ctx, B>;
            fn $method(self, rhs: &Bdd<'ctx, B>) -> Bdd<'ctx, B> {
                Bdd::$call(self, *rhs)
            }
        }
        impl<'ctx, B: Brand> std::ops::$trait<Bdd<'ctx, B>> for &Bdd<'ctx, B> {
            type Output = Bdd<'ctx, B>;
            fn $method(self, rhs: Bdd<'ctx, B>) -> Bdd<'ctx, B> {
                Bdd::$call(*self, rhs)
            }
        }
        impl<'ctx, B: Brand> std::ops::$trait<&Bdd<'ctx, B>> for &Bdd<'ctx, B> {
            type Output = Bdd<'ctx, B>;
            fn $method(self, rhs: &Bdd<'ctx, B>) -> Bdd<'ctx, B> {
                Bdd::$call(*self, *rhs)
            }
        }
    };
}

bin_op!(BitAnd, bitand, and);
bin_op!(BitOr, bitor, or);
bin_op!(BitXor, bitxor, xor);

impl<'ctx, B: Brand> std::ops::Not for Bdd<'ctx, B> {
    type Output = Bdd<'ctx, B>;
    fn not(self) -> Bdd<'ctx, B> {
        Bdd::complement(self)
    }
}

impl<'ctx, B: Brand> std::ops::Not for &Bdd<'ctx, B> {
    type Output = Bdd<'ctx, B>;
    fn not(self) -> Bdd<'ctx, B> {
        Bdd::complement(*self)
    }
}

/// The variables a [`Bdd`] depends on as a `BTreeSet` (a canonical-order convenience over
/// [`collect_variables`](Bdd::collect_variables)).
impl<B: Brand> Bdd<'_, B> {
    /// The variables this function depends on, as a `BTreeSet<Symbol>` (canonical iteration order).
    #[must_use]
    pub fn variables(self) -> BTreeSet<Symbol> {
        self.collect_variables().into_iter().collect()
    }
}
