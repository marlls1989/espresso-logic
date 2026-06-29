//! Display and Debug formatting for [`BoolExpr`].
//!
//! Rendering walks the expression's own reverse-Polish token stream and reconstructs the operator tree
//! with **minimal parentheses**, using the canonical spellings `&` (AND), `|` (OR), `^` (XOR),
//! `!` (NOT) and `1`/`0` for the constants. The output reflects the expression's **syntactic**
//! structure, not a canonical sum-of-products.

use super::rpn::Token;
use super::BoolExpr;
use std::fmt;

// Binding-tightness levels, highest binds tightest. NOT > AND > XOR > OR mirrors the parser's
// precedence (parentheses () > NOT > AND > XOR > OR). An atom (variable/constant) binds tighter than
// any operator. A subexpression is wrapped in parentheses only when its own top-level operator binds
// *more loosely* than the position it sits in requires; since AND/OR/XOR are associative, equal-level
// children never need wrapping, which is what keeps the parenthesisation minimal.
const PREC_ATOM: u8 = 4;
const PREC_NOT: u8 = 3;
const PREC_AND: u8 = 2;
const PREC_XOR: u8 = 1;
const PREC_OR: u8 = 0;

/// Render a token stream to a string with minimal parentheses.
///
/// An iterative postfix fold: each operand on the stack carries `(text, precedence)`, where
/// `precedence` is the binding-tightness of its top-level operator. Combining wraps a child whenever
/// its precedence is below what the surrounding operator needs. No recursion, so a deeply nested
/// expression can't overflow the call stack.
fn render(tokens: &[Token]) -> String {
    // Wrap `s` in parentheses if its top operator (`have`) binds more loosely than `need`.
    fn wrap(s: String, have: u8, need: u8) -> String {
        if have < need {
            format!("({s})")
        } else {
            s
        }
    }

    let mut stack: Vec<(String, u8)> = Vec::with_capacity(tokens.len());
    for token in tokens {
        match token {
            Token::Var(name) => stack.push((name.to_string(), PREC_ATOM)),
            Token::Const(value) => {
                stack.push(((if *value { "1" } else { "0" }).to_string(), PREC_ATOM));
            }
            Token::Not => {
                let (s, p) = stack.pop().expect("display: underflow on NOT");
                // `!` binds tighter than every binary operator, so any binary operand is wrapped.
                stack.push((format!("!{}", wrap(s, p, PREC_NOT)), PREC_NOT));
            }
            Token::And => {
                let (rs, rp) = stack.pop().expect("display: underflow on AND");
                let (ls, lp) = stack.pop().expect("display: underflow on AND");
                stack.push((
                    format!("{} & {}", wrap(ls, lp, PREC_AND), wrap(rs, rp, PREC_AND)),
                    PREC_AND,
                ));
            }
            Token::Xor => {
                let (rs, rp) = stack.pop().expect("display: underflow on XOR");
                let (ls, lp) = stack.pop().expect("display: underflow on XOR");
                stack.push((
                    format!("{} ^ {}", wrap(ls, lp, PREC_XOR), wrap(rs, rp, PREC_XOR)),
                    PREC_XOR,
                ));
            }
            Token::Or => {
                let (rs, rp) = stack.pop().expect("display: underflow on OR");
                let (ls, lp) = stack.pop().expect("display: underflow on OR");
                stack.push((
                    format!("{} | {}", wrap(ls, lp, PREC_OR), wrap(rs, rp, PREC_OR)),
                    PREC_OR,
                ));
            }
        }
    }
    stack.pop().map(|(s, _)| s).unwrap_or_default()
}

/// Debug formatting for boolean expressions.
///
/// Renders with minimal parentheses based on operator precedence, using `&`/`|`/`^`/`!` and `1`/`0`.
///
/// # Examples
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let expr = BoolExpr::var("a") & BoolExpr::var("b") | BoolExpr::var("c");
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
/// use espresso_logic::BoolExpr;
///
/// let expr = BoolExpr::var("a") & BoolExpr::var("b");
/// assert_eq!(format!("{}", expr), "a & b");
/// ```
impl fmt::Display for BoolExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
