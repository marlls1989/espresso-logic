//! BDD manager implementation for canonical node representation
//!
//! This module contains the internal BDD data structures and management logic.
//! The BDD manager maintains:
//! - Global singleton manager with thread-local storage
//! - Hash consing for canonical node representation
//! - Operation caching for boolean operations
//! - Variable ordering (first-seen / insertion order)

use super::manager_cell::ManagerCell;
use crate::Symbol;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, RwLock, Weak};

/// Node identifier in the BDD
pub(crate) type NodeId = usize;

/// Variable identifier (index in variable ordering)
pub(crate) type VarId = usize;

/// Terminal node for FALSE
pub(crate) const FALSE_NODE: NodeId = 0;

/// Terminal node for TRUE
pub(crate) const TRUE_NODE: NodeId = 1;

/// Global weak reference to BDD manager
///
/// Using a weak reference allows the manager to be dropped when no BDDs are using it,
/// preventing memory leaks. A new manager will be created when needed.
///
/// The weak reference enables:
/// - Better cache hit rates when BDDs are actively in use (shared across all BDDs)
/// - Lower memory usage (shared node table)
/// - Hash consing works globally (same expressions = same nodes everywhere)
/// - Automatic cleanup when no BDDs are in use
pub(super) static GLOBAL_BDD_MANAGER: Mutex<Weak<RwLock<BddManager>>> = Mutex::new(Weak::new());

/// The owned storage handle every [`BoolExpr`](crate::BoolExpr) and [`BddContext`](crate::BddContext)
/// holds: an `Arc<RwLock<BddManager>>`. The [`Global`](crate::Global) brand shares one process-global
/// instance; each scoped context owns a fresh, independent one. Either way `Arc<RwLock<…>>` keeps
/// expressions `Send`/`Sync` and the manager alive while any expression references it.
pub(crate) type Store = Arc<RwLock<BddManager>>;

/// Binary decision diagram node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum BddNode {
    /// Terminal node (true or false)
    Terminal(bool),
    /// Decision node
    Decision {
        var: VarId,
        low: NodeId,  // false edge
        high: NodeId, // true edge
    },
}

/// Shared BDD manager that maintains the unique table and caches
///
/// The manager uses interior mutability to allow sharing BDDs across multiple references
/// while still being able to modify internal caches.
///
/// # Critical Invariant: NodeId Stability
///
/// **NodeIds are stable** - once a node is created at a given index, it remains at that
/// index forever. The `nodes` Vec only grows (via `push`), never shrinks or reorders.
/// This guarantees that:
/// - A NodeId is valid for the lifetime of the manager
/// - Multiple threads can safely traverse using NodeIds after releasing read locks
/// - Recursive traversal can release locks between calls without invalidating NodeIds
#[derive(Debug)]
pub(crate) struct BddManager {
    /// All nodes in the BDD (terminals at indices 0 and 1)
    /// INVARIANT: Nodes are never removed or reordered - only appended
    pub(super) nodes: Vec<BddNode>,
    /// Unique table: (var, low, high) -> NodeId for hash consing
    pub(super) unique_table: HashMap<(VarId, NodeId, NodeId), NodeId>,
    /// Variable ordering: variable name -> variable id
    pub(super) var_to_id: BTreeMap<Symbol, VarId>,
    /// Reverse mapping: variable id -> variable name
    pub(super) id_to_var: Vec<Symbol>,
    /// Cache for ITE operations: (f, g, h) -> result
    pub(super) ite_cache: HashMap<(NodeId, NodeId, NodeId), NodeId>,
}

impl BddManager {
    /// Get or create the singleton BDD manager
    ///
    /// All BDDs in the program share a single manager for maximum efficiency
    /// through shared node tables and caches. The manager is automatically
    /// cleaned up when no BDDs reference it anymore.
    pub(super) fn get_or_create() -> Arc<RwLock<Self>> {
        let mut guard = GLOBAL_BDD_MANAGER.lock().unwrap();
        if let Some(manager) = guard.upgrade() {
            manager
        } else {
            let manager = Arc::new(RwLock::new(BddManager::new_empty()));
            *guard = Arc::downgrade(&manager);
            manager
        }
    }

