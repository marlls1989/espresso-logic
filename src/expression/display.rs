//! Display and Debug formatting for boolean expressions

use super::context::Brand;
use super::{BoolExpr, ExprNode};
use std::fmt;

/// Context for formatting expressions with minimal parentheses: the operator the current node sits
/// directly inside, which decides whether the node needs wrapping parentheses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpContext {
    None, // Top level or inside parentheses
    And,  // Inside an AND operation
    Or,   // Inside an OR operation
    Not,  // Inside a NOT operation
}

impl<B: Brand> BoolExpr<B> {
    /// Format with operator-precedence context to minimise parentheses.
    ///
    /// Renders via [`fold_with_context`](BoolExpr::fold_with_context) — an iterative walk — so a deeply
    /// nested expression can't overflow the call stack while formatting. The top-down context is the
    /// surrounding operator; a node wraps itself in parentheses only when its precedence requires it
    /// (an `AND` inside a `NOT`, or an `OR` inside an `AND`/`NOT`). A `NOT` never needs parens itself —
    /// its compound operand wraps via this same rule — and a variable/constant never does.
    fn fmt_with_context(&self, f: &mut fmt::Formatter<'_>, ctx: OpContext) -> fmt::Result {
        let rendered = self.fold_with_context(
            ctx,
            // descend: a node's children sit "inside" that node's operator.
            |node, _parent| match node {
                ExprNode::And(..) => (OpContext::And, OpContext::And),
                ExprNode::Or(..) => (OpContext::Or, OpContext::Or),
                ExprNode::Not(()) => (OpContext::Not, OpContext::Not),
                ExprNode::Variable(_) | ExprNode::Constant(_) => (OpContext::None, OpContext::None),
            },
            // combine: build each node's string, parenthesising by its own surrounding context.
            |node, ctx| match node {
                ExprNode::Variable(name) => name.to_string(),
                ExprNode::Constant(val) => if val { "1" } else { "0" }.to_string(),
                ExprNode::Not(inner) => format!("~{inner}"),
                ExprNode::And(left, right) => {
                    let s = format!("{left} * {right}");
                    if ctx == OpContext::Not {
                        format!("({s})")
                    } else {
                        s
                    }
                }
                ExprNode::Or(left, right) => {
                    let s = format!("{left} + {right}");
                    if ctx == OpContext::And || ctx == OpContext::Not {
                        format!("({s})")
                    } else {
                        s
                    }
                }
            },
        );
        write!(f, "{rendered}")
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
impl<B: Brand> fmt::Debug for BoolExpr<B> {
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
impl<B: Brand> fmt::Display for BoolExpr<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
