//! The owned, refcounted BDD handle.
//!
//! A [`Bdd`] is a lightweight handle into a [`BddBuilder`](super::BddBuilder): a refcounted clone of the
//! builder's storage cell plus the canonical root node it denotes. Because it owns a clone of the cell
//! (rather than borrowing the builder), it keeps its manager alive and can be stored, returned, or
//! outlive the builder that minted it.
//!
//! A handle carries two orthogonal type parameters: a [`Brand`] `B` (uniqueness only) and a
//! [`ManagerCell`] `C` (the storage backend). Two handles can be combined only when they share both — i.e.
//! when they came from the same builder. Mixing handles of two different brands is a compile error,
//! enforced by the invariant brand parameter. As a runtime backstop against a brand clash (two builders
//! that happen to share a brand type), every binary operation asserts the two handles point at the same
//! manager.
//!
//! The handle is [`Clone`] (a refcount bump) but **not** `Copy`: operators consume their operands by
//! value, with reference variants for reuse, while derivation and query methods borrow `&self`.

use std::borrow::Borrow;
use std::collections::{BTreeSet, HashMap};
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;

use super::brand::Brand;
use crate::cover::{Anonymous, Cover, CoverType, Cube, CubeType, Minterm, OutputSet, Symbols};
use crate::impl_binary_operator;
use crate::expression::manager::{
    BddManager, BddNode as ManagerNode, NodeId, FALSE_NODE, TRUE_NODE,
};
use crate::expression::manager_cell::ManagerCell;
use crate::expression::BoolExpr;
use crate::Symbol;

/// An owned, refcounted handle to a canonical BDD root within one builder.
///
/// `cell` is a refcounted clone of the owning builder's storage cell, so the handle keeps its manager
/// alive and is independent of the builder's lifetime. `root` is the canonical node id; the brand `B` is
/// carried as an invariant type-level marker (`PhantomData<fn() -> B>`) so handles of different builders
/// never unify, and `C` is the storage backend.
///
/// # Canonicity
///
/// Within one builder every Boolean function has exactly one root node, so two handles denote the same
/// function **iff** their roots are equal — making [`equivalent_to`](Self::equivalent_to) an O(1) id
/// comparison. The brand pairing guarantees both handles share the same manager before such a comparison
/// is even type-correct; the manager-pointer assert is a runtime backstop against a brand clash.
///
/// # Cross-builder mixing is a compile error
///
/// Handles of two different builders carry distinct brands, which never unify, so an operator over them
/// fails to type-check:
///
/// ```compile_fail
/// use espresso_logic::bdd::{BddBuilder, Brand};
/// use espresso_logic::bdd::__macro_support::LocalCell;
/// fn mix<B1: Brand, B2: Brand>(c1: &BddBuilder<B1, LocalCell>, c2: &BddBuilder<B2, LocalCell>) {
///     let a = c1.var("a");
///     let b = c2.var("b");
///     let _ = a & b; // error: distinct brands `B1` and `B2` do not unify
/// }
/// ```
pub struct Bdd<B: Brand, C: ManagerCell> {
    cell: C,
    root: NodeId,
    _brand: PhantomData<fn() -> B>,
}

impl<B: Brand, C: ManagerCell> Clone for Bdd<B, C> {
    fn clone(&self) -> Self {
        Bdd {
            cell: self.cell.clone(),
            root: self.root,
            _brand: PhantomData,
        }
    }
}

impl<B: Brand, C: ManagerCell> Bdd<B, C> {
    /// Wrap a raw root node into a handle owning a refcounted clone of `cell`. Crate-internal: only the
    /// builder and the operator impls mint handles, so every `Bdd` is guaranteed to denote a node in
    /// `cell`'s manager.
    pub(super) fn from_root(cell: &C, root: NodeId) -> Self {
        Bdd {
            cell: cell.clone(),
            root,
            _brand: PhantomData,
        }
    }

    /// Assert that two handles share one manager. A type-correct pair of handles came from the same
    /// builder unless two builders happen to share a brand type (a clash); this catches that at runtime so
    /// a mismatched pair never silently computes against the wrong manager.
    fn assert_same_manager(&self, other: &Self) {
        assert!(
            self.cell.as_ptr() == other.cell.as_ptr(),
            "BDD handles come from different managers (a brand clash); mixing them is a bug"
        );
    }

