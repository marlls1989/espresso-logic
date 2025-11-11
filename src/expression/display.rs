//! Display and Debug formatting for boolean expressions

use super::{BoolExpr, BoolExprAst};
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
        let ast = self.get_or_create_ast();
        Self::fmt_ast_with_context(f, &ast, ctx)
    }

    /// Format AST with context (helper method)
    fn fmt_ast_with_context(
        f: &mut fmt::Formatter<'_>,
        ast: &BoolExprAst,
        ctx: OpContext,
    ) -> fmt::Result {
        match ast {
            BoolExprAst::Variable(name) => write!(f, "{}", name),
            BoolExprAst::Constant(val) => write!(f, "{}", if *val { "1" } else { "0" }),

            BoolExprAst::And(left, right) => {
                // AND needs parens if inside a NOT
                let needs_parens = ctx == OpContext::Not;

                if needs_parens {
                    write!(f, "(")?;
                }

                Self::fmt_ast_with_context(f, left, OpContext::And)?;
                write!(f, " * ")?;
                Self::fmt_ast_with_context(f, right, OpContext::And)?;

                if needs_parens {
                    write!(f, ")")?;
                }
                Ok(())
            }

            BoolExprAst::Or(left, right) => {
                // OR needs parens if inside AND or NOT (lower precedence)
                let needs_parens = ctx == OpContext::And || ctx == OpContext::Not;

                if needs_parens {
                    write!(f, "(")?;
                }

                Self::fmt_ast_with_context(f, left, OpContext::Or)?;
                write!(f, " + ")?;
                Self::fmt_ast_with_context(f, right, OpContext::Or)?;

                if needs_parens {
                    write!(f, ")")?;
                }
                Ok(())
            }

            BoolExprAst::Not(inner) => {
                write!(f, "~")?;
                // NOT needs parens around compound expressions (AND/OR)
                // but NOT of NOT or variables/constants don't need parens
                match inner.as_ref() {
                    BoolExprAst::Variable(_) | BoolExprAst::Constant(_) | BoolExprAst::Not(_) => {
                        Self::fmt_ast_with_context(f, inner, OpContext::Not)
                    }
                    _ => {
                        write!(f, "(")?;
                        Self::fmt_ast_with_context(f, inner, OpContext::None)?;
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
