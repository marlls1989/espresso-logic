//! Boolean expression types with operator overloading and parsing support
//!
//! This module provides a boolean expression representation that can be constructed
//! programmatically using operator overloading or parsed from strings. Expressions
//! can be minimized using the Espresso algorithm by implementing the Cover trait.

use std::collections::BTreeSet;
use std::fmt;
use std::ops::{Add, Mul, Not};
use std::sync::Arc;

mod cover;
pub use cover::ExprCover;

// Lalrpop-generated parser module
#[allow(clippy::all)]
mod parser {
    use lalrpop_util::lalrpop_mod;
    lalrpop_mod!(pub bool_expr, "/expression/bool_expr.rs");
}

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
#[derive(Clone, PartialEq, Eq)]
pub struct BoolExpr {
    inner: Arc<BoolExprInner>,
}

impl BoolExpr {
    /// Create a variable expression with the given name
    pub fn variable(name: &str) -> Self {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Variable(Arc::from(name))),
        }
    }

    /// Create a constant expression (true or false)
    pub fn constant(value: bool) -> Self {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Constant(value)),
        }
    }

    /// Parse a boolean expression from a string
    ///
    /// Supports standard boolean operators:
    /// - `+` for OR
    /// - `*` for AND  
    /// - `~` or `!` for NOT
    /// - Parentheses for grouping
    /// - Constants: `0`, `1`, `true`, `false`
    pub fn parse(input: &str) -> Result<Self, String> {
        parser::bool_expr::ExprParser::new()
            .parse(input)
            .map_err(|e| format!("Parse error: {}", e))
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
        }
    }

    /// Logical OR: create a new expression that is the disjunction of this and another
    pub fn or(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Or(self.clone(), other.clone())),
        }
    }

    /// Logical NOT: create a new expression that is the negation of this one
    pub fn not(&self) -> BoolExpr {
        BoolExpr {
            inner: Arc::new(BoolExprInner::Not(self.clone())),
        }
    }

    /// Get a reference to the inner expression (internal use)
    pub(crate) fn inner(&self) -> &BoolExprInner {
        &self.inner
    }

    /// Minimize this boolean expression using Espresso
    ///
    /// This is a convenience method that creates an `ExprCover`, minimizes it,
    /// and returns the minimized expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, expr};
    ///
    /// # fn main() -> std::io::Result<()> {
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let c = BoolExpr::variable("c");
    ///
    /// // Redundant expression
    /// let expr = expr!(a * b + a * b * c);
    ///
    /// // Minimize it
    /// let minimized = expr.minimize()?;
    ///
    /// // minimized should be simpler (just a * b)
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimize(self) -> std::io::Result<BoolExpr> {
        use crate::Cover;
        let mut cover = ExprCover::from_expr(self);
        cover.minimize()?;
        Ok(cover.to_expr())
    }
}