    // ---- Boolean operations -------------------------------------------------------------------

    /// Logical AND of two handles: `self ∧ other`. Equivalent to the `&` operator.
    #[must_use]
    pub fn and(&self, other: &Self) -> Self {
        self.assert_same_manager(other);
        let root = BddManager::ite(&self.cell, self.root, other.root, FALSE_NODE);
        Self::from_root(&self.cell, root)
    }

    /// Logical OR of two handles: `self ∨ other`. Equivalent to the `|` operator.
    #[must_use]
    pub fn or(&self, other: &Self) -> Self {
        self.assert_same_manager(other);
        let root = BddManager::ite(&self.cell, self.root, TRUE_NODE, other.root);
        Self::from_root(&self.cell, root)
    }

    /// Logical XOR of two handles: `self ⊕ other`. Equivalent to the `^` operator.
    #[must_use]
    pub fn xor(&self, other: &Self) -> Self {
        self.assert_same_manager(other);
        let root = BddManager::xor(&self.cell, self.root, other.root);
        Self::from_root(&self.cell, root)
    }

    /// Logical NOT: `¬self`. Equivalent to the unary `!` operator (which delegates here).
    #[must_use]
    pub fn complement(self) -> Self {
        let root = BddManager::ite(&self.cell, self.root, FALSE_NODE, TRUE_NODE);
        Self::from_root(&self.cell, root)
    }

    /// If-then-else: `if self then g else h`. The fundamental BDD operation the others derive from.
    #[must_use]
    pub fn ite(self, g: Self, h: Self) -> Self {
        self.assert_same_manager(&g);
        self.assert_same_manager(&h);
        let root = BddManager::ite(&self.cell, self.root, g.root, h.root);
        Self::from_root(&self.cell, root)
    }

    // ---- Shannon cofactor / quantification (requirement 1) ------------------------------------

    /// Shannon cofactor by assignment (a.k.a. *restrict*): `self|var=value`.
    ///
    /// Substitutes the variable named `var` with the constant `value` and returns the canonical reduced
    /// result. A name that is **not** a variable of this function leaves it unchanged (a no-op);
    /// restricting every support variable collapses the function to a constant.
    #[must_use]
    pub fn restrict<S: AsRef<str>>(&self, var: S, value: bool) -> Self {
        // Resolve the name without creating it: an absent variable yields `None` → no-op.
        let var_id = self.cell.read().var_id(var.as_ref());
        match var_id {
            None => self.clone(),
            Some(id) => {
                let root = BddManager::restrict(&self.cell, self.root, id, value);
                Self::from_root(&self.cell, root)
            }
        }
    }

    /// Shannon cofactor by assignment — an alias of [`restrict`](Self::restrict).
    ///
    /// In this BDD modelling the cofactor *by a single assignment* and `restrict` are the same operation,
    /// so `cofactor` is provided as the conventional name; both substitute `var := value` and reduce. For a
    /// multi-variable cofactor, chain `restrict`/`cofactor` calls (or use [`forall`](Self::forall) /
    /// [`exists`](Self::exists) to quantify).
    #[must_use]
    pub fn cofactor<S: AsRef<str>>(&self, var: S, value: bool) -> Self {
        self.restrict(var, value)
    }

    /// Universal quantification over `vars`: `∀v∈vars. self`.
    ///
    /// Folds `restrict(v, true) & restrict(v, false)` across each variable in `vars`. A name absent from
    /// the function contributes a no-op cofactor (`self & self == self`). Quantifying over no variables
    /// returns `self`.
    #[must_use]
    pub fn forall<S: AsRef<str>>(&self, vars: &[S]) -> Self {
        self.quantify(vars, Self::and)
    }

    /// Existential quantification over `vars`: `∃v∈vars. self`.
    ///
    /// Folds `restrict(v, true) | restrict(v, false)` across each variable in `vars`. A name absent from
    /// the function contributes a no-op cofactor (`self | self == self`). Quantifying over no variables
    /// returns `self`.
    #[must_use]
    pub fn exists<S: AsRef<str>>(&self, vars: &[S]) -> Self {
        self.quantify(vars, Self::or)
    }

