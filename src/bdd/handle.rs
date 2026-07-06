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

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use super::brand::Brand;
use super::builder::BddBuilder;
use crate::cover::{
    Anonymous, Cover, CoverType, Cube, CubeType, Minterm, OutputSet, StringLabel, Symbols,
};
use crate::bdd::manager::{
    BddManager, BddNode as ManagerNode, NodeId, FALSE_NODE, TRUE_NODE,
};
use crate::bdd::manager_cell::ManagerCell;
use crate::expression::BoolExpr;
use crate::impl_binary_operator;
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

    /// The canonical root node this handle denotes. Crate-internal: only sibling BDD modules (e.g. the
    /// scoped builder's [`lift`](super::scope::Scope::lift)) read it.
    pub(super) fn root(&self) -> NodeId {
        self.root
    }

    /// This handle's storage cell. Crate-internal: only sibling BDD modules read it (e.g. the scoped
    /// builder's [`lift`](super::scope::Scope::lift), to assert same-manager identity).
    pub(super) fn cell(&self) -> &C {
        &self.cell
    }

    /// Recover a [`BddBuilder`] onto this handle's manager.
    ///
    /// The returned builder shares this handle's manager (a refcounted clone of the same cell) and its
    /// brand, so handles it mints combine freely with `self`. This lets a stored `Bdd` outlive its
    /// original builder yet still seed further construction in the same namespace.
    ///
    /// ```
    /// use espresso_logic::bdd_builder;
    ///
    /// // Build a handle, then drop the builder that made it.
    /// let a = {
    ///     let builder = bdd_builder!();
    ///     builder.var("a")
    /// };
    ///
    /// // Recover a builder onto the same manager and derive more handles.
    /// let builder = a.builder();
    /// let b = builder.var("b");
    ///
    /// // Handles from the recovered builder combine with the stored one.
    /// let f = &a & &b;
    /// assert!(f.equivalent_to(&(builder.parse("a & b").unwrap())));
    /// ```
    #[must_use]
    pub fn builder(&self) -> BddBuilder<B, C> {
        BddBuilder::from_cell(&self.cell)
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
        let root = super::encoding::and(&self.cell, self.root, other.root);
        Self::from_root(&self.cell, root)
    }

    /// Logical OR of two handles: `self ∨ other`. Equivalent to the `|` operator.
    #[must_use]
    pub fn or(&self, other: &Self) -> Self {
        self.assert_same_manager(other);
        let root = super::encoding::or(&self.cell, self.root, other.root);
        Self::from_root(&self.cell, root)
    }

    /// Logical XOR of two handles: `self ⊕ other`. Equivalent to the `^` operator.
    #[must_use]
    pub fn xor(&self, other: &Self) -> Self {
        self.assert_same_manager(other);
        let root = super::encoding::xor(&self.cell, self.root, other.root);
        Self::from_root(&self.cell, root)
    }

    /// Logical negation: `¬self`. A handle to the complement of this function.
    ///
    /// [`not`](Self::not) is an alias of this method, and the unary `!` operator is equivalent (it
    /// delegates here). Negation is offered under both names because `complement` reads naturally in a
    /// method chain while `!` reads naturally in an expression.
    #[must_use]
    pub fn complement(&self) -> Self {
        let root = super::encoding::not(&self.cell, self.root);
        Self::from_root(&self.cell, root)
    }

    /// Logical negation: `¬self`. An alias of [`complement`](Self::complement); the unary `!`
    /// operator is equivalent.
    #[must_use]
    pub fn not(&self) -> Self {
        self.complement()
    }

    /// If-then-else: `if self then g else h`. The fundamental BDD operation the others derive from.
    #[must_use]
    pub fn ite(&self, g: &Self, h: &Self) -> Self {
        self.assert_same_manager(g);
        self.assert_same_manager(h);
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
        Self::from_root(
            &self.cell,
            super::encoding::restrict(&self.cell, self.root, var.as_ref(), value),
        )
    }

    /// Shannon cofactor by assignment — an alias of [`restrict`](Self::restrict).
    ///
    /// In this BDD modelling the cofactor *by a single assignment* and `restrict` are the same operation,
    /// so `cofactor` is provided as the conventional name; both substitute `var := value` and reduce. For a
    /// multi-variable cofactor, use [`restrict_many`](Self::restrict_many) (or chain `restrict`/`cofactor`
    /// calls; or use [`forall`](Self::forall) / [`exists`](Self::exists) to quantify).
    #[must_use]
    pub fn cofactor<S: AsRef<str>>(&self, var: S, value: bool) -> Self {
        self.restrict(var, value)
    }

    /// Simultaneous multi-variable Shannon cofactor: `self|{v1=b1, v2=b2, …}` in a single pass.
    ///
    /// Fixes every named variable to its constant at once, equal to chaining
    /// [`restrict`](Self::restrict) over the same assignments in **any order** but without re-walking
    /// the diagram per variable. A name repeated in `assignment` takes its **last** entry; a name
    /// absent from `assignment` (or from `self`'s variables) is left free; an empty `assignment` is a
    /// no-op.
    ///
    /// ```
    /// use espresso_logic::bdd_builder;
    ///
    /// let builder = bdd_builder!();
    /// let a = builder.var("a");
    /// let b = builder.var("b");
    /// let c = builder.var("c");
    ///
    /// // f = a & b | !a & c; fixing a=1, b=0 leaves the a & b term false, so f reduces to false.
    /// let f = (&a & &b) | (!&a & &c);
    /// let result = f.restrict_many([("a", true), ("b", false)]);
    /// assert!(result.is_contradiction());
    /// ```
    #[must_use]
    pub fn restrict_many<S: AsRef<str>>(
        &self,
        assignment: impl IntoIterator<Item = (S, bool)>,
    ) -> Self {
        Self::from_root(
            &self.cell,
            super::encoding::restrict_many(&self.cell, self.root, assignment),
        )
    }

    /// Universal quantification over `vars`: `∀v∈vars. self`.
    ///
    /// Folds `restrict(v, true) & restrict(v, false)` across each variable in `vars`. A name absent from
    /// the function contributes a no-op cofactor (`self & self == self`). Quantifying over no variables
    /// returns `self`.
    ///
    /// `vars` accepts anything iterable of `AsRef<str>` — a slice reference (`f.forall(&["a", "b"])`), an
    /// owned `Vec<String>`, or an adaptor chain (`names.iter().filter(..)`) — not just borrowed slices.
    #[must_use]
    pub fn forall<S: AsRef<str>>(&self, vars: impl IntoIterator<Item = S>) -> Self {
        self.quantify(vars, Self::and)
    }

    /// Existential quantification over `vars`: `∃v∈vars. self`.
    ///
    /// Folds `restrict(v, true) | restrict(v, false)` across each variable in `vars`. A name absent from
    /// the function contributes a no-op cofactor (`self | self == self`). Quantifying over no variables
    /// returns `self`.
    ///
    /// `vars` accepts anything iterable of `AsRef<str>` — a slice reference (`f.exists(&["a", "b"])`), an
    /// owned `Vec<String>`, or an adaptor chain (`names.iter().filter(..)`) — not just borrowed slices.
    #[must_use]
    pub fn exists<S: AsRef<str>>(&self, vars: impl IntoIterator<Item = S>) -> Self {
        self.quantify(vars, Self::or)
    }

    /// Quantify over `vars`, folding `combine` across each variable's two cofactors. Universal and
    /// existential quantification differ only in `combine` (`and` vs `or`); this is the shared body.
    fn quantify<S: AsRef<str>>(
        &self,
        vars: impl IntoIterator<Item = S>,
        combine: fn(&Self, &Self) -> Self,
    ) -> Self {
        let mut acc = self.clone();
        for v in vars {
            let lo = acc.restrict(v.as_ref(), false);
            let hi = acc.restrict(v.as_ref(), true);
            acc = combine(&hi, &lo);
        }
        acc
    }

    // ---- Composition ----------------------------------------------------------------------------

    /// Substitutes the function `g` for the variable `var`: `self[var := g]`.
    ///
    /// A name that is **not** a variable of this function leaves it unchanged (a no-op). `g` may
    /// mention any variable, including `var` itself or a variable ordered above `var` in the
    /// diagram — substitution does not require `g` to sit "below" the point it is spliced into.
    ///
    /// ```
    /// use espresso_logic::bdd_builder;
    ///
    /// let builder = bdd_builder!();
    /// let a = builder.var("a");
    /// let b = builder.var("b");
    /// let c = builder.var("c");
    ///
    /// // f = a & !b; substitute b := (b | c).
    /// let f = &a & !&b;
    /// let g = &b | &c;
    /// let result = f.compose("b", &g);
    ///
    /// let expected = &a & !(&b | &c);
    /// assert!(result.equivalent_to(&expected));
    /// ```
    #[must_use]
    pub fn compose<S: AsRef<str>>(&self, var: S, g: &Self) -> Self {
        self.assert_same_manager(g);
        let root = super::encoding::compose(&self.cell, self.root, var.as_ref(), g.root);
        Self::from_root(&self.cell, root)
    }

    /// Simultaneous multi-variable substitution: `self[v1 := g1, v2 := g2, ...]` in a single pass.
    ///
    /// Every substitution reads `self` as it was **before** any substitution, not a
    /// partially-substituted intermediate, so a swap map exchanging two variables needs no
    /// temporary. A name repeated in `map` takes its **last** entry; a name absent from `map` (or
    /// from `self`'s variables) is left alone; an empty `map` is a no-op.
    ///
    /// ```
    /// use espresso_logic::bdd_builder;
    ///
    /// let builder = bdd_builder!();
    /// let a = builder.var("a");
    /// let b = builder.var("b");
    ///
    /// // f = a & !b; swapping a and b gives b & !a.
    /// let f = &a & !&b;
    /// let result = f.compose_map([("a", &b), ("b", &a)]);
    ///
    /// let expected = &b & !&a;
    /// assert!(result.equivalent_to(&expected));
    /// ```
    #[must_use]
    pub fn compose_map<'a, S: AsRef<str>>(
        &self,
        map: impl IntoIterator<Item = (S, &'a Self)>,
    ) -> Self
    where
        B: 'a,
        C: 'a,
    {
        let entries: Vec<(S, NodeId)> = map
            .into_iter()
            .map(|(name, g)| {
                self.assert_same_manager(g);
                (name, g.root)
            })
            .collect();
        let root = super::encoding::compose_map(&self.cell, self.root, entries);
        Self::from_root(&self.cell, root)
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

    /// Probe whether the variables `assignment` **fixes** already determine this function, without
    /// interning a single node.
    ///
    /// Returns `Some(b)` iff restricting `self` by the fixed variables yields the constant `b`, and
    /// `None` iff the function still depends on a free variable. A complete assignment over the
    /// support therefore always yields `Some`; a genuinely partial one that leaves a live dependence
    /// yields `None`. Fields that are don't-care/empty — or names the function does not test — leave
    /// their variable free.
    ///
    /// This is a **write-free** fast path: it [`fold`](Self::fold)s the diagram over the lattice
    /// `Option<bool>`, snapshotting the structure under one read guard and walking it guard-free, so
    /// it never takes the manager's write lock and interns nothing. Its result is exactly the
    /// `Ok`-arm of [`evaluate`](Self::evaluate) (which falls back to building the residual only when
    /// this returns `None`).
    ///
    /// The fold is correct by induction on the diagram: a terminal folds to its own constant; a node
    /// testing a **fixed** variable inherits the folded result of the chosen child; a node testing a
    /// **free** variable folds to a constant iff both children folded to the *same* `Some`, and to
    /// `None` otherwise — a difference between the branches is a genuine dependence on that free
    /// variable.
    ///
    /// The label type may be any [`StringLabel`](crate::StringLabel) (`String`,
    /// [`Symbol`](crate::Symbol), `Arc<str>`, …).
    ///
    /// ```
    /// use espresso_logic::{bdd_builder, Minterm, Symbol};
    ///
    /// let builder = bdd_builder!();
    /// let a = builder.var("a");
    /// let b = builder.var("b");
    /// let c = builder.var("c");
    /// let f = (&a & &b) | (!&a & &c); // a & b | !a & c
    ///
    /// // Complete over the support: determined.
    /// let m = Minterm::<Symbol>::with_labels(&[("a", Some(true)), ("b", Some(false)), ("c", Some(true))]).unwrap();
    /// assert_eq!(f.evaluate_fast(&m), Some(false));
    ///
    /// // Partial but still collapsing: a is free, but b=1 and c=1 make both branches true.
    /// let m = Minterm::<Symbol>::with_labels(&[("b", Some(true)), ("c", Some(true))]).unwrap();
    /// assert_eq!(f.evaluate_fast(&m), Some(true));
    ///
    /// // Genuinely partial: fixing only a leaves a live dependence (on b or c).
    /// let m = Minterm::<Symbol>::with_labels(&[("a", Some(true))]).unwrap();
    /// assert_eq!(f.evaluate_fast(&m), None);
    /// ```
    #[must_use]
    pub fn evaluate_fast<L: StringLabel>(&self, assignment: &Minterm<L>) -> Option<bool> {
        // The four-state Empty field folds to don't-care via the value view, leaving that variable
        // free — the same view the restrict-based `evaluate` uses.
        let fixed: HashMap<&str, bool> = assignment
            .vars()
            .iter()
            .zip(assignment.iter())
            .filter_map(|(label, value)| value.map(|v| (label.as_ref(), v)))
            .collect();
        self.fold(|node| match node {
            BddNode::Terminal(b) => Some(b),
            BddNode::Decision {
                variable,
                low,
                high,
            } => match fixed.get(variable) {
                Some(&true) => high,
                Some(&false) => low,
                None => {
                    if low == high {
                        low
                    } else {
                        None
                    }
                }
            },
        })
    }

    /// Evaluate this function under a (possibly partial) variable `assignment`, given as a [`Minterm`].
    ///
    /// A write-free [`evaluate_fast`](Self::evaluate_fast) probe decides the determined case; only when
    /// free variables remain is the residual materialised via [`restrict_many`](Self::restrict_many).
    /// Every variable the minterm **fixes** (a concrete `1`/`0` field) is fixed to its constant; a
    /// don't-care (`-`) field — or a variable the minterm does not carry — leaves that variable **free**,
    /// and a name the function does not depend on is ignored. There is no silent default: a variable
    /// absent from the assignment is treated as *unassigned*, never as `false`.
    ///
    /// The result reflects whether the fixed variables already determine the function:
    ///
    /// - `Ok(true)` / `Ok(false)` when the restricted function is constant — a complete assignment over
    ///   the support therefore always yields `Ok`.
    /// - `Err(residual)` when variables the function still depends on remain free; the residual [`Bdd`]
    ///   is the function over those free variables, owned and usable for further evaluation.
    ///
    /// The label type may be any [`StringLabel`](crate::StringLabel) (`String`,
    /// [`Symbol`](crate::Symbol), `Arc<str>`, …).
    ///
    /// Evaluation is a semantic operation, so it lives here rather than on the syntactic
    /// [`BoolExpr`](crate::BoolExpr): build the expression into a builder with
    /// [`BddBuilder::build`](crate::bdd::BddBuilder::build) first.
    pub fn evaluate<L: StringLabel>(&self, assignment: &Minterm<L>) -> Result<bool, Bdd<B, C>> {
        // Write-free fast path: if the fixed variables already determine the function, we're done
        // without interning a single node.
        if let Some(value) = self.evaluate_fast(assignment) {
            return Ok(value);
        }
        // Otherwise materialise the residual over the still-free variables in one restrict pass.
        // Restricting a name absent from the function is a no-op; don't-care/empty fields stay free.
        let residual = self.restrict_many(
            assignment
                .vars()
                .iter()
                .zip(assignment.iter())
                .filter_map(|(label, value)| value.map(|v| (label.as_ref(), v))),
        );
        debug_assert!(
            !residual.is_tautology() && !residual.is_contradiction(),
            "evaluate_fast returned None for a determined assignment - BDD evaluation bug"
        );
        Err(residual)
    }

    // ---- Introspection ------------------------------------------------------------------------

    /// The variables this function depends on, as a lazy [`BddVariables`] iterator.
    ///
    /// Yields each support variable once (deduplicated), in the order the shared graph traversal first
    /// encounters it — **not** sorted. The iterator borrows this handle and walks the shared graph
    /// incrementally: each `next()` takes a brief read guard and advances the depth-first traversal only
    /// far enough to surface the next new variable, so callers that stop early (`.next()`, `.any(..)`,
    /// `.take(n)`) never pay for the whole-graph walk.
    #[must_use]
    pub fn variables(&self) -> BddVariables<'_, B, C> {
        // O(1) construction: seed the DFS frontier with the root and let `next()` do the walking.
        BddVariables {
            bdd: self,
            stack: vec![self.root],
            visited: std::collections::HashSet::new(),
            seen_vars: std::collections::HashSet::new(),
        }
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
    ///
    /// No-reentrancy invariant: `mgr` (the manager read guard) is held for the entire walk below, so
    /// `visit` must never touch this handle's manager — no builder call, no other `Bdd` method on a
    /// handle sharing this cell, nothing that acquires the guard again. Doing so deadlocks the `RwLock`
    /// (`SyncCell`) or panics the `RefCell` (`LocalCell`). Every call site in this module passes a plain
    /// closure that only inspects the `&ManagerNode` it is given — keep it that way.
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
    pub fn cover(&self) -> Cover<Symbol, Anonymous> {
        self.extract_cubes(CoverType::F)
    }

    /// Renamed to [`cover`](Self::cover).
    #[deprecated(since = "5.2.0", note = "renamed to `cover`")]
    #[must_use]
    pub fn to_cubes(&self) -> Cover<Symbol, Anonymous> {
        self.cover()
    }

    /// Enumerate the paths to TRUE **and** FALSE as a two-sided `FR` [`Cover`].
    ///
    /// Every root→TRUE path becomes an on-set cube ([`CubeType::F`]) and every root→FALSE path an
    /// off-set cube ([`CubeType::R`]); a variable on a path is fixed `Some(true)` / `Some(false)`, one
    /// off the path is a don't-care (`None`). The two sides partition the minterm space — a BDD path
    /// reaches exactly one terminal, so the on-set and off-set are disjoint and jointly exhaustive.
    /// Variables are carried by name (`Symbol`) over a single asserted [`Anonymous`] output column, as
    /// in [`cover`](Self::cover). Feeding the result to [`minimize_fr`](Self::minimize_fr) minimises
    /// the on-set against this exact off-set rather than a recomputed complement.
    #[must_use]
    pub fn cover_fr(&self) -> Cover<Symbol, Anonymous> {
        self.extract_cubes(CoverType::FR)
    }

    /// Shared DFS backing [`cover`](Self::cover) and [`cover_fr`](Self::cover_fr).
    ///
    /// Walks every root→terminal path once under a single read guard, emitting an `F` cube per TRUE
    /// path and — only when `cover_type` carries an off-set — an `R` cube per FALSE path. The returned
    /// cover carries `cover_type` verbatim.
    fn extract_cubes(&self, cover_type: CoverType) -> Cover<Symbol, Anonymous> {
        // Canonical, alphabetically sorted header shared by every extracted cube. `variables()` yields
        // the support in traversal (unsorted) order, so sort here to keep the header canonical.
        let mut names: Vec<Symbol> = self.variables().collect();
        names.sort();
        let vars: Arc<[Symbol]> = names.into();
        let index: std::collections::HashMap<Symbol, usize> = vars
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, v)| (v, i))
            .collect();
        // A BDD's support variables are distinct, so the header cannot carry a duplicate.
        let symbols = Symbols::new(vars).expect("BDD support variables are distinct");
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

        // No-reentrancy invariant: `mgr` is held across the whole walk below (dropped explicitly once the
        // stack empties), so nothing in this loop may re-acquire this handle's manager — no builder call,
        // no other `Bdd` method on a handle sharing this cell. Doing so deadlocks the `RwLock`
        // (`SyncCell`) or panics the `RefCell` (`LocalCell`). The loop body below only reads through `mgr`
        // and pushes onto local `stack`/`path` state — keep it that way.
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
                    ManagerNode::Terminal(false) => {
                        if cover_type.has_r() {
                            let inputs =
                                Minterm::from_symbols(Arc::clone(&symbols), path.iter().copied());
                            let outputs = OutputSet::from_symbols(
                                Arc::clone(&output_symbols),
                                std::iter::once(true),
                            );
                            cubes.push(Cube::new(inputs, outputs, CubeType::R));
                        }
                    }
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

        // Build the cover from the explicit header rather than re-deriving it from the cubes: a
        // contradiction yields zero cubes, and re-deriving from those would give a zero-output header,
        // breaking the later `to_expr_by_index(0)`. `from_parts` keeps the arity-1 `Anonymous` output
        // header, so a contradiction lowers to a one-output, zero-cube cover that renders as "0".
        Cover::from_parts(symbols, output_symbols, cubes, cover_type)
    }

    /// The complete set of prime implicants of this function, as a single-output `F` [`Cover`].
    ///
    /// Every prime implicant — not the reduced, irredundant cover [`minimize`](Self::minimize) yields.
    /// Equivalent to `self.cover().primes()`; see [`Cover::primes`].
    #[must_use]
    pub fn primes(&self) -> Cover<Symbol, Anonymous> {
        self.cover().primes()
    }

    /// Re-base this function's ON-set onto exactly `vars`, universally projecting away any dropped
    /// variable.
    ///
    /// Equivalent to `self.cover().over_vars(vars)`. A variable of `vars` absent from the function is
    /// widened in as a don't-care; a support variable not in `vars` is eliminated by **universal**
    /// projection, so the result holds exactly the `vars` assignments that force the output high for
    /// every value of the eliminated variables. The cover is returned in don't-care form — compose
    /// [`Cover::maximize`] for minterms. See [`Cover::over_vars`] for the full semantics.
    ///
    /// `vars` names a variable *set*: a repeated name is deduplicated (see [`Cover::over_vars`]).
    #[must_use]
    pub fn cover_over<S: AsRef<str>>(
        &self,
        vars: impl IntoIterator<Item = S>,
    ) -> Cover<Symbol, Anonymous> {
        self.cover().over_vars(vars)
    }

    /// Re-base this function onto exactly `vars` as a two-sided `FR` [`Cover`], universally projecting
    /// away any dropped variable.
    ///
    /// Equivalent to `self.cover_fr().over_vars(vars)`. The on-set ([`CubeType::F`]) holds the `vars`
    /// assignments that force the output high for every value of the eliminated variables, the off-set
    /// ([`CubeType::R`]) those that force it low. Because each side is derived independently, they are
    /// **orthogonal but not necessarily complementary**: where the output still depends on an
    /// eliminated variable, that assignment lands in neither side (a genuine undef gap — the Muller
    /// C-element case). See [`Cover::over_vars`].
    ///
    /// `vars` names a variable *set*: a repeated name is deduplicated (see [`Cover::over_vars`]).
    #[must_use]
    pub fn cover_over_fr<S: AsRef<str>>(
        &self,
        vars: impl IntoIterator<Item = S>,
    ) -> Cover<Symbol, Anonymous> {
        self.cover_fr().over_vars(vars)
    }

    /// The **maximal** cover of this function — the inverse of [`minimize`](Self::minimize).
    ///
    /// Every cube of the returned [`Cover`] assigns **every** support variable (no don't-cares left),
    /// so each cube is a full minterm; enumerate them with [`Cover::cubes`] (each cube's
    /// [`inputs`](crate::Cube::inputs) is the [`Minterm`]). The result is **deduplicated** (first-seen
    /// order, **not** sorted) and shares one canonical, identity-aligned header. To re-base onto a
    /// different variable set (widening or universally projecting), use [`cover_over`](Self::cover_over)
    /// first.
    ///
    /// Equivalent to `self.cover().maximize()`.
    #[must_use]
    pub fn maximize(&self) -> Cover<Symbol, Anonymous> {
        self.cover().maximize()
    }

    /// The **maximal** two-sided `FR` cover of this function — the [`maximize`](Self::maximize)
    /// counterpart for the on+off-set extraction of [`cover_fr`](Self::cover_fr).
    ///
    /// Both the on-set ([`CubeType::F`]) and off-set ([`CubeType::R`]) cubes are expanded so that every
    /// cube assigns every support variable, i.e. each cube is a full minterm. The two sides partition
    /// the minterm space over the support.
    ///
    /// Equivalent to `self.cover_fr().maximize()`.
    #[must_use]
    pub fn maximize_fr(&self) -> Cover<Symbol, Anonymous> {
        self.cover_fr().maximize()
    }

    // ---- Minimisation -------------------------------------------------------------------------

    /// Minimise this function's ON-set with Espresso, returning the minimised single-output
    /// [`Cover`].
    ///
    /// Equivalent to `self.cover().minimize()`; the cover is the characteristic function over the
    /// support variables (see [`cover`](Self::cover)).
    ///
    /// # Errors
    ///
    /// Propagates any [`MinimizationError`](crate::error::MinimizationError) from the Espresso engine.
    pub fn minimize(&self) -> Result<Cover<Symbol, Anonymous>, crate::error::MinimizationError> {
        use crate::Minimizable;
        self.cover().minimize()
    }

    /// Minimise this function's ON-set with Espresso against its **exact** off-set, returning an `FR`
    /// [`Cover`] whose minimised on-set is paired with the off-set.
    ///
    /// Equivalent to `self.cover_fr().minimize()`. Unlike [`minimize`](Self::minimize), which lets
    /// Espresso derive the OFF-set from the ON-set complement, this supplies the off-set directly from
    /// the BDD's root→FALSE paths (see [`cover_fr`](Self::cover_fr)). Only the ON-set is minimised; the
    /// off-set is returned as the enumerated path cubes, not separately reduced.
    ///
    /// # Errors
    ///
    /// Propagates any [`MinimizationError`](crate::error::MinimizationError) from the Espresso engine.
    pub fn minimize_fr(&self) -> Result<Cover<Symbol, Anonymous>, crate::error::MinimizationError> {
        use crate::Minimizable;
        self.cover_fr().minimize()
    }

    // ---- Lowering back to a syntactic expression ----------------------------------------------

    /// Lower this function to an owned, factored [`BoolExpr`].
    ///
    /// Enumerates the function's ON-set with [`cover`](Self::cover), then applies algebraic
    /// **direct factorisation** to that cube set (the same path as [`Cover::to_expr`]) to produce a
    /// compact multi-level expression — it does **not** re-canonicalise through the BDD. The resulting
    /// `BoolExpr` is syntactic; building it back here yields the same canonical handle, but its token
    /// structure reflects the factored cubes, not this BDD's node graph.
    #[must_use]
    pub fn to_expr(&self) -> BoolExpr {
        self.cover()
            .to_expr_by_index(0)
            .expect("cover yields a single-output cover, so output index 0 is in bounds")
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

/// Agrees with [`PartialEq`]: hashes the manager identity (the cell's pointer address) together with the
/// canonical root id, so `==` handles always land in the same bucket. Implemented by hand (rather than
/// `#[derive(Hash)]`) so it does not require `B: Hash` or `C: Hash` bounds, matching the manual `PartialEq`.
impl<B: Brand, C: ManagerCell> std::hash::Hash for Bdd<B, C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.cell.as_ptr().hash(state);
        self.root.hash(state);
    }
}

