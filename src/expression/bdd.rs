//! Binary Decision Diagram (BDD) implementation for efficient boolean function representation
//!
//! This module provides a canonical representation of boolean functions using reduced ordered
//! binary decision diagrams (ROBDDs). BDDs offer several advantages over direct DNF conversion:
//!
//! - **Canonical representation**: Equivalent functions have identical BDD representations
//! - **Efficient operations**: AND, OR, NOT operations are polynomial time
//! - **Compact representation**: Many practical functions have small BDDs
//! - **Global sharing**: All BDDs in the program share the same manager for maximum efficiency
//!
//! # Implementation Details
//!
//! The BDD uses:
//! - **Global singleton manager**: One shared manager across all BDDs via `OnceLock`
//! - **Hash consing**: Unique table for canonical node representation (works globally)
//! - **Operation caching**: ITE results are memoized and shared across all operations
//! - **Variable ordering**: Alphabetical ordering (deterministic and consistent)
//! - **Thread-safe**: Mutex-protected manager enables concurrent BDD operations

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};

/// Node identifier in the BDD
pub type NodeId = usize;

/// Variable identifier (index in variable ordering)
pub type VarId = usize;

/// Terminal node for FALSE
pub const FALSE_NODE: NodeId = 0;

/// Terminal node for TRUE
pub const TRUE_NODE: NodeId = 1;

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
static GLOBAL_BDD_MANAGER: Mutex<Weak<RwLock<BddManager>>> = Mutex::new(Weak::new());

/// Binary decision diagram node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum BddNode {
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
struct BddManager {
    /// All nodes in the BDD (terminals at indices 0 and 1)
    /// INVARIANT: Nodes are never removed or reordered - only appended
    nodes: Vec<BddNode>,
    /// Unique table: (var, low, high) -> NodeId for hash consing
    unique_table: HashMap<(VarId, NodeId, NodeId), NodeId>,
    /// Variable ordering: variable name -> variable id
    var_to_id: BTreeMap<Arc<str>, VarId>,
    /// Reverse mapping: variable id -> variable name
    id_to_var: Vec<Arc<str>>,
    /// Cache for ITE operations: (f, g, h) -> result
    ite_cache: HashMap<(NodeId, NodeId, NodeId), NodeId>,
}

impl BddManager {
    /// Get or create the singleton BDD manager
    ///
    /// All BDDs in the program share a single manager for maximum efficiency
    /// through shared node tables and caches. The manager is automatically
    /// cleaned up when no BDDs reference it anymore.
    fn get_or_create() -> Arc<RwLock<Self>> {
        let mut guard = GLOBAL_BDD_MANAGER.lock().unwrap();
        if let Some(manager) = guard.upgrade() {
            manager
        } else {
            // Initialize manager inline with terminal nodes
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

    /// Get or create a variable ID for a variable name
    fn get_or_create_var(&mut self, name: &str) -> VarId {
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
    fn var_name(&self, id: VarId) -> Option<&Arc<str>> {
        self.id_to_var.get(id)
    }

    /// Get or create a decision node (with hash consing)
    ///
    /// # Invariant
    /// This method only creates Decision nodes, never Terminal nodes.
    /// Terminal nodes are always at positions 0 and 1.
    fn make_node(&mut self, var: VarId, low: NodeId, high: NodeId) -> NodeId {
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
    fn get_node(&self, id: NodeId) -> Option<&BddNode> {
        self.nodes.get(id)
    }

    /// If-Then-Else operation (Shannon expansion)
    ///
    /// Computes: if f then g else h
    /// This is the fundamental BDD operation from which all others are derived.
    fn ite(&mut self, f: NodeId, g: NodeId, h: NodeId) -> NodeId {
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
                BddNode::Terminal(_) => unreachable!(),
            }
        } else {
            // Variable doesn't appear in this branch
            (node_id, node_id)
        }
    }
}

/// Binary Decision Diagram
///
/// Represents a boolean function in canonical form. BDDs support efficient
/// boolean operations and can be converted to/from [`BoolExpr`].
///
/// BDDs are primarily used for efficient cover generation from boolean expressions.
/// When minimizing a [`BoolExpr`], it is first converted to a BDD, then to cubes,
/// which are then minimized by the Espresso algorithm.
///
/// [`BoolExpr`]: crate::expression::BoolExpr
#[derive(Debug, Clone)]
pub struct Bdd {
    manager: Arc<RwLock<BddManager>>,
    root: NodeId,
}

impl Bdd {
    /// Create a BDD representing a constant
    pub fn constant(value: bool) -> Self {
        let manager = BddManager::get_or_create();
        Bdd {
            manager,
            root: if value { TRUE_NODE } else { FALSE_NODE },
        }
    }

    /// Create a BDD representing a variable
    pub fn variable(name: &str) -> Self {
        let manager = BddManager::get_or_create();
        let mut mgr = manager.write().unwrap();
        let var_id = mgr.get_or_create_var(name);
        let node = mgr.make_node(var_id, FALSE_NODE, TRUE_NODE);
        drop(mgr); // Explicitly release the lock
        Bdd {
            manager,
            root: node,
        }
    }

    /// Create a BDD from a [`BoolExpr`]
    ///
    /// This is a convenient wrapper around [`BoolExpr::to_bdd()`].
    ///
    /// [`BoolExpr`]: crate::expression::BoolExpr
    /// [`BoolExpr::to_bdd()`]: crate::expression::BoolExpr::to_bdd
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, Bdd};
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b);
    ///
    /// let bdd = Bdd::from_expr(&expr);
    /// assert_eq!(bdd.node_count(), 4); // 2 terminals + 2 decision nodes
    /// ```
    pub fn from_expr(expr: &crate::expression::BoolExpr) -> Self {
        expr.to_bdd()
    }

