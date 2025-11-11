//! BDD manager implementation for canonical node representation
//!
//! This module contains the internal BDD data structures and management logic.
//! The BDD manager maintains:
//! - Global singleton manager with thread-local storage
//! - Hash consing for canonical node representation
//! - Operation caching for efficient boolean operations
//! - Variable ordering (alphabetical)

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
    pub(super) var_to_id: BTreeMap<Arc<str>, VarId>,
    /// Reverse mapping: variable id -> variable name
    pub(super) id_to_var: Vec<Arc<str>>,
    /// Cache for ITE operations: (f, g, h) -> result
    pub(super) ite_cache: HashMap<(NodeId, NodeId, NodeId), NodeId>,
    /// Cache for DNF: NodeId -> Weak<Dnf>
    /// Weak references allow sharing DNF across BDDs without preventing cleanup
    pub(super) dnf_cache: HashMap<NodeId, Weak<crate::cover::Dnf>>,
    /// Cache for factorised ASTs: NodeId -> Weak<BoolExprAst>
    /// Weak references allow sharing factorised ASTs without preventing cleanup
    pub(super) ast_cache: HashMap<NodeId, Weak<super::BoolExprAst>>,
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
                dnf_cache: HashMap::new(),
                ast_cache: HashMap::new(),
            }));
            *guard = Arc::downgrade(&manager);
            manager
        }
    }

    /// Get or create a variable ID for a variable name
    pub(super) fn get_or_create_var(&mut self, name: &str) -> VarId {
        let key: Arc<str> = Arc::from(name);
        if let Some(&id) = self.var_to_id.get(&key) {
            id
        } else {
            let id = self.id_to_var.len();
            self.var_to_id.insert(Arc::clone(&key), id);
            self.id_to_var.push(key);
            id
        }
    }

    /// Get variable name from ID
    pub(super) fn var_name(&self, id: VarId) -> Option<&Arc<str>> {
        self.id_to_var.get(id)
    }

    /// Get or create a decision node (with hash consing)
    ///
    /// # Invariant
    /// This method only creates Decision nodes, never Terminal nodes.
    /// Terminal nodes are always at positions 0 and 1.
    pub(super) fn make_node(&mut self, var: VarId, low: NodeId, high: NodeId) -> NodeId {
        // Reduction rule: if low == high, return that node (redundant test elimination)
        if low == high {
            return low;
        }

        // Check unique table
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

    /// If-Then-Else operation (Shannon expansion)
    ///
    /// Computes: if f then g else h
    /// This is the fundamental BDD operation from which all others are derived.
    pub(super) fn ite(&mut self, f: NodeId, g: NodeId, h: NodeId) -> NodeId {
        // Terminal cases
        if f == TRUE_NODE {
            return g;
        }
        if f == FALSE_NODE {
            return h;
        }
        if g == TRUE_NODE && h == FALSE_NODE {
            return f;
        }
        if g == h {
            return g;
        }

        // Check cache
        let cache_key = (f, g, h);
        if let Some(&result) = self.ite_cache.get(&cache_key) {
            return result;
        }

        // Find the topmost variable among f, g, h
        let f_node = self.get_node(f).expect(
            "Invalid node ID in ITE operation - this indicates a bug in the BDD implementation",
        );
        let g_node = self.get_node(g).expect(
            "Invalid node ID in ITE operation - this indicates a bug in the BDD implementation",
        );
        let h_node = self.get_node(h).expect(
            "Invalid node ID in ITE operation - this indicates a bug in the BDD implementation",
        );

        let (top_var, f_var, g_var, h_var) = match (f_node, g_node, h_node) {
            (BddNode::Terminal(_), BddNode::Terminal(_), BddNode::Terminal(_)) => {
                unreachable!("All terminals should be handled above")
            }
            _ => {
                let f_var = Self::node_var(f_node);
                let g_var = Self::node_var(g_node);
                let h_var = Self::node_var(h_node);
                let top_var = f_var.min(g_var).min(h_var);
                (top_var, f_var, g_var, h_var)
            }
        };

        // Shannon expansion on the topmost variable
        let (f_low, f_high) = Self::cofactors(f_node, f_var, top_var, f);
        let (g_low, g_high) = Self::cofactors(g_node, g_var, top_var, g);
        let (h_low, h_high) = Self::cofactors(h_node, h_var, top_var, h);

        let low = self.ite(f_low, g_low, h_low);
        let high = self.ite(f_high, g_high, h_high);

        let result = self.make_node(top_var, low, high);
        self.ite_cache.insert(cache_key, result);
        result
    }

    /// Get the variable of a node (usize::MAX for terminals)
    pub(super) fn node_var(node: &BddNode) -> VarId {
        match node {
            BddNode::Terminal(_) => usize::MAX,
            BddNode::Decision { var, .. } => *var,
        }
    }

    /// Get cofactors (low and high children) for Shannon expansion
    pub(super) fn cofactors(
        node: &BddNode,
        node_var: VarId,
        split_var: VarId,
        node_id: NodeId,
    ) -> (NodeId, NodeId) {
        if node_var == split_var {
            match node {
                BddNode::Decision { low, high, .. } => (*low, *high),
                BddNode::Terminal(_) => unreachable!(),
            }
        } else {
            // Variable doesn't appear in this branch
            (node_id, node_id)
        }
    }
}