    /// A fresh, empty manager seeded with the two terminal nodes (`FALSE_NODE = 0`, `TRUE_NODE = 1`).
    ///
    /// Used both for the global singleton and for every scoped context's store (via
    /// [`new_store`](Self::new_store)).
    pub(super) fn new_empty() -> Self {
        BddManager {
            nodes: vec![
                BddNode::Terminal(false), // FALSE_NODE = 0
                BddNode::Terminal(true),  // TRUE_NODE = 1
            ],
            unique_table: HashMap::new(),
            var_to_id: BTreeMap::new(),
            id_to_var: Vec::new(),
            ite_cache: HashMap::new(),
        }
    }

    /// A fresh, independent [`Store`] backing a single scoped [`BddContext`](crate::BddContext).
    pub(super) fn new_store() -> Store {
        Arc::new(RwLock::new(BddManager::new_empty()))
    }

    /// Get or create the variable id for `name`, managing the cell's borrow itself.
    ///
    /// Read-mostly: an already-known variable resolves under a shared borrow (concurrent lookups run in
    /// parallel on a [`SyncCell`](super::manager_cell::SyncCell)); only a genuinely new variable escalates
    /// to an exclusive borrow to append it. The shared borrow is dropped before the exclusive borrow is
    /// taken, so the read→write hand-off never overlaps two borrows of the same cell — required for the
    /// [`LocalCell`](super::manager_cell::LocalCell)'s `RefCell` (which would panic on overlap) and the
    /// `SyncCell`'s `RwLock` (which would deadlock).
    pub(crate) fn make_var<C: ManagerCell>(cell: &C, name: &str) -> VarId {
        {
            let manager = cell.read();
            if let Some(id) = manager.var_id(name) {
                return id;
            }
        }
        // Re-check under the exclusive borrow: another thread may have appended `name` meanwhile.
        cell.write().get_or_create_var(name)
    }

    /// Read-only lookup of an existing variable id — the shared-borrow fast path of
    /// [`make_var`](Self::make_var). Also used by the BDD layer to resolve a variable *name* to a
    /// `VarId` without creating it (a name absent from the ordering yields `None`, which the cofactor /
    /// quantification primitives treat as a no-op).
    pub(crate) fn var_id(&self, name: &str) -> Option<VarId> {
        self.var_to_id.get(&Symbol::from(name)).copied()
    }

    /// Append `name` as a new variable (or return its id if already present). Caller holds the write lock.
    fn get_or_create_var(&mut self, name: &str) -> VarId {
        let key: Symbol = Symbol::from(name);
        if let Some(&id) = self.var_to_id.get(&key) {
            id
        } else {
            let id = self.id_to_var.len();
            self.var_to_id.insert(key.clone(), id);
            self.id_to_var.push(key);
            id
        }
    }

    /// Get variable name from ID
    pub(crate) fn var_name(&self, id: VarId) -> Option<&Symbol> {
        self.id_to_var.get(id)
    }

    /// Get or create a canonical decision node, managing the cell's borrow itself.
    ///
    /// Read-mostly hash-consing: the reduction rule needs no borrow, an already-interned node resolves
    /// under a shared borrow (concurrent lookups run in parallel on a
    /// [`SyncCell`](super::manager_cell::SyncCell)), and only a brand-new node escalates to an exclusive
    /// borrow. The shared borrow is dropped before the exclusive borrow is taken, so the two never
    /// overlap (see [`make_var`](Self::make_var)). NodeIds are stable, so the id returned from the read
    /// path stays valid after its borrow is released.
    pub(crate) fn make_node<C: ManagerCell>(
        cell: &C,
        var: VarId,
        low: NodeId,
        high: NodeId,
    ) -> NodeId {
        // Reduction rule (no borrow): a redundant test collapses to its child.
        if low == high {
            return low;
        }
        let key = (var, low, high);
        // Shared-borrow fast path: an existing canonical node needs no exclusive borrow.
        {
            let manager = cell.read();
            if let Some(&existing) = manager.unique_table.get(&key) {
                return existing;
            }
        }
        // Append path: re-check under the exclusive borrow (another thread may have interned it), insert.
        cell.write().insert_node(var, low, high)
    }

