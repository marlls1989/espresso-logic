//! BDD manager implementation for canonical node representation
//!
//! This module contains the internal BDD data structures and management logic.
//! The BDD manager maintains:
//! - Global singleton manager with thread-local storage
//! - Hash consing for canonical node representation
//! - Operation caching for efficient boolean operations
//! - Variable ordering (first-seen / insertion order)

use crate::Symbol;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, RwLock, Weak};

/// Node identifier in the BDD
pub(super) type NodeId = usize;

/// Variable identifier (index in variable ordering)
pub(super) type VarId = usize;

/// Terminal node for FALSE
pub(super) const FALSE_NODE: NodeId = 0;

/// Terminal node for TRUE
pub(super) const TRUE_NODE: NodeId = 1;

/// Global weak reference to BDD manager
///
/// Using a weak reference allows the manager to be dropped when no BDDs are using it,
/// preventing memory leaks. A new manager will be created when needed.
///
/// The weak reference enables:
/// - Better cache hit rates when BDDs are actively in use (shared across all BDDs)
/// - More efficient memory usage (shared node table)
/// - Hash consing works globally (same expressions = same nodes everywhere)
/// - Automatic cleanup when no BDDs are in use
pub(super) static GLOBAL_BDD_MANAGER: Mutex<Weak<RwLock<BddManager>>> = Mutex::new(Weak::new());

