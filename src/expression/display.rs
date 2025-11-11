//! Display and Debug formatting for boolean expressions

use super::{BoolExpr, BoolExprInner};
use std::fmt;

/// Context for formatting expressions with minimal parentheses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpContext {
    None, // Top level or inside parentheses
    And,  // Inside an AND operation
    Or,   // Inside an OR operation
    Not,  // Inside a NOT operation
}

impl BoolExpr {
    /// Format with operator precedence context to minimize parentheses
    fn fmt_with_context(&self, f: &mut fmt::Formatter<'_>, ctx: OpContext) -> fmt::Result {
        match self.inner.as_ref() {
            BoolExprInner::Variable(name) => write!(f, "{}", name),
            BoolExprInner::Constant(val) => write!(f, "{}", if *val { "1" } else { "0" }),

            BoolExprInner::And(left, right) => {
                // AND needs parens if inside a NOT
                let needs_parens = ctx == OpContext::Not;

                if needs_parens {
                    write!(f, "(")?;
                }

                left.fmt_with_context(f, OpContext::And)?;
                write!(f, " * ")?;
                right.fmt_with_context(f, OpContext::And)?;

                if needs_parens {
                    write!(f, ")")?;
                }
                Ok(())
            }

            BoolExprInner::Or(left, right) => {
                // OR needs parens if inside AND or NOT (lower precedence)
                let needs_parens = ctx == OpContext::And || ctx == OpContext::Not;

                if needs_parens {
                    write!(f, "(")?;
                }

                left.fmt_with_context(f, OpContext::Or)?;
                write!(f, " + ")?;
                right.fmt_with_context(f, OpContext::Or)?;

                if needs_parens {
                    write!(f, ")")?;
                }
                Ok(())
            }

            BoolExprInner::Not(expr) => {
                write!(f, "~")?;
                // NOT needs parens around compound expressions (AND/OR)
                // but NOT of NOT or variables/constants don't need parens
                match expr.inner.as_ref() {
                    BoolExprInner::Variable(_)
                    | BoolExprInner::Constant(_)
                    | BoolExprInner::Not(_) => expr.fmt_with_context(f, OpContext::Not),
                    _ => {
                        write!(f, "(")?;
                        expr.fmt_with_context(f, OpContext::None)?;
                        write!(f, ")")
                    }
                }
            }
        }
    }
}

/// Debug formatting for boolean expressions
///
/// Formats expressions with minimal parentheses based on operator precedence.
/// Uses standard boolean algebra notation: `*` for AND, `+` for OR, `~` for NOT.
///
/// # Examples
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let c = BoolExpr::variable("c");
/// let expr = a.and(&b).or(&c);
///
/// println!("{:?}", expr);  // Outputs: a * b + c (no unnecessary parentheses)
/// ```
impl fmt::Debug for BoolExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_context(f, OpContext::None)
    }
}

/// Display formatting for boolean expressions
///
/// Delegates to the `Debug` implementation. Use `{}` or `{:?}` interchangeably.
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
/// println!("{}", expr);  // Same as println!("{:?}", expr)
/// ```
impl fmt::Display for BoolExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