    /// Intern a decision node, re-checking the unique table. Caller holds the write lock.
    ///
    /// # Invariant
    /// Only creates Decision nodes, never Terminal nodes (terminals are fixed at positions 0 and 1).
    fn insert_node(&mut self, var: VarId, low: NodeId, high: NodeId) -> NodeId {
        // Reduction rule: if low == high, return that node (redundant test elimination)
        if low == high {
            return low;
        }

        // Authoritative unique-table check (the read-path check above is only advisory)
        let key = (var, low, high);
        if let Some(&existing) = self.unique_table.get(&key) {
            return existing;
        }

        // Create new decision node (never terminals - those are at 0 and 1)
        let node_id = self.nodes.len();
        self.nodes.push(BddNode::Decision { var, low, high });
        self.unique_table.insert(key, node_id);
        node_id
    }

    /// Get node by ID
    pub(crate) fn get_node(&self, id: NodeId) -> Option<&BddNode> {
        self.nodes.get(id)
    }

    /// If-Then-Else (`if f then g else h`), managing the cell's borrow itself.
    ///
    /// The fundamental BDD operation all others derive from. Read-mostly, but — unlike a held-guard
    /// design — **no borrow spans more than a single step**: each `Solve`/`Combine` step takes its own
    /// short-lived shared borrow to read (resolve/expand/read children) and, only when a triple must be
    /// committed, a separate short-lived exclusive borrow to intern the node and record its cache entry.
    /// A shared borrow is never live when the exclusive borrow is taken, which is the discipline the
    /// [`LocalCell`](super::manager_cell::LocalCell)'s `RefCell` requires (overlapping `borrow()` then
    /// `borrow_mut()` would panic) and the [`SyncCell`](super::manager_cell::SyncCell)'s `RwLock` requires
    /// (read→write on the same lock would deadlock). NodeIds and cache entries are stable / monotonic, so
    /// releasing the borrow between steps never invalidates an id read in an earlier step. Each commit
    /// interns the node and records its cache entry as one transaction (never released with a node created
    /// but its result uncached), so re-deriving an existing expression resolves entirely on shared borrows
    /// (parallel across threads on the `SyncCell`), and even a fresh computation takes the exclusive
    /// borrow only momentarily, once per committed triple.
    ///
    /// Evaluated **iteratively** with an explicit work-stack rather than recursion, so a tall BDD (deep
    /// variable ordering) can't overflow the call stack. Memoisation is preserved exactly: every
    /// sub-triple is resolved through `ite_resolved` (terminal cases + `ite_cache`), so shared
    /// sub-problems collapse to cache hits, keeping the walk linear in the number of distinct reachable
    /// triples, not exponential.
    pub(crate) fn ite<C: ManagerCell>(cell: &C, f: NodeId, g: NodeId, h: NodeId) -> NodeId {
        /// One unit of work. `Solve` resolves a triple (expanding it if needed); `Combine` runs
        /// after a triple's two children are resolved and builds the result node for it.
        enum Work {
            Solve(NodeId, NodeId, NodeId),
            Combine {
                triple: (NodeId, NodeId, NodeId),
                top_var: VarId,
                low: (NodeId, NodeId, NodeId),
                high: (NodeId, NodeId, NodeId),
            },
        }

        let mut stack = vec![Work::Solve(f, g, h)];
        while let Some(work) = stack.pop() {
            match work {
                Work::Solve(f, g, h) => {
                    // Resolve or Shannon-expand under one short-lived shared borrow, released at the end
                    // of this block (before any later step takes its own borrow).
                    let expanded = {
                        let manager = cell.read();
                        if manager.ite_resolved(f, g, h).is_some() {
                            None
                        } else {
                            Some(manager.ite_expand(f, g, h))
                        }
                    };
                    // Bail if the triple was already resolved (terminal or memoised).
                    let Some((top_var, low, high)) = expanded else {
                        continue;
                    };
                    // Schedule both cofactor triples and a Combine that runs once they're resolved
                    // (Combine pushed first → pops last, LIFO).
                    stack.push(Work::Combine {
                        triple: (f, g, h),
                        top_var,
                        low,
                        high,
                    });
                    stack.push(Work::Solve(high.0, high.1, high.2));
                    stack.push(Work::Solve(low.0, low.1, low.2));
                }
                Work::Combine {
                    triple,
                    top_var,
                    low,
                    high,
                } => {
                    // Read the resolved children under one short-lived shared borrow. A diamond can
                    // schedule the same Combine twice; the first caches the result, so skip if it is
                    // already there.
                    let children = {
                        let manager = cell.read();
                        if manager.ite_cache.contains_key(&triple) {
                            None
                        } else {
                            let low_id = manager.ite_resolved(low.0, low.1, low.2).expect(
                                "ITE low child unresolved at combine time - BDD scheduling bug",
                            );
                            let high_id = manager.ite_resolved(high.0, high.1, high.2).expect(
                                "ITE high child unresolved at combine time - BDD scheduling bug",
                            );
                            Some((low_id, high_id))
                        }
                    };
                    let Some((low_id, high_id)) = children else {
                        continue;
                    };
                    // Commit under a separate short-lived exclusive borrow — taken only after the shared
                    // borrow above has been dropped, so the two never overlap. Intern the node and record
                    // its cache entry as one transaction, re-checking in case another thread committed it
                    // meanwhile.
                    let mut manager = cell.write();
                    if !manager.ite_cache.contains_key(&triple) {
                        let result = manager.insert_node(top_var, low_id, high_id);
                        manager.ite_cache.insert(triple, result);
                    }
                }
            }
        }

        // Final result read under its own short-lived shared borrow.
        cell.read().ite_resolved(f, g, h).expect(
            "top-level ITE triple unresolved after iterative evaluation - BDD scheduling bug",
        )
    }