/// Macro for building boolean expressions with clean syntax
///
/// Automatically inserts borrows so you can write `expr!(a * b + !a * !b)`
/// instead of `&a * &b + &(!&a) * &(!&b)`.
///
/// Supports:
/// - `*` for AND
/// - `+` for OR
/// - `!` for NOT  
/// - Parentheses for grouping
///
/// # Examples
///
/// ```
/// use espresso_logic::{BoolExpr, expr};
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
///
/// let xnor = expr!(a * b + !a * !b);  // Clean syntax!
/// ```
#[macro_export]
macro_rules! expr {
    // Parenthesized expression
    (( $($inner:tt)* )) => {
        expr!($($inner)*)
    };

    // NOT of identifier
    (! $e:ident) => {
        (&$e).not()
    };

    // NOT of parenthesized expression
    (! ( $($inner:tt)* )) => {
        expr!($($inner)*).not()
    };

    // Binary operations - just add & to everything
    ($a:ident + $b:ident) => {
        &$a + &$b
    };
    ($a:ident * $b:ident) => {
        &$a * &$b
    };

    // Complex mixed expressions with +
    ($a:ident * $b:ident + $c:ident * $d:ident * $e:ident) => {
        $a.and(&$b).or(&$c.and(&$d).and(&$e))
    };
    ($a:ident * $b:ident + $c:ident * $d:ident) => {
        $a.and(&$b).or(&$c.and(&$d))
    };
    ($a:ident * $b:ident + ! $c:ident * $d:ident) => {
        $a.and(&$b).or(&$c.not().and(&$d))
    };
    ($a:ident * $b:ident + $c:ident * ! $d:ident) => {
        $a.and(&$b).or(&$c.and(&$d.not()))
    };
    ($a:ident * $b:ident + ! $c:ident * ! $d:ident) => {
        $a.and(&$b).or(&$c.not().and(&$d.not()))
    };
    ($a:ident * ! $b:ident + ! $c:ident * $d:ident) => {
        $a.and(&$b.not()).or(&$c.not().and(&$d))
    };
    ($a:ident * ! $b:ident + $c:ident * ! $d:ident) => {
        $a.and(&$b.not()).or(&$c.and(&$d.not()))
    };
    ($a:ident * ! $b:ident + ! $c:ident * ! $d:ident) => {
        $a.and(&$b.not()).or(&$c.not().and(&$d.not()))
    };
    (! $a:ident * $b:ident + $c:ident * ! $d:ident) => {
        $a.not().and(&$b).or(&$c.and(&$d.not()))
    };
    (! $a:ident * $b:ident + ! $c:ident * $d:ident) => {
        $a.not().and(&$b).or(&$c.not().and(&$d))
    };
    (! $a:ident * $b:ident + ! $c:ident * ! $d:ident) => {
        $a.not().and(&$b).or(&$c.not().and(&$d.not()))
    };
    (! $a:ident * ! $b:ident + $c:ident * $d:ident) => {
        $a.not().and(&$b.not()).or(&$c.and(&$d))
    };
    (! $a:ident * ! $b:ident + $c:ident * ! $d:ident) => {
        $a.not().and(&$b.not()).or(&$c.and(&$d.not()))
    };
    (! $a:ident * ! $b:ident + ! $c:ident * $d:ident) => {
        $a.not().and(&$b.not()).or(&$c.not().and(&$d))
    };
    (! $a:ident * ! $b:ident + ! $c:ident * ! $d:ident) => {
        $a.not().and(&$b.not()).or(&$c.not().and(&$d.not()))
    };

    // AND chains
    ($a:ident * $b:ident * $c:ident) => {
        &$a * &$b * &$c
    };

    // Parenthesized sub-expressions with AND
    (( $($a:tt)* ) * $b:ident) => {
        expr!(( $($a)* )).and(&$b)
    };
    ($a:ident * ( $($b:tt)* )) => {
        $a.and(&expr!(( $($b)* )))
    };
    (( $($a:tt)* ) * ( $($b:tt)* )) => {
        expr!(( $($a)* )).and(&expr!(( $($b)* )))
    };
    (! ( $($a:tt)* ) * $b:ident) => {
        expr!(! ( $($a)* )).and(&$b)
    };
    ($a:ident * ! ( $($b:tt)* )) => {
        $a.and(&expr!(! ( $($b)* )))
    };
    (( $($a:tt)* ) * ! $b:ident) => {
        expr!(( $($a)* )).and(&$b.not())
    };
    (! $a:ident * ( $($b:tt)* )) => {
        $a.not().and(&expr!(( $($b)* )))
    };

    // Parenthesized sub-expressions with OR
    (( $($a:tt)* ) + $b:ident) => {
        expr!(( $($a)* )).or(&$b)
    };
    ($a:ident + ( $($b:tt)* )) => {
        $a.or(&expr!(( $($b)* )))
    };
    (( $($a:tt)* ) + ( $($b:tt)* )) => {
        expr!(( $($a)* )).or(&expr!(( $($b)* )))
    };
    (! ( $($a:tt)* ) + $b:ident) => {
        expr!(! ( $($a)* )).or(&$b)
    };
    ($a:ident + ! ( $($b:tt)* )) => {
        $a.or(&expr!(! ( $($b)* )))
    };
    (( $($a:tt)* ) + ! $b:ident) => {
        expr!(( $($a)* )).or(&$b.not())
    };
    (! $a:ident + ( $($b:tt)* )) => {
        $a.not().or(&expr!(( $($b)* )))
    };

    // Fallback - simple identifier
    ($e:ident) => {
        $e
    };
}