    /// Convert this BDD to a [`BoolExpr`]
    ///
    /// Extracts cubes from the BDD and reconstructs a [`BoolExpr`] in DNF form.
    ///
    /// [`BoolExpr`]: crate::expression::BoolExpr
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, Bdd};
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b);
    ///
    /// let bdd = expr.to_bdd();
    /// let expr2 = bdd.to_expr();
    ///
    /// // Should be logically equivalent
    /// assert!(expr.equivalent_to(&expr2));
    /// ```
    pub fn to_expr(&self) -> crate::expression::BoolExpr {
        use crate::expression::BoolExpr;

        let cubes = self.to_cubes();

        if cubes.is_empty() {
            return BoolExpr::constant(false);
        }

        // Convert each cube to a product term
        let mut terms = Vec::new();
        for cube in cubes {
            if cube.is_empty() {
                // Empty cube means all variables are don't-care (tautology)
                terms.push(BoolExpr::constant(true));
            } else {
                // Build product term for this cube
                let mut factors: Vec<BoolExpr> = Vec::new();
                for (var, &polarity) in &cube {
                    let var_expr = BoolExpr::variable(var);
                    if polarity {
                        factors.push(var_expr);
                    } else {
                        factors.push(var_expr.not());
                    }
                }

                let product = factors.into_iter().reduce(|acc, f| acc.and(&f)).unwrap();
                terms.push(product);
            }
        }

        // OR all terms together
        let mut ret = terms.into_iter().reduce(|acc, t| acc.or(&t)).unwrap();

        // populate the cache with the current BDD
        ret.bdd_cache = Arc::new(OnceLock::from(self.clone()));
        ret
    }

    /// Check if this BDD is a terminal (constant)
    pub fn is_terminal(&self) -> bool {
        self.root == TRUE_NODE || self.root == FALSE_NODE
    }

    /// Check if this BDD represents TRUE
    pub fn is_true(&self) -> bool {
        self.root == TRUE_NODE
    }

    /// Check if this BDD represents FALSE
    pub fn is_false(&self) -> bool {
        self.root == FALSE_NODE
    }

    /// Get the number of nodes in this BDD
    pub fn node_count(&self) -> usize {
        self.count_reachable_nodes(self.root, &mut HashMap::new())
    }

    /// Count reachable nodes from a given root
    fn count_reachable_nodes(&self, node: NodeId, visited: &mut HashMap<NodeId, ()>) -> usize {
        if visited.contains_key(&node) {
            return 0;
        }
        visited.insert(node, ());

        // Acquire lock, extract needed data, then release before recursing.
        // This is safe because NodeIds are stable (nodes are never removed/reordered).
        let (is_terminal, low, high) = {
            let inner = self.manager.read().unwrap();
            match inner.get_node(node) {
                Some(BddNode::Terminal(_)) => (true, 0, 0),
                Some(BddNode::Decision { low, high, .. }) => (false, *low, *high),
                None => {
                    panic!("Invalid node ID {} encountered during node counting - this indicates a bug in the BDD implementation", node);
                }
            }
        }; // Lock released here

        if is_terminal {
            1
        } else {
            1 + self.count_reachable_nodes(low, visited) + self.count_reachable_nodes(high, visited)
        }
    }

    /// Get the variable count (number of distinct variables)
    pub fn var_count(&self) -> usize {
        let mut vars = std::collections::HashSet::new();
        self.collect_vars(self.root, &mut vars);
        vars.len()
    }

    /// Collect all variables reachable from a node
    fn collect_vars(&self, node: NodeId, vars: &mut std::collections::HashSet<VarId>) {
        // Acquire lock, extract needed data, then release before recursing.
        // This is safe because NodeIds are stable (nodes are never removed/reordered).
        let node_info = {
            let inner = self.manager.read().unwrap();
            match inner.get_node(node) {
                Some(BddNode::Terminal(_)) => None,
                Some(BddNode::Decision { var, low, high }) => Some((*var, *low, *high)),
                None => {
                    panic!("Invalid node ID {} encountered during variable collection - this indicates a bug in the BDD implementation", node);
                }
            }
        }; // Lock released here

        if let Some((var, low, high)) = node_info {
            if vars.insert(var) {
                self.collect_vars(low, vars);
                self.collect_vars(high, vars);
            }
        }
    }

    /// Extract cubes (product terms) from the BDD
    ///
    /// Returns a vector of cubes, where each cube is a map from variable name to
    /// its literal value (true for positive literal, false for negative literal).
    ///
    /// Each cube represents one path from the root to the TRUE terminal.
    ///
    /// **Internal use only.** Public API should use `Dnf::from(&bdd)` instead.
    pub(crate) fn to_cubes(&self) -> Vec<BTreeMap<Arc<str>, bool>> {
        let mut results = Vec::new();
        let mut current_path = BTreeMap::new();
        self.extract_cubes(self.root, &mut current_path, &mut results);
        results
    }

    /// Extract cubes recursively by traversing the BDD
    fn extract_cubes(
        &self,
        node: NodeId,
        current_path: &mut BTreeMap<Arc<str>, bool>,
        results: &mut Vec<BTreeMap<Arc<str>, bool>>,
    ) {
        // Acquire lock, extract needed data, then release before recursing.
        // This is safe because NodeIds are stable (nodes are never removed/reordered).
        let node_info = {
            let inner = self.manager.read().unwrap();
            match inner.get_node(node) {
                Some(BddNode::Terminal(true)) => Some((true, None)),
                Some(BddNode::Terminal(false)) => Some((false, None)),
                Some(BddNode::Decision { var, low, high }) => {
                    let var_name = inner.var_name(*var)
                        .expect("Invalid variable ID encountered during cube extraction - this indicates a bug in the BDD implementation");
                    Some((false, Some((Arc::clone(var_name), *low, *high))))
                }
                None => {
                    panic!("Invalid node ID {} encountered during cube extraction - this indicates a bug in the BDD implementation", node);
                }
            }
        }; // Lock released here

        match node_info {
            Some((true, None)) => {
                // Reached TRUE terminal - add current path as a cube
                results.push(current_path.clone());
            }
            Some((false, None)) => {
                // Reached FALSE terminal - this path doesn't contribute
            }
            Some((false, Some((var_name, low, high)))) => {
                // Traverse low edge (var = false)
                current_path.insert(Arc::clone(&var_name), false);
                self.extract_cubes(low, current_path, results);
                current_path.remove(&var_name);

                // Traverse high edge (var = true)
                current_path.insert(Arc::clone(&var_name), true);
                self.extract_cubes(high, current_path, results);
                current_path.remove(&var_name);
            }
            _ => unreachable!(),
        }
    }

    /// Logical AND operation
    ///
    /// Computes the conjunction of two BDDs using the ITE operation:
    /// `and(f, g) = ite(f, g, false)`
    pub fn and(&self, other: &Bdd) -> Bdd {
        // Use ITE: and(f, g) = ite(f, g, false)
        // Clone manager from self to avoid mutex access
        let manager = Arc::clone(&self.manager);
        let result = manager
            .write()
            .unwrap()
            .ite(self.root, other.root, FALSE_NODE);
        Bdd {
            manager,
            root: result,
        }
    }

    /// Logical OR operation
    ///
    /// Computes the disjunction of two BDDs using the ITE operation:
    /// `or(f, g) = ite(f, true, g)`
    pub fn or(&self, other: &Bdd) -> Bdd {
        // Use ITE: or(f, g) = ite(f, true, g)
        // Clone manager from self to avoid mutex access
        let manager = Arc::clone(&self.manager);
        let result = manager
            .write()
            .unwrap()
            .ite(self.root, TRUE_NODE, other.root);
        Bdd {
            manager,
            root: result,
        }
    }

    /// Logical NOT operation
    ///
    /// Computes the negation of a BDD using the ITE operation:
    /// `not(f) = ite(f, false, true)`
    pub fn not(&self) -> Bdd {
        // Use ITE: not(f) = ite(f, false, true)
        // Clone manager from self to avoid mutex access
        let manager = Arc::clone(&self.manager);
        let result = manager
            .write()
            .unwrap()
            .ite(self.root, FALSE_NODE, TRUE_NODE);
        Bdd {
            manager,
            root: result,
        }
    }
}

