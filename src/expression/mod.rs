//! Boolean expression types with operator overloading and parsing support
//!
//! This module provides a boolean expression representation that can be constructed
//! programmatically using operator overloading, the `expr!` macro, or parsed from strings.
//! Expressions can be minimized using the Espresso algorithm.
//!
//! # Main Types
//!
//! - [`BoolExpr`] - A boolean expression that supports three construction methods:
//!   1. Method API: `a.and(&b).or(&c)`
//!   2. Operator overloading: `&a * &b + &c`
//!   3. **`expr!` macro**: `expr!(a * b + c)` - Recommended!
//! - [`Bdd`] - Binary Decision Diagram for canonical representation and efficient
//!   operations. Used internally for efficient cover generation during minimization.
//!
//! # Quick Start
//!
//! ## Using the `expr!` Macro (Recommended)
//!
//! The `expr!` macro provides the cleanest syntax with three usage styles:
//!
//! ```
//! use espresso_logic::{BoolExpr, expr};
//!
//! // Style 1: String literals (most concise - no variable declarations!)
//! let xor = expr!("a" * "b" + !"a" * !"b");
//! println!("{}", xor);  // Output: a * b + ~a * ~b
//!
//! // Style 2: Existing BoolExpr variables
//! let a = BoolExpr::variable("a");
//! let b = BoolExpr::variable("b");
//! let expr = expr!(a * b + !a * !b);
//!
//! // Style 3: Mix both
//! let result = expr!(a * "temp" + b);
//! ```
//!
//! ## Parsing from Strings
//!
//! ```
//! use espresso_logic::BoolExpr;
//!
//! # fn main() -> std::io::Result<()> {
//! let expr = BoolExpr::parse("a * b + ~a * ~b")?;
//! let complex = BoolExpr::parse("(a + b) * (c + d)")?;
//! println!("{}", expr);  // Minimal parentheses: a * b + ~a * ~b
//! # Ok(())
//! # }
//! ```
//!
//! ## Minimizing and Evaluating
//!
//! ```
//! use espresso_logic::{BoolExpr, expr, Minimizable};
//! use std::collections::HashMap;
//! use std::sync::Arc;
//!
//! # fn main() -> std::io::Result<()> {
//! let a = BoolExpr::variable("a");
//! let b = BoolExpr::variable("b");
//! let c = BoolExpr::variable("c");
//!
//! // Redundant expression
//! let redundant = expr!(a * b + a * b * c);
//!
//! // Evaluate with specific values
//! let mut assignment = HashMap::new();
//! assignment.insert(Arc::from("a"), true);
//! assignment.insert(Arc::from("b"), true);
//! assignment.insert(Arc::from("c"), false);
//! let result = redundant.evaluate(&assignment);
//! assert_eq!(result, true);
//!
//! // Minimize it (returns new minimized instance)
//! let minimized = redundant.minimize()?;
//! println!("Minimized: {}", minimized);  // Output: a * b
//!
//! // Check logical equivalence
//! let redundant2 = expr!(a * b + a * b * c);
//! assert!(redundant2.equivalent_to(&minimized));
//! # Ok(())
//! # }
//! ```

// Submodules
mod conversions;
mod display;
pub mod error;
mod eval;
mod operators;
mod parser;

pub use error::{ExpressionParseError, ParseBoolExprError};

use crate::bdd::Bdd;
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

/// Inner representation of a boolean expression
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoolExprInner {
    /// A named variable
    Variable(Arc<str>),
    /// Logical AND of two expressions
    And(BoolExpr, BoolExpr),
    /// Logical OR of two expressions
    Or(BoolExpr, BoolExpr),
    /// Logical NOT of an expression
    Not(BoolExpr),
    /// A constant value (true or false)
    Constant(bool),
}

/// A boolean expression that can be manipulated programmatically
///
/// Uses `Arc` internally for efficient cloning. Provides a fluent method-based API
/// and an `expr!` macro for clean syntax.
///
/// # Examples
///
/// # Examples
///
/// ## Method-based API
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = a.and(&b).or(&a.not().and(&b.not()));
/// ```
///
/// ## Using operator overloading (requires explicit &)
/// ```  
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = &a * &b + &(&a).not() * &(&b).not();
/// ```
#[derive(Clone)]
pub struct BoolExpr {
    pub(crate) inner: Arc<BoolExprInner>,
    /// Cached BDD representation (computed lazily on first access)
    pub(crate) bdd_cache: Arc<OnceLock<Bdd>>,
}