    /// Exclusive-or of two nodes, `xor(f, g) = ite(f, ¬g, g)`, managing the cell's borrow itself.
    ///
    /// Built from [`ite`](Self::ite) (so it inherits the same hash-consing and memoisation and stays
    /// canonical): `¬g = ite(g, FALSE, TRUE)`, then select `¬g` when `f` is true and `g` when `f` is
    /// false. Each sub-`ite` does its own read-mostly borrowing. Shared by
    /// [`BoolExpr::xor`](crate::BoolExpr::xor) and the public BDD builder.
    pub(crate) fn xor<C: ManagerCell>(cell: &C, f: NodeId, g: NodeId) -> NodeId {
        let not_g = Self::ite(cell, g, FALSE_NODE, TRUE_NODE);
        Self::ite(cell, f, not_g, g)
    }

    /// Shannon cofactor by assignment: substitute `var := value` throughout `node` and reduce.
    ///
    /// Returns the canonical root of `node|var=value` — the function obtained by fixing variable `var`
    /// to `value` (a.k.a. *restrict*). Restricting a node that never tests `var` is a no-op and returns
    /// `node` unchanged.
    ///
    /// Recursive substitute-and-reduce: at each decision node testing `var`, the matching child replaces
    /// the node; at a node testing another variable, both children are restricted and re-interned with
    /// [`make_node`](Self::make_node) (which preserves canonicity and applies the reduction rule). Each
    /// recursion level reads the node's `(var, low, high)` under a **single short-lived shared borrow**,
    /// drops it, recurses, and only then interns the rebuilt node via `make_node` — so no shared borrow
    /// is ever live when the exclusive borrow `make_node` may take is taken (the borrow discipline the
    /// [`LocalCell`](super::manager_cell::LocalCell)'s `RefCell` requires and the
    /// [`SyncCell`](super::manager_cell::SyncCell)'s `RwLock` requires). A per-call memo collapses the
    /// shared sub-DAG so the walk stays linear in the number of distinct reachable nodes.
    pub(crate) fn restrict<C: ManagerCell>(
        cell: &C,
        node: NodeId,
        var: VarId,
        value: bool,
    ) -> NodeId {
        let mut memo: HashMap<NodeId, NodeId> = HashMap::new();
        Self::restrict_rec(cell, node, var, value, &mut memo)
    }