    /// Quantify over `vars`, folding `combine` across each variable's two cofactors. Universal and
    /// existential quantification differ only in `combine` (`and` vs `or`); this is the shared body.
    fn quantify<S: AsRef<str>>(&self, vars: &[S], combine: fn(&Self, &Self) -> Self) -> Self {
        let mut acc = self.clone();
        for v in vars {
            let lo = acc.restrict(v.as_ref(), false);
            let hi = acc.restrict(v.as_ref(), true);
            acc = combine(&hi, &lo);
        }
        acc
    }

    // ---- Tautology / contradiction (requirement 4) --------------------------------------------

    /// Whether this function is the constant `true` (a tautology). O(1): the canonical root is the TRUE
    /// terminal.
    #[must_use]
    pub fn is_tautology(&self) -> bool {
        self.root == TRUE_NODE
    }

    /// Whether this function is the constant `false` (a contradiction). O(1): the canonical root is the
    /// FALSE terminal.
    #[must_use]
    pub fn is_contradiction(&self) -> bool {
        self.root == FALSE_NODE
    }

    // ---- Canonical equivalence ----------------------------------------------------------------

    /// Whether `self` and `other` denote the same Boolean function. O(1).
    ///
    /// Sharing the brand `B` means both handles came from the same builder, hence the same canonical
    /// manager, so equal functions have equal roots and this reduces to a root-id comparison. An assert
    /// confirms the two cells are physically the same (a runtime backstop against a brand clash).
    #[must_use]
    pub fn equivalent_to(&self, other: &Self) -> bool {
        self.assert_same_manager(other);
        self.root == other.root
    }

    // ---- Evaluation ---------------------------------------------------------------------------

    /// Evaluate this function under a variable `assignment`.
    ///
    /// This is the canonical evaluation: it follows a single root→terminal path, branching at each
    /// decision node on the assigned value of that node's variable, so the cost is O(path length)
    /// — at most the number of variables the function depends on — regardless of how large the
    /// original syntactic expression was, and shared subfunctions are visited once. **A variable
    /// absent from `assignment` reads as `false`** (partial assignments are allowed). The key type
    /// may be any `Borrow<str>` (`&str`, `String`, [`Symbol`], `Arc<str>`, …).
    ///
    /// Evaluation is a semantic operation, so it lives here rather than on the syntactic
    /// [`BoolExpr`](crate::BoolExpr): build the expression into a builder with
    /// [`BddBuilder::build`](crate::bdd::BddBuilder::build) first.
    #[must_use]
    pub fn evaluate<K>(&self, assignment: &HashMap<K, bool>) -> bool
    where
        K: Borrow<str> + Eq + Hash,
    {
        let mgr = self.cell.read();
        let mut node = self.root;
        loop {
            match mgr.expect_node(node) {
                ManagerNode::Terminal(value) => return *value,
                ManagerNode::Decision { var, low, high } => {
                    let name = mgr
                        .var_name(*var)
                        .expect("decision node variable must have a name");
                    let set = assignment.get(name.as_str()).copied().unwrap_or(false);
                    node = if set { *high } else { *low };
                }
            }
        }
    }

    // ---- Introspection ------------------------------------------------------------------------

    /// The variables this function depends on, sorted alphabetically by name.
    #[must_use]
    pub fn collect_variables(&self) -> Vec<Symbol> {
        let mut ids = std::collections::HashSet::new();
        self.collect_var_ids(&mut ids);
        let mgr = self.cell.read();
        let mut names: Vec<Symbol> = ids
            .into_iter()
            .filter_map(|id| mgr.var_name(id).cloned())
            .collect();
        names.sort();
        names
    }

    /// Number of distinct, reachable nodes in this BDD (including reached terminals).
    #[must_use]
    pub fn node_count(&self) -> usize {
        let mut count = 0;
        self.for_each_reachable_node(|_| count += 1);
        count
    }

    /// Number of distinct variables this function depends on.
    #[must_use]
    pub fn var_count(&self) -> usize {
        let mut ids = std::collections::HashSet::new();
        self.collect_var_ids(&mut ids);
        ids.len()
    }