impl BoolExpr {
    /// Create a variable expression with the given name
    pub fn variable(name: &str) -> Self {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Variable(Arc::from(name))),
            bdd_cache: Arc::new(OnceLock::new()),
        }
    }

    /// Create a constant expression (true or false)
    pub fn constant(value: bool) -> Self {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Constant(value)),
            bdd_cache: Arc::new(OnceLock::new()),
        }
    }

    /// Collect all variables used in this expression in alphabetical order
    ///
    /// Returns a `BTreeSet` which maintains variables in sorted order.
    /// This ordering is used when converting to covers for minimization.
    pub fn collect_variables(&self) -> BTreeSet<Arc<str>> {
        let mut vars = BTreeSet::new();
        self.collect_variables_impl(&mut vars);
        vars
    }

    /// Convert this boolean expression to a Binary Decision Diagram ([`Bdd`])
    ///
    /// BDDs provide a canonical representation of boolean functions and support
    /// efficient operations. This conversion walks the expression tree and builds
    /// the BDD bottom-up.
    ///
    /// # Caching
    ///
    /// The BDD is cached on first computation, so subsequent calls are O(1).
    /// Subexpressions also use their caches, enabling dynamic programming.
    ///
    /// # Use in Minimization
    ///
    /// When a [`BoolExpr`] is minimized, it is first converted to a [`Bdd`],
    /// then cubes are extracted from the BDD to create a [`Cover`], which is
    /// then minimized by the Espresso algorithm. BDDs enable efficient cover
    /// generation with automatic optimizations.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b);
    ///
    /// let bdd = expr.to_bdd();
    /// // BDD can now be used for efficient operations
    /// ```
    ///
    /// [`Bdd`]: crate::bdd::Bdd
    /// [`Cover`]: crate::Cover
    pub fn to_bdd(&self) -> Bdd {
        self.bdd_cache
            .get_or_init(|| {
                match self.inner.as_ref() {
                    BoolExprInner::Constant(val) => Bdd::constant(*val),
                    BoolExprInner::Variable(name) => Bdd::variable(name),
                    BoolExprInner::And(left, right) => {
                        // Use to_bdd() on subexpressions to leverage their caches
                        let left_bdd = left.to_bdd();
                        let right_bdd = right.to_bdd();
                        left_bdd.and(&right_bdd)
                    }
                    BoolExprInner::Or(left, right) => {
                        // Use to_bdd() on subexpressions to leverage their caches
                        let left_bdd = left.to_bdd();
                        let right_bdd = right.to_bdd();
                        left_bdd.or(&right_bdd)
                    }
                    BoolExprInner::Not(inner) => {
                        // Use to_bdd() on subexpression to leverage its cache
                        let inner_bdd = inner.to_bdd();
                        inner_bdd.not()
                    }
                }
            })
            .clone()
    }

    fn collect_variables_impl(&self, vars: &mut BTreeSet<Arc<str>>) {
        match self.inner.as_ref() {
            BoolExprInner::Variable(name) => {
                vars.insert(Arc::clone(name));
            }
            BoolExprInner::And(left, right) | BoolExprInner::Or(left, right) => {
                left.collect_variables_impl(vars);
                right.collect_variables_impl(vars);
            }
            BoolExprInner::Not(expr) => {
                expr.collect_variables_impl(vars);
            }
            BoolExprInner::Constant(_) => {}
        }
    }

    /// Logical AND: create a new expression that is the conjunction of this and another
    pub fn and(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr {
            inner: Arc::new(BoolExprInner::And(self.clone(), other.clone())),
            bdd_cache: Arc::new(OnceLock::new()),
        }
    }

    /// Logical OR: create a new expression that is the disjunction of this and another
    pub fn or(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Or(self.clone(), other.clone())),
            bdd_cache: Arc::new(OnceLock::new()),
        }
    }

    /// Logical NOT: create a new expression that is the negation of this one
    pub fn not(&self) -> BoolExpr {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Not(self.clone())),
            bdd_cache: Arc::new(OnceLock::new()),
        }
    }
}

/// Manual PartialEq implementation that only compares the expression structure,
/// not the cached BDD (cache is an optimization, not part of the logical value)
impl PartialEq for BoolExpr {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for BoolExpr {}

#[cfg(test)]
mod tests;