/// Shows the canonical root id (the function's identity within its builder) and the manager pointer, so
/// two handles that are `==` print equal roots. The decoded function is not rendered — use
/// [`cover`](Bdd::cover) for that.
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
        Bdd::complement(&self)
    }
}

impl<B: Brand, C: ManagerCell> std::ops::Not for &Bdd<B, C> {
    type Output = Bdd<B, C>;
    fn not(self) -> Bdd<B, C> {
        Bdd::complement(self)
    }
}

/// Lazy iterator over the support variables of a [`Bdd`], created by [`Bdd::variables`].
///
/// Borrows its [`Bdd`] and walks the shared graph incrementally: each `next()` takes a brief read guard
/// and continues a deduplicated depth-first traversal only until it reaches the next support variable,
/// which it resolves and yields (first-encounter order, **not** sorted). Nothing is materialised up
/// front, so a caller that stops early skips the rest of the walk. Because finishing the walk is the
/// only way to know the count, this is not an [`ExactSizeIterator`].
pub struct BddVariables<'a, B: Brand, C: ManagerCell> {
    /// The borrowed parent; supplies the manager cell and root, and ties the walk to its lifetime.
    bdd: &'a Bdd<B, C>,
    /// DFS frontier of nodes still to visit (seeded with the root).
    stack: Vec<NodeId>,
    /// Nodes already popped, so a shared subgraph is walked once.
    visited: std::collections::HashSet<NodeId>,
    /// Variable ids already yielded, so each support variable surfaces exactly once.
    seen_vars: std::collections::HashSet<usize>,
}

