//! Evaluation and equivalence checking for boolean expressions

use super::manager::{BddNode, NodeId};
use super::BoolExpr;
use std::collections::HashMap;
use std::sync::Arc;

impl BoolExpr {
    /// Check if two boolean expressions are logically equivalent
    ///
    /// Uses a two-phase BDD-based approach (v3.1+):
    /// 1. **Fast BDD equality check** - Convert both expressions to BDDs and compare. BDDs use
    ///    canonical representation, so equal BDDs guarantee equivalence. Very fast (O(e) where
    ///    e is expression size).
    /// 2. **Exact minimization fallback** - If BDDs differ, use exact minimization for thorough
    ///    verification. This handles edge cases and provides definitive results.
    ///
    /// Much more efficient than exhaustive truth table comparison for expressions with many variables.
    ///
    /// # Performance
    ///
    /// - **Typical case**: O(n) where n is the number of variables (BDD check)
    /// - **Worst case**: O(2^n) for exact minimization fallback (rare)
    ///
    /// Most equivalences are determined by the fast BDD check. The exact minimization
    /// fallback only runs when BDD representations differ.
    ///
    /// # Performance Comparison to v3.0
    ///
    /// Version 3.1+ uses BDD-based equivalence checking (via lazy caching) which provides:
    ///
    /// - **First call on expression**: O(n × m) where n = vars, m = nodes in expression tree
    /// - **Subsequent calls**: O(1) thanks to BDD caching
    /// - **Minimization fallback**: O(m × k) where m is cubes and k is variables
    /// - **Old approach (v3.0)**: O(2^n) where n is the number of variables (exponential)
    ///
    /// This makes equivalency checking **dramatically faster** for expressions with many variables:
    /// - 10 variables: 1,024× faster
    /// - 20 variables: 1,048,576× faster
    /// - 30 variables: Previously impossible, now feasible
    ///
    /// [`Bdd`]: crate::bdd::Bdd
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    ///
    /// // Different structures, same logic
    /// let expr1 = a.and(&b);
    /// let expr2 = b.and(&a); // Commutative
    ///
    /// assert!(expr1.equivalent_to(&expr2));
    /// ```
    pub fn equivalent_to(&self, other: &BoolExpr) -> bool {
        use crate::{Cover, CoverType};

        // Handle constant expressions specially
        let self_vars = self.collect_variables();
        let other_vars = other.collect_variables();

        if self_vars.is_empty() && other_vars.is_empty() {
            // Both are constants - just evaluate
            return self.evaluate(&HashMap::new()) == other.evaluate(&HashMap::new());
        }

        // OPTIMIZATION: First try BDD equality check (fast)
        // BDDs use canonical representation, so equal BDDs mean equivalent functions
        let self_bdd = self;
        let other_bdd = other;

        if self_bdd == other_bdd {
            // BDDs are equal - expressions are definitely equivalent
            return true;
        }

        // BDDs differ - fall back to exact minimization for thorough verification
        // This handles edge cases where BDD construction might differ but functions are still equivalent
        let mut cover = Cover::new(CoverType::F);

        // Add both BDDs as separate outputs
        if cover.add_expr(&self_bdd, "expr1").is_err() {
            return false;
        }
        if cover.add_expr(&other_bdd, "expr2").is_err() {
            return false;
        }

        // Minimize exactly once - if this fails, assume not equivalent
        use crate::cover::Minimizable as _;
        cover = match cover.minimize_exact() {
            Ok(minimized) => minimized,
            Err(_) => return false,
        };

        // Check if all cubes have identical output patterns for both outputs
        // After exact minimization, if the expressions are equivalent, every cube
        // will have the same value for both outputs (both 0 or both 1)
        for cube in cover.cubes() {
            let outputs = cube.outputs();
            if outputs.len() >= 2 && outputs[0] != outputs[1] {
                return false;
            }
        }

        true
    }

    /// Evaluate the expression with a given variable assignment
    ///
    /// Traverses the BDD following the variable assignments until reaching a terminal node.
    /// Returns the boolean value of the terminal node.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    /// use std::collections::HashMap;
    /// use std::sync::Arc;
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b);
    ///
    /// let mut assignment = HashMap::new();
    /// assignment.insert(Arc::from("a"), true);
    /// assignment.insert(Arc::from("b"), true);
    ///
    /// assert_eq!(expr.evaluate(&assignment), true);
    ///
    /// assignment.insert(Arc::from("b"), false);
    /// assert_eq!(expr.evaluate(&assignment), false);
    /// ```
    pub fn evaluate(&self, assignment: &HashMap<Arc<str>, bool>) -> bool {
        self.evaluate_node(self.root, assignment)
    }

    /// Recursively evaluate a BDD node
    fn evaluate_node(&self, node_id: NodeId, assignment: &HashMap<Arc<str>, bool>) -> bool {
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
}
