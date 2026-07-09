//! Display and Debug formatting for [`BoolExpr`].
//!
//! Rendering walks the expression's own reverse-Polish token stream and reconstructs the operator tree
//! with **minimal parentheses**, using the canonical spellings `&` (AND), `|` (OR), `^` (XOR),
//! `!` (NOT) and `1`/`0` for the constants. The output reflects the expression's **syntactic**
//! structure, not a canonical sum-of-products.

use super::rpn::{self, Token};
use super::BoolExpr;
use std::fmt;

// Binding-tightness levels, highest binds tightest. NOT > AND > XOR > OR mirrors the parser's
// precedence (parentheses () > NOT > AND > XOR > OR). An atom (variable/constant) binds tighter than
// any operator. `& | ^` are *left*-associative in the grammar, so the rendering must round-trip:
// `a & (b & c)` is a different tree from `(a & b) & c` and may not lose its parentheses. The left
// operand of a binary node is therefore wrapped only when it binds *strictly* looser than the node
// (an equal-precedence left child re-parses correctly unwrapped, since parsing is left-associative),
// while the right operand is wrapped when it binds *at most as tightly* (an equal-precedence right
// child must keep its parentheses, else it would re-parse as left-nested). This keeps the
// parenthesisation minimal while preserving the syntactic tree.
const PREC_ATOM: u8 = 4;
const PREC_NOT: u8 = 3;
const PREC_AND: u8 = 2;
const PREC_XOR: u8 = 1;
const PREC_OR: u8 = 0;

/// Render a token stream to a string with minimal parentheses.
///
/// An iterative postfix fold: each operand on the stack carries `(text, precedence)`, where
/// `precedence` is the binding-tightness of its top-level operator. The left operand of a binary node
/// is wrapped when its precedence is strictly below what the node needs; the right operand is wrapped
/// when its precedence is at most what the node needs, so a right-nested associative child keeps the
/// parentheses that make the rendering round-trip. No recursion, so a deeply nested expression can't
/// overflow the call stack.
fn render(tokens: &[Token]) -> String {
    // Wrap `s` if its top operator (`have`) binds strictly more loosely than `need`. Used for the
    // left operand of a left-associative binary node (and for NOT's operand).
    fn wrap(s: String, have: u8, need: u8) -> String {
        if have < need {
            format!("({s})")
        } else {
            s
        }
    }

    // Wrap `s` if its top operator binds at most as tightly as `need`. Used for the *right* operand of
    // a left-associative binary node: an equal-precedence right child must stay parenthesised, else it
    // would re-parse as left-nested — a different tree.
    fn wrap_right(s: String, have: u8, need: u8) -> String {
        if have <= need {
            format!("({s})")
        } else {
            s
        }
    }

    // An empty token stream renders as the empty string. (Constructed expressions always carry at
    // least one token, so this only guards the degenerate input the shared walk would reject.)
    if tokens.is_empty() {
        return String::new();
    }

    // Each operand on the value stack carries `(text, precedence)`; combining wraps a child whenever
    // its precedence is below what the surrounding operator needs.
    let (text, _) = rpn::fold_postfix(
        tokens,
        |name| (name.to_string(), PREC_ATOM),
        |value| ((if value { "1" } else { "0" }).to_string(), PREC_ATOM),
        // `!` binds tighter than every binary operator, so any binary operand is wrapped.
        |(s, p)| (format!("!{}", wrap(s, p, PREC_NOT)), PREC_NOT),
        |(ls, lp), (rs, rp)| {
            (
                format!(
                    "{} & {}",
                    wrap(ls, lp, PREC_AND),
                    wrap_right(rs, rp, PREC_AND)
                ),
                PREC_AND,
            )
        },
        |(ls, lp), (rs, rp)| {
            (
                format!(
                    "{} | {}",
                    wrap(ls, lp, PREC_OR),
                    wrap_right(rs, rp, PREC_OR)
                ),
                PREC_OR,
            )
        },
        |(ls, lp), (rs, rp)| {
            (
                format!(
                    "{} ^ {}",
                    wrap(ls, lp, PREC_XOR),
                    wrap_right(rs, rp, PREC_XOR)
                ),
                PREC_XOR,
            )
        },
    );
    text
}

/// Debug formatting for boolean expressions.
///
/// Renders with minimal parentheses based on operator precedence, using `&`/`|`/`^`/`!` and `1`/`0`.
///
/// # Examples
///
/// ```
/// use espresso_logic::expr;
///
/// let expr = expr!("a" & "b" | "c");
/// assert_eq!(format!("{:?}", expr), "a & b | c"); // no unnecessary parentheses
/// ```
impl fmt::Debug for BoolExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", render(self.tokens()))
    }
}

/// Display formatting for boolean expressions. Delegates to the [`Debug`] implementation; use `{}` or
/// `{:?}` interchangeably.
///
/// # Examples
///
/// ```
/// use espresso_logic::expr;
///
/// let expr = expr!("a" & "b");
/// assert_eq!(format!("{}", expr), "a & b");
/// ```
impl fmt::Display for BoolExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