/// Opaque: the borrowed graph carries no useful `Debug`, and the remaining count is unknown without
/// finishing the walk.
impl<B: Brand, C: ManagerCell> std::fmt::Debug for BddVariables<'_, B, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BddVariables").finish_non_exhaustive()
    }
}

impl<B: Brand, C: ManagerCell> Iterator for BddVariables<'_, B, C> {
    type Item = Symbol;

    fn next(&mut self) -> Option<Symbol> {
        // Continue the deduplicated DFS under a brief read guard, stopping at the first not-yet-seen
        // decision variable. `visited`/`seen_vars` persist across calls, so a full drain performs one
        // whole-graph walk in total, and an early-stopping caller performs only part of it.
        let mgr = self.bdd.cell.read();
        while let Some(node) = self.stack.pop() {
            if !self.visited.insert(node) {
                continue;
            }
            if let ManagerNode::Decision { var, low, high } = mgr.expect_node(node) {
                let (var, low, high) = (*var, *low, *high);
                self.stack.push(low);
                self.stack.push(high);
                if self.seen_vars.insert(var) {
                    // An unnamed decision var id should not occur; skip it rather than end the walk.
                    if let Some(name) = mgr.var_name(var).cloned() {
                        return Some(name);
                    }
                }
            }
        }
        None
    }
}