impl PartialEq for Bdd {
    fn eq(&self, other: &Self) -> bool {
        // BDDs are equal if they share the same manager and have the same root node
        // The singleton manager ensures consistent representation across all BDDs
        Arc::ptr_eq(&self.manager, &other.manager) && self.root == other.root
    }
}

impl Eq for Bdd {}

// Note: Blanket Minimizable implementation has been moved to cover/dnf.rs
// to operate on types convertible to/from Dnf instead of Bdd.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_nodes() {
        let t = Bdd::constant(true);
        let f = Bdd::constant(false);

        assert!(t.is_true());
        assert!(!t.is_false());
        assert!(f.is_false());
        assert!(!f.is_true());
        assert!(t.is_terminal());
        assert!(f.is_terminal());
    }

    #[test]
    fn test_variable_creation() {
        let a = Bdd::variable("a");
        let b = Bdd::variable("b");

        assert!(!a.is_terminal());
        assert!(!b.is_terminal());
        assert_ne!(a, b);
    }

    #[test]
    fn test_ite_terminal_cases() {
        let t = Bdd::constant(true);
        let f = Bdd::constant(false);
        let a = Bdd::variable("a");

        // Test basic operations which are implemented via ITE internally
        // a AND true = a
        let result = a.and(&t);
        assert_eq!(result, a);

        // a AND false = false
        let result = a.and(&f);
        assert_eq!(result, f);

        // a OR true = true
        let result = a.or(&t);
        assert_eq!(result, t);

        // a OR false = a
        let result = a.or(&f);
        assert_eq!(result, a);
    }

    #[test]
    fn test_node_count() {
        let t = Bdd::constant(true);
        assert_eq!(t.node_count(), 1);

        let a = Bdd::variable("a");
        // Variable node: 1 decision node + 2 terminal nodes
        assert_eq!(a.node_count(), 3);
    }

    #[test]
    fn test_var_count() {
        let t = Bdd::constant(true);
        assert_eq!(t.var_count(), 0);

        let a = Bdd::variable("a");
        assert_eq!(a.var_count(), 1);
    }

    #[test]
    fn test_hash_consing() {
        let a1 = Bdd::variable("a");
        let a2 = Bdd::variable("a");

        // Same variable should produce same node (hash consing)
        assert_eq!(a1, a2);
    }

    #[test]
    fn test_and_operation() {
        let t = Bdd::constant(true);
        let f = Bdd::constant(false);
        let a = Bdd::variable("a");
        let b = Bdd::variable("b");

        // Test terminal cases
        assert_eq!(a.and(&t), a); // a AND true = a
        assert!(a.and(&f).is_false()); // a AND false = false
        assert_eq!(t.and(&a), a); // true AND a = a
        assert!(f.and(&a).is_false()); // false AND a = false

        // Test with variables
        let result = a.and(&b);
        assert!(!result.is_terminal());
        assert!(!result.is_true());
        assert!(!result.is_false());

        // a AND a = a (idempotent)
        let result = a.and(&a);
        assert_eq!(result, a);
    }

    #[test]
    fn test_or_operation() {
        let t = Bdd::constant(true);
        let f = Bdd::constant(false);
        let a = Bdd::variable("a");
        let b = Bdd::variable("b");

        // Test terminal cases
        assert_eq!(a.or(&f), a); // a OR false = a
        assert!(a.or(&t).is_true()); // a OR true = true
        assert_eq!(f.or(&a), a); // false OR a = a
        assert!(t.or(&a).is_true()); // true OR a = true

        // Test with variables
        let result = a.or(&b);
        assert!(!result.is_terminal());

        // a OR a = a (idempotent)
        let result = a.or(&a);
        assert_eq!(result, a);
    }

    #[test]
    fn test_not_operation() {
        let t = Bdd::constant(true);
        let f = Bdd::constant(false);
        let a = Bdd::variable("a");

        // Test terminal cases
        assert!(t.not().is_false()); // NOT true = false
        assert!(f.not().is_true()); // NOT false = true

        // Test double negation
        let not_a = a.not();
        assert!(!not_a.is_terminal());
        let not_not_a = not_a.not();
        assert_eq!(not_not_a, a); // NOT NOT a = a
    }

    #[test]
    fn test_and_or_combination() {
        let a = Bdd::variable("a");
        let b = Bdd::variable("b");

        // (a AND b) OR (a AND b) = a AND b (idempotent)
        let ab = a.and(&b);
        let result = ab.or(&ab);
        assert_eq!(result, ab);

        // (a OR b) AND (a OR b) = a OR b (idempotent)
        let a_or_b = a.or(&b);
        let result = a_or_b.and(&a_or_b);
        assert_eq!(result, a_or_b);
    }

    #[test]
    fn test_de_morgans_laws() {
        let a = Bdd::variable("a");
        let b = Bdd::variable("b");

        // NOT(a AND b) = (NOT a) OR (NOT b)
        let not_ab = a.and(&b).not();
        let not_a_or_not_b = a.not().or(&b.not());
        assert_eq!(not_ab, not_a_or_not_b);

        // NOT(a OR b) = (NOT a) AND (NOT b)
        let not_a_or_b = a.or(&b).not();
        let not_a_and_not_b = a.not().and(&b.not());
        assert_eq!(not_a_or_b, not_a_and_not_b);
    }

    #[test]
    fn test_commutativity() {
        let a = Bdd::variable("a");
        let b = Bdd::variable("b");

        // a AND b = b AND a
        let ab = a.and(&b);
        let ba = b.and(&a);
        assert_eq!(ab, ba);

        // a OR b = b OR a
        let a_or_b = a.or(&b);
        let b_or_a = b.or(&a);
        assert_eq!(a_or_b, b_or_a);
    }

    #[test]
    fn test_associativity() {
        let a = Bdd::variable("a");
        let b = Bdd::variable("b");
        let c = Bdd::variable("c");

        // (a AND b) AND c = a AND (b AND c)
        let ab_and_c = a.and(&b).and(&c);
        let a_and_bc = a.and(&b.and(&c));
        assert_eq!(ab_and_c, a_and_bc);

        // (a OR b) OR c = a OR (b OR c)
        let ab_or_c = a.or(&b).or(&c);
        let a_or_bc = a.or(&b.or(&c));
        assert_eq!(ab_or_c, a_or_bc);
    }

    #[test]
    fn test_distributivity() {
        let a = Bdd::variable("a");
        let b = Bdd::variable("b");
        let c = Bdd::variable("c");

        // a AND (b OR c) = (a AND b) OR (a AND c)
        let a_and_bc = a.and(&b.or(&c));
        let ab_or_ac = a.and(&b).or(&a.and(&c));
        assert_eq!(a_and_bc, ab_or_ac);

        // a OR (b AND c) = (a OR b) AND (a OR c)
        let a_or_bc = a.or(&b.and(&c));
        let ab_or_ac = a.or(&b).and(&a.or(&c));
        assert_eq!(a_or_bc, ab_or_ac);
    }

    #[test]
    fn test_to_cubes_simple() {
        let a = Bdd::variable("a");
        let b = Bdd::variable("b");

        // a AND b should produce one cube: {a: true, b: true}
        let ab = a.and(&b);
        let cubes = ab.to_cubes();
        assert_eq!(cubes.len(), 1);
        assert_eq!(cubes[0].get(&Arc::from("a")), Some(&true));
        assert_eq!(cubes[0].get(&Arc::from("b")), Some(&true));
    }

    #[test]
    fn test_to_cubes_or() {
        let a = Bdd::variable("a");
        let b = Bdd::variable("b");

        // a OR b should produce two cubes
        let a_or_b = a.or(&b);
        let cubes = a_or_b.to_cubes();
        assert_eq!(cubes.len(), 2);
    }

    #[test]
    fn test_to_cubes_constant() {
        let t = Bdd::constant(true);
        let f = Bdd::constant(false);

        // TRUE should produce one empty cube (tautology)
        let cubes = t.to_cubes();
        assert_eq!(cubes.len(), 1);
        assert!(cubes[0].is_empty());

        // FALSE should produce no cubes
        let cubes = f.to_cubes();
        assert_eq!(cubes.len(), 0);
    }

    #[test]
    fn test_to_cubes_complex() {
        let a = Bdd::variable("a");
        let b = Bdd::variable("b");
        let c = Bdd::variable("c");

        // (a AND b) OR (b AND c) OR (a AND c) - majority function
        let ab = a.and(&b);
        let bc = b.and(&c);
        let ac = a.and(&c);
        let majority = ab.or(&bc).or(&ac);

        let cubes = majority.to_cubes();
        // Should produce 3 cubes for the three products
        assert!(cubes.len() >= 2); // BDD may optimize this
        assert!(cubes.len() <= 3);
    }

    #[test]
    fn test_roundtrip_bdd_expr() {
        use crate::expression::BoolExpr;

        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.and(&b);

        // Convert to BDD and back
        let bdd = expr.to_bdd();
        let expr2 = bdd.to_expr();

        // Should be logically equivalent
        assert!(expr.equivalent_to(&expr2));
    }

    #[test]
    fn test_bdd_from_expr() {
        use crate::expression::BoolExpr;

        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let expr = a.and(&b);

        // Test both conversion methods
        let bdd1 = expr.to_bdd();
        let bdd2 = Bdd::from_expr(&expr);

        // Both should produce equivalent BDDs
        assert_eq!(bdd1.node_count(), bdd2.node_count());
    }

    #[test]
    fn test_bdd_consensus_theorem() {
        use crate::expression::BoolExpr;

        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        // Consensus theorem: a*b + ~a*c + b*c
        // The b*c term is redundant
        let expr = a.and(&b).or(&a.not().and(&c)).or(&b.and(&c));
        let bdd = expr.to_bdd();
        let cubes = bdd.to_cubes();

        // BDD should recognize that b*c is redundant and produce only 2 cubes
        assert_eq!(cubes.len(), 2);
    }

    #[test]
    fn test_bdd_xor() {
        use crate::expression::BoolExpr;

        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        // XOR: a*~b + ~a*b
        let xor = a.and(&b.not()).or(&a.not().and(&b));
        let bdd = xor.to_bdd();
        let cubes = bdd.to_cubes();

        // Should produce 2 cubes
        assert_eq!(cubes.len(), 2);

        // Convert back and verify equivalence
        let expr2 = bdd.to_expr();
        assert!(xor.equivalent_to(&expr2));
    }

    #[test]
    fn test_global_manager_sharing() {
        use crate::expression::BoolExpr;

        // Create multiple BDDs
        let a1 = BoolExpr::variable("a");
        let a2 = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        let bdd1 = a1.to_bdd();
        let bdd2 = a2.to_bdd();
        let bdd3 = b.to_bdd();

        // All BDDs should share the same manager (Arc pointer equality)
        assert!(Arc::ptr_eq(&bdd1.manager, &bdd2.manager));
        assert!(Arc::ptr_eq(&bdd1.manager, &bdd3.manager));

        // Same expressions should produce identical BDDs (hash consing works globally)
        assert_eq!(bdd1, bdd2);
    }
}