impl fmt::Debug for BoolExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner.as_ref() {
            BoolExprInner::Variable(name) => write!(f, "{}", name),
            BoolExprInner::And(left, right) => write!(f, "({:?} * {:?})", left, right),
            BoolExprInner::Or(left, right) => write!(f, "({:?} + {:?})", left, right),
            BoolExprInner::Not(expr) => write!(f, "~{:?}", expr),
            BoolExprInner::Constant(val) => write!(f, "{}", if *val { "1" } else { "0" }),
        }
    }
}

impl fmt::Display for BoolExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// Operator overloading
// Implemented for both owned and borrowed types
// The expr! macro wraps expressions to enable clean `a * b + !a * !b` syntax

/// Logical AND operator for references: `&a * &b`
impl Mul for &BoolExpr {
    type Output = BoolExpr;

    fn mul(self, rhs: &BoolExpr) -> BoolExpr {
        self.and(rhs)
    }
}

/// Logical AND operator: `a * b` (delegates to reference version)
impl Mul for BoolExpr {
    type Output = BoolExpr;

    fn mul(self, rhs: BoolExpr) -> BoolExpr {
        self.and(&rhs)
    }
}

/// Logical OR operator for references: `&a + &b`
impl Add for &BoolExpr {
    type Output = BoolExpr;

    fn add(self, rhs: &BoolExpr) -> BoolExpr {
        self.or(rhs)
    }
}

/// Logical OR operator: `a + b` (delegates to reference version)
impl Add for BoolExpr {
    type Output = BoolExpr;

    fn add(self, rhs: BoolExpr) -> BoolExpr {
        self.or(&rhs)
    }
}

/// Logical NOT operator for references: `!&a`
impl Not for &BoolExpr {
    type Output = BoolExpr;

    fn not(self) -> BoolExpr {
        BoolExpr::not(self)
    }
}

/// Logical NOT operator: `!a` (delegates to reference version)
impl Not for BoolExpr {
    type Output = BoolExpr;

    fn not(self) -> BoolExpr {
        BoolExpr::not(&self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_creation() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let a2 = BoolExpr::variable("a");

        // Variables are compared by structure
        assert_eq!(a, a2);
        assert_ne!(a, b);
    }

    #[test]
    fn test_constant_creation() {
        let t = BoolExpr::constant(true);
        let f = BoolExpr::constant(false);

        assert_eq!(t, BoolExpr::constant(true));
        assert_ne!(t, f);
    }

    #[test]
    fn test_method_api() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");

        // Test AND method - no clones in user code!
        let and_expr = a.and(&b);
        match and_expr.inner() {
            BoolExprInner::And(_, _) => {}
            _ => panic!("Expected And expression"),
        }

        // Test OR method - can still use a and b
        let or_expr = a.or(&b);
        match or_expr.inner() {
            BoolExprInner::Or(_, _) => {}
            _ => panic!("Expected Or expression"),
        }

        // Test NOT method
        let not_expr = a.not();
        match not_expr.inner() {
            BoolExprInner::Not(_) => {}
            _ => panic!("Expected Not expression"),
        }
    }

    #[test]
    fn test_complex_expression() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        // Build complex expression: (a AND b) OR (NOT a AND c)
        let expr = a.and(&b).or(&a.not().and(&c));

        match expr.inner() {
            BoolExprInner::Or(_, _) => {}
            _ => panic!("Expected Or at top level"),
        }
    }

    #[test]
    fn test_collect_variables() {
        let a = BoolExpr::variable("a");
        let b = BoolExpr::variable("b");
        let c = BoolExpr::variable("c");

        // Using method API
        let expr = a.and(&b).or(&c);
        let vars = expr.collect_variables();

        assert_eq!(vars.len(), 3);
        let var_names: Vec<String> = vars.iter().map(|s| s.to_string()).collect();
        assert_eq!(var_names, vec!["a", "b", "c"]); // Should be alphabetical
    }
}