    /// Collect every variable id reachable from the root, deduplicated on nodes so a variable
    /// labelling several nodes is never missed.
    fn collect_var_ids(&self, vars: &mut std::collections::HashSet<usize>) {
        self.for_each_reachable_node(|node| {
            if let ManagerNode::Decision { var, .. } = node {
                vars.insert(*var);
            }
        });
    }

    /// Visit every distinct node reachable from the root exactly once (iterative DFS, deduplicated on
    /// node id so a shared subgraph is walked once). The shared traversal owns the read guard for the
    /// whole walk (NodeIds are stable), the visited set, the bounds-checked node lookup, and the
    /// child scheduling; `visit` sees each reachable node and decides what to record.
    fn for_each_reachable_node(&self, mut visit: impl FnMut(&ManagerNode)) {
        let mgr = self.cell.read();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![self.root];
        while let Some(node) = stack.pop() {
            if !visited.insert(node) {
                continue;
            }
            let node = mgr.expect_node(node);
            if let ManagerNode::Decision { low, high, .. } = node {
                stack.push(*low);
                stack.push(*high);
            }
            visit(node);
        }
    }

    // ---- Cover / minterm materialisation ------------------------------------------------------

    /// Enumerate the paths to TRUE as a single-output sum-of-products [`Cover`].
    ///
    /// Each root→TRUE path becomes one input cube: a variable on the path is fixed `Some(true)` /
    /// `Some(false)`, a variable off the path is a don't-care (`None`). Variables are carried by name
    /// (`Symbol`); the output side is a single [`Anonymous`] column, asserted by every cube — i.e. the
    /// cover is the **characteristic function** of this BDD over its support variables. The returned cover
    /// is an `F` (ON-set) cover.
    #[must_use]
    pub fn to_cubes(&self) -> Cover<Symbol, Anonymous> {
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

        let mgr = self.cell.read();
        let mut stack = vec![Work::Node(self.root)];
        while let Some(work) = stack.pop() {
            match work {
                Work::SetPath(i, value) => path[i] = value,
                Work::Node(node) => match mgr.expect_node(node) {
                    ManagerNode::Terminal(true) => {
                        let inputs =
                            Minterm::from_symbols(Arc::clone(&symbols), path.iter().copied());
                        let outputs = OutputSet::from_symbols(
                            Arc::clone(&output_symbols),
                            std::iter::once(true),
                        );
                        cubes.push(Cube::new(inputs, outputs, CubeType::F));
                    }
                    ManagerNode::Terminal(false) => {}
                    ManagerNode::Decision { var, low, high } => {
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
    pub fn to_minterms<S: AsRef<str>>(&self, vars: &[S]) -> Vec<Minterm<Symbol>> {
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
    pub fn minimize(&self) -> Result<Cover<Symbol, Anonymous>, crate::error::MinimizationError> {
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
    pub fn to_expr(&self) -> BoolExpr {
        self.to_cubes()
            .to_expr_by_index(0)
            .expect("to_cubes yields a single-output cover, so output index 0 is in bounds")
    }
}

/// Canonical equality: two handles are equal iff they denote the same function (same root within the
/// shared manager). Only handles of the same brand are comparable, which guarantees they share a manager;
/// the manager-pointer assert is a runtime backstop against a brand clash.
impl<B: Brand, C: ManagerCell> PartialEq for Bdd<B, C> {
    fn eq(&self, other: &Self) -> bool {
        self.equivalent_to(other)
    }
}

impl<B: Brand, C: ManagerCell> Eq for Bdd<B, C> {}

/// Shows the canonical root id (the function's identity within its builder) and the manager pointer, so
/// two handles that are `==` print equal roots. The decoded function is not rendered — use
/// [`to_cubes`](Bdd::to_cubes) for that.
impl<B: Brand, C: ManagerCell> std::fmt::Debug for Bdd<B, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bdd")
            .field("root", &self.root)
            .field("mgr", &self.cell.as_ptr())
            .finish()
    }
}

// ---- Operator overloading -------------------------------------------------------------------------
//
// Each operator is implemented for every owned/borrowed combination so `a & b`, `&a & b`, `a & &b`, and
// `&a & &b` all type-check, every combination forwarding to the inherent `&self, &Self` method (so the
// `assert_same_manager` clash check fires whichever combination is used). The shared
// `impl_binary_operator!` macro generates the four impls; the leading group carries the handle's generic
// parameters. Mixing two different builders is a compile error: the operands must share the brand `B` and
// cell `C`, and a mismatch fails to type-check (see the `Bdd` type docs for a `compile_fail` example).

impl_binary_operator!({B: Brand, C: ManagerCell} Bdd<B, C>, BitAnd, bitand, and);
impl_binary_operator!({B: Brand, C: ManagerCell} Bdd<B, C>, BitOr, bitor, or);
impl_binary_operator!({B: Brand, C: ManagerCell} Bdd<B, C>, BitXor, bitxor, xor);

impl<B: Brand, C: ManagerCell> std::ops::Not for Bdd<B, C> {
    type Output = Bdd<B, C>;
    fn not(self) -> Bdd<B, C> {
        Bdd::complement(self)
    }
}

impl<B: Brand, C: ManagerCell> std::ops::Not for &Bdd<B, C> {
    type Output = Bdd<B, C>;
    fn not(self) -> Bdd<B, C> {
        Bdd::complement(self.clone())
    }
}

/// The variables a [`Bdd`] depends on as a `BTreeSet` (a canonical-order convenience over
/// [`collect_variables`](Bdd::collect_variables)).
impl<B: Brand, C: ManagerCell> Bdd<B, C> {
    /// The variables this function depends on, as a `BTreeSet<Symbol>` (canonical iteration order).
    #[must_use]
    pub fn variables(&self) -> BTreeSet<Symbol> {
        self.collect_variables().into_iter().collect()
    }
}

/// One node of a [`Bdd`], as seen by [`Bdd::fold`] and [`Bdd::fold_with_context`].
///
/// A reduced ordered BDD is a directed acyclic graph of Shannon decision nodes over two terminals, so
/// the fold surface mirrors that structure directly: a [`Terminal`](BddNode::Terminal) leaf, or a
/// [`Decision`](BddNode::Decision) testing one `variable` with a `low` (variable = false) and `high`
/// (variable = true) child — **not** the And/Or/Not shape of a syntactic
/// [`BoolExpr`](crate::BoolExpr) (which folds over [`ExprNode`](crate::ExprNode)).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BddNode<'a, T> {
    /// A terminal leaf — the constant `false` or `true`.
    Terminal(bool),
    /// A decision node testing `variable`: take `low` when it is false, `high` when it is true.
    Decision {
        /// The tested variable's name.
        variable: &'a str,
        /// Result for the `variable = false` branch.
        low: T,
        /// Result for the `variable = true` branch.
        high: T,
    },
}

impl<B: Brand, C: ManagerCell> Bdd<B, C> {
    /// Fold the decision diagram bottom-up, combining each node's children results with `f`.
    ///
    /// Walks the BDD **as a BDD**: `f` sees a [`BddNode::Terminal`] for each terminal and a
    /// [`BddNode::Decision`] (carrying its already-folded `low`/`high` results) for each decision node.
    /// Results are memoised per node, so a shared subgraph is folded exactly once — the cost is
    /// O(distinct reachable nodes), keeping the fold bounded by the diagram rather than the number of
    /// paths. The walk is iterative (explicit work-stack), so a tall diagram cannot overflow the call
    /// stack. `T: Clone` because a shared child's folded result is reused by each parent.
    ///
    /// For a fold over a syntactic expression's And/Or/Xor/Not structure, see
    /// [`BoolExpr::fold`](crate::BoolExpr::fold).
    pub fn fold<T, F>(&self, f: F) -> T
    where
        F: Fn(BddNode<'_, T>) -> T + Copy,
        T: Clone,
    {
        enum Work {
            Visit(NodeId),
            Combine(NodeId),
        }
        let mgr = self.cell.read();
        let mut memo: HashMap<NodeId, T> = HashMap::new();
        let mut stack = vec![Work::Visit(self.root)];
        while let Some(work) = stack.pop() {
            match work {
                Work::Visit(node) => {
                    if memo.contains_key(&node) {
                        continue;
                    }
                    match mgr.expect_node(node) {
                        ManagerNode::Terminal(value) => {
                            memo.insert(node, f(BddNode::Terminal(*value)));
                        }
                        // Schedule the combine after both children have been folded (LIFO: push
                        // Combine first so it pops last).
                        ManagerNode::Decision { low, high, .. } => {
                            stack.push(Work::Combine(node));
                            stack.push(Work::Visit(*high));
                            stack.push(Work::Visit(*low));
                        }
                    }
                }
                Work::Combine(node) => {
                    // A shared node can be scheduled to combine more than once; the first wins.
                    if memo.contains_key(&node) {
                        continue;
                    }
                    let (var, low, high) = match mgr.get_node(node) {
                        Some(ManagerNode::Decision { var, low, high }) => (*var, *low, *high),
                        _ => unreachable!("combine is scheduled only for decision nodes"),
                    };
                    let name = mgr
                        .var_name(var)
                        .expect("decision node variable must have a name");
                    let low_t = memo
                        .get(&low)
                        .cloned()
                        .expect("low child folded before combine");
                    let high_t = memo
                        .get(&high)
                        .cloned()
                        .expect("high child folded before combine");
                    let result = f(BddNode::Decision {
                        variable: name.as_str(),
                        low: low_t,
                        high: high_t,
                    });
                    memo.insert(node, result);
                }
            }
        }
        memo.remove(&self.root)
            .expect("root node folded after the walk")
    }

    /// Fold the decision diagram with a context that flows **top-down**, mirroring
    /// [`BoolExpr::fold_with_context`](crate::BoolExpr::fold_with_context) over the BDD structure.
    ///
    /// - **`descend`** runs on the way *down*. Given a decision node's shape ([`BddNode<()>`], carrying
    ///   the tested `variable`) and its own context, it returns the `(low, high)` contexts to hand to
    ///   that node's two children. It is never called on a terminal.
    /// - **`combine`** runs on the way *back up*. Given a node whose children already hold their folded
    ///   results ([`BddNode<T>`]) plus that node's own context, it produces this node's result.
    ///
    /// Because the context can differ along each path, results are **not** memoised: the diagram is
    /// unfolded into a tree, so the cost can be exponential in the diagram size in the worst case (use
    /// [`fold`](Self::fold) when no top-down context is needed). The walk is iterative, so depth alone
    /// cannot overflow the call stack.
    ///
    /// [`BddNode<()>`]: BddNode
    /// [`BddNode<T>`]: BddNode
    pub fn fold_with_context<Ctx, T, D, G>(&self, root_context: Ctx, descend: D, combine: G) -> T
    where
        D: Fn(BddNode<'_, ()>, &Ctx) -> (Ctx, Ctx),
        G: Fn(BddNode<'_, T>, Ctx) -> T,
    {
        enum Work<Ctx> {
            Enter(NodeId, Ctx),
            Combine(NodeId, Ctx),
        }
        let mgr = self.cell.read();
        let mut work = vec![Work::Enter(self.root, root_context)];
        let mut results: Vec<T> = Vec::new();
        while let Some(frame) = work.pop() {
            match frame {
                Work::Enter(node, ctx) => match mgr.expect_node(node) {
                    ManagerNode::Terminal(value) => {
                        results.push(combine(BddNode::Terminal(*value), ctx));
                    }
                    ManagerNode::Decision { var, low, high } => {
                        let name = mgr
                            .var_name(*var)
                            .expect("decision node variable must have a name");
                        let (low_ctx, high_ctx) = descend(
                            BddNode::Decision {
                                variable: name.as_str(),
                                low: (),
                                high: (),
                            },
                            &ctx,
                        );
                        work.push(Work::Combine(node, ctx));
                        work.push(Work::Enter(*high, high_ctx));
                        work.push(Work::Enter(*low, low_ctx));
                    }
                },
                Work::Combine(node, ctx) => {
                    let var = match mgr.get_node(node) {
                        Some(ManagerNode::Decision { var, .. }) => *var,
                        _ => unreachable!("combine is scheduled only for decision nodes"),
                    };
                    let name = mgr
                        .var_name(var)
                        .expect("decision node variable must have a name");
                    let high = results.pop().expect("high child result");
                    let low = results.pop().expect("low child result");
                    results.push(combine(
                        BddNode::Decision {
                            variable: name.as_str(),
                            low,
                            high,
                        },
                        ctx,
                    ));
                }
            }
        }
        results.pop().expect("fold_with_context produced a result")
    }
}
