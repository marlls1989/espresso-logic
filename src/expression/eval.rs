//! Evaluation and equivalence checking for boolean expressions

use super::manager::{BddNode, NodeId};
use super::BoolExpr;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

impl BoolExpr {
    /// Check whether two boolean expressions are logically equivalent.
    ///
    /// Every `BoolExpr` is a root into one shared, **canonical** reduced-ordered BDD (all
    /// expressions live in the same global manager), so two expressions denote the same function
    /// **iff their BDD roots are equal**. Equivalence is therefore an exact, constant-time check —
    /// identical to the [`==`](BoolExpr) operator — with no fallible step and no exponential
    /// truth-table evaluation.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    ///
    /// // Different structure, same logic (commutativity).
    /// assert!(a.and(&b).equivalent_to(&b.and(&a)));
    /// ```
    #[must_use]
    pub fn equivalent_to(&self, other: &BoolExpr) -> bool {
        self == other
    }

    /// Evaluate the expression under a variable assignment.
    ///
    /// Follows the BDD from the root, taking the high edge where a variable is `true` and the low
    /// edge where it is `false`, until a terminal is reached. The map key can be any `Borrow<str>`
    /// (`&str`, `String`, `Symbol`, `Arc<str>`, …).
    ///
    /// **A variable not present in `assignment` is treated as `false`** (partial assignments are
    /// allowed; unspecified inputs default to `false`).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    /// use std::collections::HashMap;
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b);
    ///
    /// let mut assignment: HashMap<&str, bool> = HashMap::new();
    /// assignment.insert("a", true);
    /// assignment.insert("b", true);
    /// assert_eq!(expr.evaluate(&assignment), true);
    ///
    /// assignment.insert("b", false);
    /// assert_eq!(expr.evaluate(&assignment), false);
    ///
    /// // Omitted variable defaults to false.
    /// let only_a: HashMap<&str, bool> = HashMap::from([("a", true)]);
    /// assert_eq!(expr.evaluate(&only_a), false);
    /// ```
    #[must_use]
    pub fn evaluate<K>(&self, assignment: &HashMap<K, bool>) -> bool
    where
        K: Borrow<str> + Eq + Hash,
    {
        self.evaluate_node(self.root, assignment)
    }

    /// Evaluate a BDD node under `assignment` by following one edge per level.
    ///
    /// Evaluation visits a single root-to-terminal path (the edge taken at each decision node is
    /// fixed by the variable's value), so this is a plain loop — no recursion, no stack. One read
    /// guard for the whole descent (NodeIds are stable). An unassigned variable reads as `false`
    /// (see [`evaluate`](Self::evaluate)).
    fn evaluate_node<K>(&self, node_id: NodeId, assignment: &HashMap<K, bool>) -> bool
    where
        K: Borrow<str> + Eq + Hash,
    {
        let mgr = self.manager.read().unwrap();
        let mut node_id = node_id;
        loop {
            match mgr.get_node(node_id) {
                Some(BddNode::Terminal(val)) => return *val,
                Some(BddNode::Decision { var, low, high }) => {
                    let var_name = mgr
                        .var_name(*var)
                        .expect("Invalid variable ID in BDD evaluation");
                    let var_value = assignment.get(var_name.as_str()).copied().unwrap_or(false);
                    node_id = if var_value { *high } else { *low };
                }
                None => panic!("Invalid node ID {node_id} in BDD evaluation"),
            }
        }
    }
}
