//! BDD traversal, queries, and analysis operations
//!
//! This module contains methods for traversing the BDD structure, extracting information,
//! and querying properties of boolean expressions.

use super::manager::{BddNode, NodeId, VarId, FALSE_NODE, TRUE_NODE};
use super::BoolExpr;
use crate::cover::{Minterm, Symbols};
use std::collections::{BTreeSet, HashMap};
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
    ///
    /// Deduplicates on visited *nodes* (not variables): a variable can label several distinct
    /// nodes, so stopping at the first occurrence of a variable would miss variables that only
    /// appear deeper in other branches. Tracking visited nodes keeps the walk both complete and
    /// linear in the BDD size.
    fn collect_var_ids(&self, node: NodeId, vars: &mut std::collections::HashSet<VarId>) {
        let mut visited = std::collections::HashSet::new();
        self.collect_var_ids_inner(node, vars, &mut visited);
    }

    /// Recursive helper for [`collect_var_ids`] that tracks already-visited nodes.
    fn collect_var_ids_inner(
        &self,
        node: NodeId,
        vars: &mut std::collections::HashSet<VarId>,
        visited: &mut std::collections::HashSet<NodeId>,
    ) {
        if !visited.insert(node) {
            return;
        }

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
            vars.insert(var);
            self.collect_var_ids_inner(low, vars, visited);
            self.collect_var_ids_inner(high, vars, visited);
        }
    }

    /// Extract cubes (product terms) from the BDD as [`Minterm`]s
    ///
    /// Returns a shared slice of input minterms, each carrying the full (alphabetically sorted)
    /// variable header of the expression. A variable on a cube's path is fixed to `Some(true)`
    /// / `Some(false)`; variables off the path are don't-care (`None`).
    ///
    /// Each minterm represents one path from the root to the TRUE terminal.
    ///
    /// The result is `Arc<[Minterm]>`: extracted minterms are cached, so this is a cheap
    /// reference-count clone of the shared cache rather than a fresh allocation per call.
    pub fn to_cubes(&self) -> Arc<[Minterm]> {
        self.get_or_create_cubes()
    }

    /// Extract cubes directly from BDD via traversal (bypasses cache)
    ///
    /// This is the internal method that actually traverses the BDD structure.
    /// Most code should use `to_cubes()` instead, which uses caching.
    ///
    /// Every returned minterm shares one canonical header `Arc`, so cubes of the same
    /// expression stay on the [`Minterm`] fast-comparison path.
    pub(super) fn extract_cubes_from_bdd(&self) -> Arc<[Minterm]> {
        // Canonical, alphabetically sorted variable header shared by every extracted minterm.
        let vars: Arc<[Arc<str>]> = self.collect_variables().into_iter().collect();
        let index: HashMap<Arc<str>, usize> = vars
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, v)| (v, i))
            .collect();
        let symbols = Symbols::new(vars);

        // The DFS accumulates into a Vec (legit tree-traversal scratch), then freezes into Arc<[]>.
        let mut results = Vec::new();
        // Scratch path indexed by header position; `None` = variable not yet fixed (don't-care).
        let mut path: Vec<Option<bool>> = vec![None; symbols.arity()];
        self.extract_cubes(self.root, &symbols, &index, &mut path, &mut results);
        results.into()
    }

    /// Extract cubes recursively by traversing the BDD
    fn extract_cubes(
        &self,
        node: NodeId,
        symbols: &Arc<Symbols>,
        index: &HashMap<Arc<str>, usize>,
        path: &mut [Option<bool>],
        results: &mut Vec<Minterm>,
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
                // Reached TRUE terminal - materialise the current path as a minterm.
                results.push(Minterm::from_symbols(
                    Arc::clone(symbols),
                    path.iter().copied(),
                ));
            }
            Some((false, None)) => {
                // Reached FALSE terminal - this path doesn't contribute
            }
            Some((false, Some((var_name, low, high)))) => {
                let i = index[&var_name];

                // Traverse low edge (var = false)
                path[i] = Some(false);
                self.extract_cubes(low, symbols, index, path, results);

                // Traverse high edge (var = true)
                path[i] = Some(true);
                self.extract_cubes(high, symbols, index, path, results);

                // Restore don't-care on backtrack.
                path[i] = None;
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