/// Binary decision diagram node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum BddNode {
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
pub(super) struct BddManager {
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
            // Initialise manager inline with terminal nodes
            let manager = Arc::new(RwLock::new(BddManager {
                nodes: vec![
                    BddNode::Terminal(false), // FALSE_NODE = 0
                    BddNode::Terminal(true),  // TRUE_NODE = 1
                ],
                unique_table: HashMap::new(),
                var_to_id: BTreeMap::new(),
                id_to_var: Vec::new(),
                ite_cache: HashMap::new(),
            }));
            *guard = Arc::downgrade(&manager);
            manager
        }
    }

    /// Get or create the variable id for `name`, managing the manager's lock itself.
    ///
    /// Read-mostly: an already-known variable resolves under a shared read lock (concurrent lookups run
    /// in parallel); only a genuinely new variable escalates to the write lock to append it.
    pub(super) fn make_var(lock: &RwLock<Self>, name: &str) -> VarId {
        {
            let manager = lock.read().unwrap();
            if let Some(id) = manager.var_id(name) {
                return id;
            }
        }
        // Re-check under the write lock: another thread may have appended `name` meanwhile.
        lock.write().unwrap().get_or_create_var(name)
    }

    /// Read-only lookup of an existing variable id — the shared-lock fast path of
    /// [`make_var`](Self::make_var).
    fn var_id(&self, name: &str) -> Option<VarId> {
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
    pub(super) fn var_name(&self, id: VarId) -> Option<&Symbol> {
        self.id_to_var.get(id)
    }

    /// Get or create a canonical decision node, managing the manager's lock itself.
    ///
    /// Read-mostly hash-consing: the reduction rule needs no lock, an already-interned node resolves
    /// under a shared read lock (concurrent lookups run in parallel), and only a brand-new node
    /// escalates to the write lock. NodeIds are stable, so the id returned from the read path stays
    /// valid after the lock is released.
    pub(super) fn make_node(lock: &RwLock<Self>, var: VarId, low: NodeId, high: NodeId) -> NodeId {
        // Reduction rule (no lock): a redundant test collapses to its child.
        if low == high {
            return low;
        }
        let key = (var, low, high);
        // Shared-lock fast path: an existing canonical node needs no write lock.
        {
            let manager = lock.read().unwrap();
            if let Some(&existing) = manager.unique_table.get(&key) {
                return existing;
            }
        }
        // Append path: re-check under the write lock (another thread may have interned it), then insert.
        lock.write().unwrap().insert_node(var, low, high)
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
    pub(super) fn get_node(&self, id: NodeId) -> Option<&BddNode> {
        self.nodes.get(id)
    }

    /// If-Then-Else (`if f then g else h`), managing the manager's lock itself.
    ///
    /// The fundamental BDD operation all others derive from. Read-mostly: a **single read lock is held
    /// across the whole traversal**, shared by every read step — cache/terminal lookups
    /// ([`ite_resolved`](Self::ite_resolved)), Shannon expansion ([`ite_expand`](Self::ite_expand)),
    /// reading back child results, and the final result read. It is dropped *only* to **commit** a
    /// resolved triple (the lone write), then re-acquired: read-then-write on the same lock would
    /// deadlock, so the read lock is released for exactly the duration of that write. Each commit interns
    /// the result node and records its cache entry as one atomic transaction (never released with a node
    /// created but its result uncached). So re-deriving an existing expression resolves under one read
    /// lock with no writes at all (parallel across threads), and even a fresh computation takes the write
    /// lock only momentarily, once per committed triple.
    ///
    /// Evaluated **iteratively** with an explicit work-stack rather than recursion, so a tall BDD (deep
    /// variable ordering) can't overflow the call stack. Memoisation is preserved exactly: every
    /// sub-triple is resolved through `ite_resolved` (terminal cases + `ite_cache`), so shared
    /// sub-problems collapse to cache hits, keeping the walk linear in the number of distinct reachable
    /// triples, not exponential. NodeIds are stable, so an id read under one lock stays valid for use
    /// after that lock is released.
    pub(super) fn ite(lock: &RwLock<Self>, f: NodeId, g: NodeId, h: NodeId) -> NodeId {
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
        // One read lock held across the whole traversal — every read step (Solve's resolve/expand,
        // Combine's cache/child reads, and the final result read) shares it. It is dropped *only* when a
        // Combine must commit (a write), then re-acquired: read-then-write on the same `RwLock` would
        // deadlock, so the read lock is released for exactly the duration of the write and no longer.
        let mut guard = lock.read().unwrap();
        while let Some(work) = stack.pop() {
            match work {
                Work::Solve(f, g, h) => {
                    // Bail if the triple is already resolved (terminal or memoised), otherwise
                    // Shannon-expand around the topmost variable — both under the held read lock.
                    if guard.ite_resolved(f, g, h).is_some() {
                        continue;
                    }
                    let (top_var, low, high) = guard.ite_expand(f, g, h);
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
                    // A diamond can schedule the same Combine twice; the first caches the result, so skip
                    // if it is already there (keep holding the read lock). Otherwise both children are
                    // resolved by now — read their ids under the held read lock.
                    if guard.ite_cache.contains_key(&triple) {
                        continue;
                    }
                    let low_id = guard
                        .ite_resolved(low.0, low.1, low.2)
                        .expect("ITE low child unresolved at combine time - BDD scheduling bug");
                    let high_id = guard
                        .ite_resolved(high.0, high.1, high.2)
                        .expect("ITE high child unresolved at combine time - BDD scheduling bug");
                    // Committing this triple is an append (to `ite_cache`, and possibly `nodes`) — the
                    // only thing that forces dropping the read lock. Drop it, then under the write lock
                    // intern the node and record its cache entry as one transaction (never released with
                    // a node created but its result uncached), re-checking in case another thread
                    // committed it meanwhile. Then re-acquire the read lock and carry on.
                    drop(guard);
                    {
                        let mut manager = lock.write().unwrap();
                        if !manager.ite_cache.contains_key(&triple) {
                            let result = manager.insert_node(top_var, low_id, high_id);
                            manager.ite_cache.insert(triple, result);
                        }
                    }
                    guard = lock.read().unwrap();
                }
            }
        }

        // The read lock is still held here, so the final result read needs no new acquisition.
        guard.ite_resolved(f, g, h).expect(
            "top-level ITE triple unresolved after iterative evaluation - BDD scheduling bug",
        )
    }

    /// Exclusive-or of two nodes, `xor(f, g) = ite(f, ¬g, g)`, managing the manager's lock itself.
    ///
    /// Built from [`ite`](Self::ite) (so it inherits the same hash-consing and memoisation and stays
    /// canonical): `¬g = ite(g, FALSE, TRUE)`, then select `¬g` when `f` is true and `g` when `f` is
    /// false. Each sub-`ite` does its own read-mostly locking. Shared by
    /// [`BoolExpr::xor`](crate::BoolExpr::xor) and the public BDD builder.
    pub(super) fn xor(lock: &RwLock<Self>, f: NodeId, g: NodeId) -> NodeId {
        let not_g = Self::ite(lock, g, FALSE_NODE, TRUE_NODE);
        Self::ite(lock, f, not_g, g)
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
