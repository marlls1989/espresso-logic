//! BDD traversal, queries, and analysis operations
//!
//! This module contains methods for traversing the BDD structure, extracting information,
//! and querying properties of boolean expressions.

use super::manager::{BddNode, NodeId, VarId, FALSE_NODE, TRUE_NODE};
use super::BoolExpr;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

impl BoolExpr {
    /// Collect all variables used in this expression in alphabetical order
    ///
    /// Returns a `BTreeSet` which maintains variables in sorted order.
    /// This ordering is used when converting to covers for minimisation.
    pub fn collect_variables(&self) -> BTreeSet<Arc<str>> {
        let mut var_ids = std::collections::HashSet::new();
        self.collect_var_ids(self.root, &mut var_ids);

        // Convert var IDs to names
        let mgr = self.manager.read().unwrap();
        var_ids
            .into_iter()
            .filter_map(|id| mgr.var_name(id).cloned())
            .collect()
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

    /// Extract cubes (product terms) from the BDD using cached DNF
    ///
    /// Returns a vector of cubes, where each cube is a map from variable name to
    /// its literal value (true for positive literal, false for negative literal).
    ///
    /// Each cube represents one path from the root to the TRUE terminal.
    ///
    /// This method uses the DNF cache to avoid expensive BDD traversal.
    pub fn to_cubes(&self) -> Vec<BTreeMap<Arc<str>, bool>> {
        let dnf = self.get_or_create_dnf();
        dnf.cubes().to_vec()
    }

    /// Extract cubes directly from BDD via traversal (bypasses cache)
    ///
    /// This is the internal method that actually traverses the BDD structure.
    /// Most code should use `to_cubes()` instead, which uses caching.
    pub(super) fn extract_cubes_from_bdd(&self) -> Vec<BTreeMap<Arc<str>, bool>> {
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

    /// Check if this expression is a terminal (constant)
    pub fn is_terminal(&self) -> bool {
        self.root == TRUE_NODE || self.root == FALSE_NODE
    }

    /// Check if this expression represents TRUE
    pub fn is_true(&self) -> bool {
        self.root == TRUE_NODE
    }

    /// Check if this expression represents FALSE
    pub fn is_false(&self) -> bool {
        self.root == FALSE_NODE
    }

    /// Get the number of nodes in this BDD representation
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
}