    /// Recursive core of [`restrict`](Self::restrict), threading a per-call memo.
    fn restrict_rec<C: ManagerCell>(
        cell: &C,
        node: NodeId,
        var: VarId,
        value: bool,
        memo: &mut HashMap<NodeId, NodeId>,
    ) -> NodeId {
        if let Some(&cached) = memo.get(&node) {
            return cached;
        }

        // Read this node's shape under one short-lived shared borrow, then drop it before recursing or
        // interning (the borrow discipline: never hold a read across a potential write).
        let shape = {
            let manager = cell.read();
            match manager.get_node(node).expect(
                "Invalid node ID in restrict - this indicates a bug in the BDD implementation",
            ) {
                // Terminals carry no variable: restricting cannot change a constant.
                BddNode::Terminal(_) => None,
                BddNode::Decision {
                    var: v,
                    low,
                    high,
                } => Some((*v, *low, *high)),
            }
        };

        let result = match shape {
            None => node,
            Some((v, low, high)) => {
                if v == var {
                    // This node tests `var`: collapse to the matching cofactor and continue restricting
                    // it (a deeper node could test `var` again only on a non-reduced order, but recursing
                    // is always correct and the memo keeps it cheap).
                    let chosen = if value { high } else { low };
                    Self::restrict_rec(cell, chosen, var, value, memo)
                } else {
                    // `var` is not tested here: restrict both children and re-intern. If neither child
                    // changed, `make_node` returns the canonical id for the same triple, so an
                    // unaffected subgraph rebuilds to itself (the no-op guarantee).
                    let new_low = Self::restrict_rec(cell, low, var, value, memo);
                    let new_high = Self::restrict_rec(cell, high, var, value, memo);
                    Self::make_node(cell, v, new_low, new_high)
                }
            }
        };

        memo.insert(node, result);
        result
    }

    /// Resolve an ITE triple **without** Shannon expansion.
    ///
    /// Returns `Some(node)` when `(f, g, h)` is a terminal case or already lives in `ite_cache`,
    /// and `None` when it still needs expanding. This is the memo-aware lookup the iterative
    /// [`ite`](Self::ite) loop uses both to short-circuit `Solve` items and to read back child
    /// results in `Combine`. The terminal-case checks mirror the head of the former recursive `ite`.
    fn ite_resolved(&self, f: NodeId, g: NodeId, h: NodeId) -> Option<NodeId> {
        if f == TRUE_NODE {
            return Some(g);
        }
        if f == FALSE_NODE {
            return Some(h);
        }
        if g == TRUE_NODE && h == FALSE_NODE {
            return Some(f);
        }
        if g == h {
            return Some(g);
        }
        self.ite_cache.get(&(f, g, h)).copied()
    }

    /// Shannon-expand a non-terminal ITE triple around its topmost variable.
    ///
    /// Returns the split variable and the two child triples (low/false cofactor and high/true
    /// cofactor). Only called when [`ite_resolved`](Self::ite_resolved) returned `None`, so at
    /// least `f` is a decision node and `top_var` is a real variable.
    fn ite_expand(
        &self,
        f: NodeId,
        g: NodeId,
        h: NodeId,
    ) -> (VarId, (NodeId, NodeId, NodeId), (NodeId, NodeId, NodeId)) {
        let f_node = self.get_node(f).expect(
            "Invalid node ID in ITE operation - this indicates a bug in the BDD implementation",
        );
        let g_node = self.get_node(g).expect(
            "Invalid node ID in ITE operation - this indicates a bug in the BDD implementation",
        );
        let h_node = self.get_node(h).expect(
            "Invalid node ID in ITE operation - this indicates a bug in the BDD implementation",
        );

        let f_var = Self::node_var(f_node);
        let g_var = Self::node_var(g_node);
        let h_var = Self::node_var(h_node);
        let top_var = f_var.min(g_var).min(h_var);

        let (f_low, f_high) = Self::cofactors(f_node, f_var, top_var, f);
        let (g_low, g_high) = Self::cofactors(g_node, g_var, top_var, g);
        let (h_low, h_high) = Self::cofactors(h_node, h_var, top_var, h);

        (top_var, (f_low, g_low, h_low), (f_high, g_high, h_high))
    }

    /// Get the variable of a node (usize::MAX for terminals)
    fn node_var(node: &BddNode) -> VarId {
        match node {
            BddNode::Terminal(_) => usize::MAX,
            BddNode::Decision { var, .. } => *var,
        }
    }

