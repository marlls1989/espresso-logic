//! BDD traversal, queries, and analysis operations
//!
//! This module contains methods for traversing the BDD structure, extracting information,
//! and querying properties of boolean expressions.

use super::manager::{BddNode, NodeId, VarId, FALSE_NODE, TRUE_NODE};
use super::BoolExpr;
use crate::cover::{Minterm, Symbols};
use crate::Symbol;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

impl BoolExpr {
    /// Collect all variables used in this expression in alphabetical order
    ///
    /// Returns a `BTreeSet` which maintains variables in sorted order.
    /// This ordering is used when converting to covers for minimisation.
    pub fn collect_variables(&self) -> BTreeSet<Symbol> {
        let mut var_ids = std::collections::HashSet::new();
        self.collect_var_ids(self.root, &mut var_ids);

        // Convert var IDs to names
        let mgr = self.manager.read().unwrap();
        var_ids
            .into_iter()
            .filter_map(|id| mgr.var_name(id).cloned())
            .collect()
    }

    /// Collect all variable IDs reachable from `root` (iterative DFS over the BDD DAG).
    ///
    /// Deduplicates on visited *nodes* (not variables): a variable can label several distinct
    /// nodes, so stopping at the first occurrence of a variable would miss variables that only
    /// appear deeper in other branches. Tracking visited nodes keeps the walk both complete and
    /// linear in the BDD size. An explicit work-stack avoids unbounded recursion on large BDDs;
    /// one read guard is held for the whole walk (NodeIds are stable).
    fn collect_var_ids(&self, root: NodeId, vars: &mut std::collections::HashSet<VarId>) {
        let mgr = self.manager.read().unwrap();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![root];
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
                None => panic!("Invalid node ID {node} encountered during variable collection - this indicates a bug in the BDD implementation"),
            }
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
    /// The result is `Arc<[Minterm<Symbol>]>`: extracted minterms are cached, so this is a cheap
    /// reference-count clone of the shared cache rather than a fresh allocation per call.
    pub fn to_cubes(&self) -> Arc<[Minterm<Symbol>]> {
        self.get_or_create_cubes()
    }

    /// Extract cubes directly from BDD via traversal (bypasses cache)
    ///
    /// This is the internal method that actually traverses the BDD structure.
    /// Most code should use `to_cubes()` instead, which uses caching.
    ///
    /// Every returned minterm shares one canonical header `Arc`, so cubes of the same
    /// expression stay on the [`Minterm`] fast-comparison path.
    pub(super) fn extract_cubes_from_bdd(&self) -> Arc<[Minterm<Symbol>]> {
        // Canonical, alphabetically sorted variable header shared by every extracted minterm.
        let vars: Arc<[Symbol]> = self.collect_variables().into_iter().collect();
        let index: HashMap<Symbol, usize> = vars
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, v)| (v, i))
            .collect();
        let symbols = Symbols::new(vars);

        // Iterative DFS enumerating every root->TRUE path (an explicit work-stack rather than
        // recursion, so a deep BDD can't overflow the stack). `SetPath` items replay the recursive
        // "fix this header slot on descent, restore it on backtrack" around the two child visits;
        // because the stack is LIFO they fire in the same order the recursion did. One read guard
        // is held for the whole walk (NodeIds are stable). Stack depth is O(BDD height).
        enum Work {
            Node(NodeId),
            SetPath(usize, Option<bool>),
        }

        let mut results = Vec::new();
        // Scratch path indexed by header position; `None` = variable not yet fixed (don't-care).
        let mut path: Vec<Option<bool>> = vec![None; symbols.arity()];

        let mgr = self.manager.read().unwrap();
        let mut stack = vec![Work::Node(self.root)];
        while let Some(work) = stack.pop() {
            match work {
                Work::SetPath(i, value) => path[i] = value,
                Work::Node(node) => match mgr.get_node(node) {
                    // TRUE terminal - materialise the current path as a minterm.
                    Some(BddNode::Terminal(true)) => {
                        results.push(Minterm::from_symbols(Arc::clone(&symbols), path.iter().copied()));
                    }
                    // FALSE terminal - this path doesn't contribute.
                    Some(BddNode::Terminal(false)) => {}
                    Some(BddNode::Decision { var, low, high }) => {
                        let var_name = mgr.var_name(*var).expect("Invalid variable ID encountered during cube extraction - this indicates a bug in the BDD implementation");
                        let i = *index.get(var_name).expect("BDD variable absent from the collected header - this indicates a bug in the BDD implementation");
                        // Recursion was: path[i]=false; visit(low); path[i]=true; visit(high); path[i]=None.
                        // Push in reverse so LIFO pops reproduce that exact order.
                        stack.push(Work::SetPath(i, None));
                        stack.push(Work::Node(*high));
                        stack.push(Work::SetPath(i, Some(true)));
                        stack.push(Work::Node(*low));
                        stack.push(Work::SetPath(i, Some(false)));
                    }
                    None => panic!("Invalid node ID {node} encountered during cube extraction - this indicates a bug in the BDD implementation"),
                },
            }
        }
        results.into()
    }

    /// Check if this expression is a terminal (constant)
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.root == TRUE_NODE || self.root == FALSE_NODE
    }

    /// Check if this expression represents TRUE
    #[must_use]
    pub fn is_true(&self) -> bool {
        self.root == TRUE_NODE
    }

    /// Check if this expression represents FALSE
    #[must_use]
    pub fn is_false(&self) -> bool {
        self.root == FALSE_NODE
    }

    /// Get the number of (distinct, reachable) nodes in this BDD representation.
    ///
    /// Iterative DFS over the BDD DAG, counting each reachable node once; one read guard for the
    /// whole walk (NodeIds are stable).
    #[must_use]
    pub fn node_count(&self) -> usize {
        let mgr = self.manager.read().unwrap();
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
                None => panic!("Invalid node ID {node} encountered during node counting - this indicates a bug in the BDD implementation"),
            }
        }
        count
    }

    /// Get the variable count (number of distinct variables)
    #[must_use]
    pub fn var_count(&self) -> usize {
        let mut vars = std::collections::HashSet::new();
        self.collect_var_ids(self.root, &mut vars);
        vars.len()
    }
}
