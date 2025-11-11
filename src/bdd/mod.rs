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
use std::sync::{Arc, Mutex, RwLock, Weak};

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
    /// Cache for DNF: NodeId -> Weak<Dnf>
    /// Weak references allow sharing DNF across BDDs without preventing cleanup
    dnf_cache: HashMap<NodeId, Weak<crate::cover::Dnf>>,
    /// Cache for factorized ASTs: NodeId -> Weak<BoolExprAst>
    /// Weak references allow sharing factorized ASTs without preventing cleanup
    ast_cache: HashMap<NodeId, Weak<crate::expression::BoolExprAst>>,
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
                dnf_cache: HashMap::new(),
                ast_cache: HashMap::new(),
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
    /// Cached DNF (cubes) for this BDD, kept alive with strong reference
    /// This avoids expensive BDD traversal when converting to expressions
    /// Uses OnceLock for interior mutability across clones
    dnf_cache: std::sync::OnceLock<Arc<crate::cover::Dnf>>,
}

impl Bdd {
    /// Create a BDD representing a constant
    pub fn constant(value: bool) -> Self {
        let manager = BddManager::get_or_create();
        Bdd {
            manager,
            root: if value { TRUE_NODE } else { FALSE_NODE },
            dnf_cache: std::sync::OnceLock::new(),
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
            dnf_cache: std::sync::OnceLock::new(),
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
    /// Since BoolExpr now uses BDD as primary storage, this is a simple wrapper.
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
        crate::expression::BoolExpr::from(self.clone())
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
        self.collect_var_ids(self.root, &mut vars);
        vars.len()
    }

    /// Collect all variable IDs reachable from a node
    fn collect_var_ids(&self, node: NodeId, vars: &mut std::collections::HashSet<VarId>) {
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
                self.collect_var_ids(low, vars);
                self.collect_var_ids(high, vars);
            }
        }
    }

    /// Collect all variable names used in this BDD
    ///
    /// Returns a BTreeSet of variable names in alphabetical order.
    /// This is useful for determining which variables a boolean function depends on.
    pub fn collect_variables(&self) -> std::collections::BTreeSet<std::sync::Arc<str>> {
        let mut var_ids = std::collections::HashSet::new();
        self.collect_var_ids(self.root, &mut var_ids);

        // Convert var IDs to names
        let mgr = self.manager.read().unwrap();
        var_ids
            .into_iter()
            .filter_map(|id| mgr.var_name(id).cloned())
            .collect()
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
            dnf_cache: std::sync::OnceLock::new(),
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
            dnf_cache: std::sync::OnceLock::new(),
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
            dnf_cache: std::sync::OnceLock::new(),
        }
    }

    /// Evaluate the BDD with a given variable assignment
    ///
    /// Traverses the BDD following the variable assignments until reaching a terminal node.
    /// Returns the boolean value of the terminal node.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Bdd;
    /// use std::collections::HashMap;
    /// use std::sync::Arc;
    ///
    /// let a = Bdd::variable("a");
    /// let b = Bdd::variable("b");
    /// let bdd = a.and(&b);
    ///
    /// let mut assignment = HashMap::new();
    /// assignment.insert(Arc::from("a"), true);
    /// assignment.insert(Arc::from("b"), true);
    ///
    /// assert_eq!(bdd.evaluate(&assignment), true);
    ///
    /// assignment.insert(Arc::from("b"), false);
    /// assert_eq!(bdd.evaluate(&assignment), false);
    /// ```
    pub fn evaluate(
        &self,
        assignment: &std::collections::HashMap<std::sync::Arc<str>, bool>,
    ) -> bool {
        self.evaluate_node(self.root, assignment)
    }

    /// Recursively evaluate a BDD node
    fn evaluate_node(
        &self,
        node_id: NodeId,
        assignment: &std::collections::HashMap<std::sync::Arc<str>, bool>,
    ) -> bool {
        // Acquire lock, extract needed data, then release before recursing
        let node_info = {
            let mgr = self.manager.read().unwrap();
            match mgr.get_node(node_id) {
                Some(BddNode::Terminal(val)) => (true, *val, 0, 0, None),
                Some(BddNode::Decision { var, low, high }) => {
                    let var_name = mgr
                        .var_name(*var)
                        .expect("Invalid variable ID in BDD evaluation");
                    (false, false, *low, *high, Some(Arc::clone(var_name)))
                }
                None => panic!("Invalid node ID {} in BDD evaluation", node_id),
            }
        }; // Lock released here

        match node_info {
            (true, val, _, _, _) => val, // Terminal node
            (false, _, low, high, Some(var_name)) => {
                // Decision node: follow edge based on variable value
                let var_value = assignment.get(&var_name).copied().unwrap_or(false);
                if var_value {
                    self.evaluate_node(high, assignment)
                } else {
                    self.evaluate_node(low, assignment)
                }
            }
            _ => unreachable!(),
        }
    }

    /// Convert BDD to optimised AST representation using factorization
    ///
    /// Extracts cubes from the BDD and applies algebraic factorization to produce
    /// a compact, readable expression.
    ///
    /// Uses two-level caching for both DNF and AST:
    /// 1. Bdd's own dnf_cache (strong reference, lives with this Bdd)
    /// 2. BddManager's dnf_cache and ast_cache (weak references, shared across BDDs)
    pub(crate) fn to_ast_optimised(&self) -> std::sync::Arc<crate::expression::BoolExprAst> {
        // Check AST cache first (fastest path)
        {
            let mgr = self.manager.read().unwrap();
            if let Some(weak) = mgr.ast_cache.get(&self.root) {
                if let Some(ast) = weak.upgrade() {
                    return ast;
                }
            }
        }

        // AST not cached, get DNF and factorize
        // Level 1: Check if this Bdd instance has cached DNF
        let dnf = if let Some(dnf_arc) = self.dnf_cache.get() {
            // Use cached DNF (clone the Arc, cheap)
            Arc::clone(dnf_arc)
        } else {
            // Level 2: Check manager's DNF cache
            let from_manager = {
                let mgr = self.manager.read().unwrap();
                mgr.dnf_cache
                    .get(&self.root)
                    .and_then(|weak| weak.upgrade())
            };

            if let Some(dnf_arc) = from_manager {
                // Found in manager's cache, store in our cache too
                let _ = self.dnf_cache.set(Arc::clone(&dnf_arc));
                dnf_arc
            } else {
                // Not cached anywhere, extract cubes
                let cubes = self.to_cubes();
                let dnf = Arc::new(crate::cover::Dnf::from_cubes(&cubes));

                // Store in both caches
                let _ = self.dnf_cache.set(Arc::clone(&dnf));
                {
                    let mut mgr = self.manager.write().unwrap();
                    mgr.dnf_cache.insert(self.root, Arc::downgrade(&dnf));
                }

                dnf
            }
        };

        // Convert DNF cubes to the format expected by factorization
        let cube_terms: Vec<(std::collections::BTreeMap<std::sync::Arc<str>, bool>, bool)> = dnf
            .cubes()
            .iter()
            .map(|cube| (cube.clone(), true))
            .collect();

        // Use factorization to build a nice AST
        let ast = crate::expression::factorization::factorise_cubes_to_ast(cube_terms);

        // Cache the AST in the manager for sharing
        {
            let mut mgr = self.manager.write().unwrap();
            mgr.ast_cache.insert(self.root, Arc::downgrade(&ast));
        }

        ast
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
mod tests;
