//! Evaluation of [`BoolExpr`] under a variable assignment.

use super::rpn;
use super::BoolExpr;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

impl BoolExpr {
    /// Evaluate the expression under a variable assignment.
    ///
    /// Folds the reverse-Polish token stream with an explicit value stack (no recursion), so an
    /// arbitrarily deep expression cannot overflow the call stack. The map key can be any
    /// `Borrow<str>` (`&str`, `String`, `Symbol`, `Arc<str>`, …).
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
    /// let expr = BoolExpr::var("a") & BoolExpr::var("b");
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
        rpn::evaluate(self.tokens(), assignment)
    }
}