impl<B: Brand, C: ManagerCell> std::iter::FusedIterator for BddVariables<'_, B, C> {}

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
    /// The diagram's structure is snapshotted under a single read guard which is released **before** any
    /// call to `f`, so `f` may re-enter the builder (any operation that locks the same cell) without
    /// double-borrowing the `RefCell` or deadlocking the `RwLock`.
    ///
    /// For a fold over a syntactic expression's And/Or/Xor/Not structure, see
    /// [`BoolExpr::fold`](crate::BoolExpr::fold).
    pub fn fold<T, F>(&self, f: F) -> T
    where
        F: Fn(BddNode<'_, T>) -> T + Copy,
        T: Clone,
    {
        // Snapshot the reachable structure under one read guard, then run the user fold entirely
        // guard-free over the snapshot.
        let snapshot = self.snapshot_reachable();

        enum Work {
            Visit(NodeId),
            Combine(NodeId),
        }
        let mut memo: HashMap<NodeId, T> = HashMap::new();
        let mut stack = vec![Work::Visit(self.root)];
        while let Some(work) = stack.pop() {
            match work {
                Work::Visit(node) => {
                    if memo.contains_key(&node) {
                        continue;
                    }
                    match &snapshot[&node] {
                        SnapNode::Terminal(value) => {
                            memo.insert(node, f(BddNode::Terminal(*value)));
                        }
                        // Schedule the combine after both children have been folded (LIFO: push
                        // Combine first so it pops last).
                        SnapNode::Decision { low, high, .. } => {
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
                    let SnapNode::Decision {
                        variable,
                        low,
                        high,
                    } = &snapshot[&node]
                    else {
                        unreachable!("combine is scheduled only for decision nodes")
                    };
                    let low_t = memo
                        .get(low)
                        .cloned()
                        .expect("low child folded before combine");
                    let high_t = memo
                        .get(high)
                        .cloned()
                        .expect("high child folded before combine");
                    let result = f(BddNode::Decision {
                        variable: variable.as_str(),
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
    /// The diagram's structure is snapshotted under a single read guard which is released **before** any
    /// call to `descend`/`combine`, so those closures may re-enter the builder without double-borrowing
    /// the `RefCell` or deadlocking the `RwLock`.
    ///
    /// [`BddNode<()>`]: BddNode
    /// [`BddNode<T>`]: BddNode
    pub fn fold_with_context<Ctx, T, D, G>(&self, root_context: Ctx, descend: D, combine: G) -> T
    where
        D: Fn(BddNode<'_, ()>, &Ctx) -> (Ctx, Ctx),
        G: Fn(BddNode<'_, T>, Ctx) -> T,
    {
        // Snapshot the reachable structure under one read guard, then run the user closures entirely
        // guard-free over the snapshot.
        let snapshot = self.snapshot_reachable();

        enum Work<Ctx> {
            Enter(NodeId, Ctx),
            Combine(NodeId, Ctx),
        }
        let mut work = vec![Work::Enter(self.root, root_context)];
        let mut results: Vec<T> = Vec::new();
        while let Some(frame) = work.pop() {
            match frame {
                Work::Enter(node, ctx) => match &snapshot[&node] {
                    SnapNode::Terminal(value) => {
                        results.push(combine(BddNode::Terminal(*value), ctx));
                    }
                    SnapNode::Decision {
                        variable,
                        low,
                        high,
                    } => {
                        let (low_ctx, high_ctx) = descend(
                            BddNode::Decision {
                                variable: variable.as_str(),
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
                    let SnapNode::Decision { variable, .. } = &snapshot[&node] else {
                        unreachable!("combine is scheduled only for decision nodes")
                    };
                    let high = results.pop().expect("high child result");
                    let low = results.pop().expect("low child result");
                    results.push(combine(
                        BddNode::Decision {
                            variable: variable.as_str(),
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

    /// Snapshot every node reachable from the root under a **single** read guard, returning an owned
    /// per-node shape map (terminal value, or tested variable name plus `low`/`high` child ids). The
    /// guard is released when this returns, so the [`fold`](Self::fold) /
    /// [`fold_with_context`](Self::fold_with_context) walks run their user closures guard-free and may
    /// re-enter the builder. Node ids are stable, so the snapshotted ids stay valid after the guard
    /// drops.
    fn snapshot_reachable(&self) -> HashMap<NodeId, SnapNode> {
        let mgr = self.cell.read();
        let mut snapshot: HashMap<NodeId, SnapNode> = HashMap::new();
        let mut stack = vec![self.root];
        while let Some(node) = stack.pop() {
            if snapshot.contains_key(&node) {
                continue;
            }
            let snap = match mgr.expect_node(node) {
                ManagerNode::Terminal(value) => SnapNode::Terminal(*value),
                ManagerNode::Decision { var, low, high } => {
                    let variable = mgr
                        .var_name(*var)
                        .expect("decision node variable must have a name")
                        .clone();
                    stack.push(*low);
                    stack.push(*high);
                    SnapNode::Decision {
                        variable,
                        low: *low,
                        high: *high,
                    }
                }
            };
            snapshot.insert(node, snap);
        }
        snapshot
    }
}

/// An owned snapshot of one BDD node, captured under a read guard so [`Bdd::fold`] /
/// [`Bdd::fold_with_context`] can run their user closures after the guard is released. Mirrors the
/// manager's node shape but owns the tested variable's name (so the borrow handed to the fold closures
/// outlives the read guard).
enum SnapNode {
    /// A terminal leaf — the constant `false` or `true`.
    Terminal(bool),
    /// A decision node testing `variable`, with its `low`/`high` child ids.
    Decision {
        variable: Symbol,
        low: NodeId,
        high: NodeId,
    },
}
