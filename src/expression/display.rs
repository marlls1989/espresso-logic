//! Display and Debug formatting for [`BoolExpr`].
//!
//! Rendering walks the expression's own reverse-Polish token stream and reconstructs the operator tree
//! with **minimal parentheses**. The operator spellings, the constants and the precedence that decides
//! which parentheses are necessary all come from the expression's own
//! [`Syntax`](super::Syntax) — `&` (AND), `|` (OR), `^` (XOR), `!` (NOT) and `1`/`0` under
//! [`StdSyntax`](super::StdSyntax). The output reflects the expression's **syntactic** structure, not a
//! canonical sum-of-products.

use super::rpn::{self, Token};
use super::syntax::syntax_seal;
use super::{BoolExpr, Syntax};
use std::fmt;

/// Render a token stream to a string with minimal parentheses, in the syntax `S`.
///
/// The spellings and the binding-tightness levels both come from `S`'s specification. Within any
/// syntax, an atom (variable/constant) binds tighter than any operator, and `& | ^` are
/// *left*-associative, so the rendering must round-trip: `a & (b & c)` is a different tree from
/// `(a & b) & c` and may not lose its parentheses. The left operand of a binary node is therefore
/// wrapped only when it binds *strictly* looser than the node (an equal-precedence left child re-parses
/// correctly unwrapped, since parsing is left-associative), while the right operand is wrapped when it
/// binds *at most as tightly* (an equal-precedence right child must keep its parentheses, else it would
/// re-parse as left-nested). This keeps the parenthesisation minimal while preserving the syntactic
/// tree.
///
/// An iterative postfix fold: each operand on the stack carries `(text, precedence)`, where
/// `precedence` is the binding-tightness of its top-level operator. No recursion, so a deeply nested
/// expression can't overflow the call stack.
fn render<S: Syntax>(tokens: &[Token]) -> String {
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

    let spec = <S as syntax_seal::Sealed>::SPEC;

    // Each operand on the value stack carries `(text, precedence)`; combining wraps a child whenever
    // its precedence is below what the surrounding operator needs.
    let (text, _) = rpn::fold_postfix(
        tokens,
        |name| (name.to_string(), spec.prec_atom),
        |value| {
            (
                (if value { spec.one } else { spec.zero }).to_string(),
                spec.prec_atom,
            )
        },
        // NOT binds tighter than every binary operator, so any binary operand is wrapped. Its spelling
        // is emitted as a prefix/suffix pair, one of which is empty in either fixity.
        |(s, p)| {
            (
                format!(
                    "{}{}{}",
                    spec.not_prefix,
                    wrap(s, p, spec.prec_not),
                    spec.not_suffix
                ),
                spec.prec_not,
            )
        },
        |(ls, lp), (rs, rp)| {
            (
                format!(
                    "{} {} {}",
                    wrap(ls, lp, spec.prec_and),
                    spec.and,
                    wrap_right(rs, rp, spec.prec_and)
                ),
                spec.prec_and,
            )
        },
        |(ls, lp), (rs, rp)| {
            (
                format!(
                    "{} {} {}",
                    wrap(ls, lp, spec.prec_or),
                    spec.or,
                    wrap_right(rs, rp, spec.prec_or)
                ),
                spec.prec_or,
            )
        },
        |(ls, lp), (rs, rp)| {
            (
                format!(
                    "{} {} {}",
                    wrap(ls, lp, spec.prec_xor),
                    spec.xor,
                    wrap_right(rs, rp, spec.prec_xor)
                ),
                spec.prec_xor,
            )
        },
    );
    text
}

/// Debug formatting for boolean expressions.
///
/// Renders with minimal parentheses based on operator precedence, in the expression's own syntax —
/// `&`/`|`/`^`/`!` and `1`/`0` under [`StdSyntax`](super::StdSyntax).
///
/// # Examples
///
/// ```
/// use espresso_logic::expr;
///
/// let expr = expr!("a" & "b" | "c");
/// assert_eq!(format!("{:?}", expr), "a & b | c"); // no unnecessary parentheses
/// ```
impl<S: Syntax> fmt::Debug for BoolExpr<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", render::<S>(self.tokens()))
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
impl<S: Syntax> fmt::Display for BoolExpr<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