    /// Get cofactors (low and high children) for Shannon expansion
    fn cofactors(
        node: &BddNode,
        node_var: VarId,
        split_var: VarId,
        node_id: NodeId,
    ) -> (NodeId, NodeId) {
        if node_var == split_var {
            match node {
                BddNode::Decision { low, high, .. } => (*low, *high),
                // A terminal's `node_var` is `usize::MAX`; `split_var` is always a real variable,
                // so `node_var == split_var` cannot hold for a terminal node.
                BddNode::Terminal(_) => {
                    unreachable!("terminal node cannot match a real split variable")
                }
            }
        } else {
            // Variable doesn't appear in this branch
            (node_id, node_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::manager_cell::{LocalCell, SyncCell};

    /// Build `(a & b) | (a ^ c)` over an arbitrary cell, exercising `make_var`, `make_node`, `ite`, and
    /// `xor` and returning the root and the variable arity. Used to assert both cells produce the same
    /// canonical structure through the *single* generic engine.
    fn build_sample<C: ManagerCell>(cell: &C) -> (NodeId, usize) {
        let a = BddManager::make_var(cell, "a");
        let b = BddManager::make_var(cell, "b");
        let c = BddManager::make_var(cell, "c");
        let a_node = BddManager::make_node(cell, a, FALSE_NODE, TRUE_NODE);
        let b_node = BddManager::make_node(cell, b, FALSE_NODE, TRUE_NODE);
        let c_node = BddManager::make_node(cell, c, FALSE_NODE, TRUE_NODE);

        let and = BddManager::ite(cell, a_node, b_node, FALSE_NODE);
        let xor = BddManager::xor(cell, a_node, c_node);
        let or = BddManager::ite(cell, and, TRUE_NODE, xor);
        (or, cell.read().id_to_var.len())
    }

    /// The `RefCell`-backed [`LocalCell`] must drive the engine without panicking — the borrow
    /// discipline (no shared borrow live when an exclusive borrow is taken) is the riskiest part of the
    /// engine abstraction, and a `RefCell` panics on overlap where the `RwLock` would deadlock.
    #[test]
    fn engine_runs_on_local_cell() {
        let cell = LocalCell::new_empty();
        let (root, arity) = build_sample(&cell);
        assert_eq!(arity, 3);
        // Re-deriving an identical expression must hit the caches and yield the same canonical root.
        let (root2, _) = build_sample(&cell);
        assert_eq!(root, root2);
    }

    /// The `RwLock`-backed [`SyncCell`] must produce the *same* canonical root and arity as the
    /// `LocalCell` — the two cells share one generic engine, so structure must be identical.
    #[test]
    fn both_cells_agree() {
        let local = LocalCell::new_empty();
        let sync = SyncCell::new_empty();
        let (local_root, local_arity) = build_sample(&local);
        let (sync_root, sync_arity) = build_sample(&sync);
        assert_eq!(local_root, sync_root);
        assert_eq!(local_arity, sync_arity);
    }

    /// `restrict` must implement Shannon cofactor by assignment over the engine: `(a & b)|a=1 == b`,
    /// `(a & b)|a=0 == FALSE`, and restricting an absent variable is a no-op. Driven over the
    /// `RefCell`-backed [`LocalCell`] to exercise the borrow discipline of the recursive walk.
    #[test]
    fn restrict_cofactors_on_local_cell() {
        let cell = LocalCell::new_empty();
        let a = BddManager::make_var(&cell, "a");
        let b = BddManager::make_var(&cell, "b");
        let a_node = BddManager::make_node(&cell, a, FALSE_NODE, TRUE_NODE);
        let b_node = BddManager::make_node(&cell, b, FALSE_NODE, TRUE_NODE);
        let and = BddManager::ite(&cell, a_node, b_node, FALSE_NODE); // a & b

        // (a & b)|a=true == b
        assert_eq!(BddManager::restrict(&cell, and, a, true), b_node);
        // (a & b)|a=false == FALSE
        assert_eq!(BddManager::restrict(&cell, and, a, false), FALSE_NODE);

        // Restricting a variable absent from the function is a no-op.
        let c = BddManager::make_var(&cell, "c");
        assert_eq!(BddManager::restrict(&cell, and, c, true), and);
    }

    /// A deeply nested chain must not overflow the stack on either cell (the engine is iterative) and
    /// must not trip the `RefCell`'s borrow discipline.
    #[test]
    fn deep_chain_on_local_cell() {
        let cell = LocalCell::new_empty();
        let names: Vec<String> = (0..400).map(|i| format!("v{i}")).collect();
        let mut acc = {
            let id = BddManager::make_var(&cell, &names[0]);
            BddManager::make_node(&cell, id, FALSE_NODE, TRUE_NODE)
        };
        for name in &names[1..] {
            let id = BddManager::make_var(&cell, name);
            let node = BddManager::make_node(&cell, id, FALSE_NODE, TRUE_NODE);
            acc = BddManager::ite(&cell, acc, node, FALSE_NODE); // acc & node
        }
        assert_ne!(acc, FALSE_NODE);
    }
}
