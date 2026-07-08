//! Parsing support for boolean expressions.
//!
//! The lalrpop grammar (`bool_expr.lalrpop`) emits a reverse-Polish [`Token`] program directly, which
//! [`BoolExpr::parse`] wraps into an owned [`BoolExpr`].

use super::error::{ExpressionParseError, ParseBoolExprError};
use super::rpn::Token;
use super::BoolExpr;
use crate::StringLabel;
use lalrpop_util::ParseError;
use std::sync::Arc;

// The lalrpop-generated parser, included from OUT_DIR via the library's own macro (the documented
// idiom). `clippy::all` blankets the generated code's clippy lints; it triggers no rustc warnings.
lalrpop_util::lalrpop_mod!(
    #[allow(clippy::all)]
    parser_impl,
    "/expression/bool_expr.rs"
);

/// Parse a string into a reverse-Polish [`Token`] program with borrowed variable names.
fn parse_program(input: &str) -> Result<Vec<Token<&str>>, ParseBoolExprError> {
    parser_impl::ExprParser::new().parse(input).map_err(|e| {
        // The grammar uses lalrpop's built-in lexer (no custom `Location`/`Error` types), so `e` is
        // `ParseError<usize, Token<'input>, &'static str>`: every location lalrpop reports is already
        // a byte offset into `input`. Extract it structurally instead of scraping the `Display` text.
        let position = match &e {
            ParseError::InvalidToken { location } => Some(*location),
            ParseError::UnrecognizedEof { location, .. } => Some(*location),
            ParseError::UnrecognizedToken {
                token: (start, ..), ..
            } => Some(*start),
            ParseError::ExtraToken { token: (start, ..) } => Some(*start),
            ParseError::User { .. } => None,
        };
        let message = e.to_string();
        ExpressionParseError::InvalidSyntax {
            message: Arc::from(message.as_str()),
            input: Arc::from(input),
            position,
        }
        .into()
    })
}

/// Intern a borrowed-name [`Token`] program into an owned [`BoolExpr<S>`], re-interning each variable
/// name into the target label type `S`. This is the shared body behind both the inherent
/// [`BoolExpr::parse`] and the generic [`FromStr`](std::str::FromStr) path.
fn program_into_expr<S: StringLabel>(program: Vec<Token<&str>>) -> BoolExpr<S> {
    let program: Vec<Token<S>> = program
        .into_iter()
        .map(|token| match token {
            Token::Var(v) => Token::Var(S::from(v)),
            Token::Const(c) => Token::Const(c),
            Token::Not => Token::Not,
            Token::And => Token::And,
            Token::Or => Token::Or,
            Token::Xor => Token::Xor,
        })
        .collect();
    BoolExpr::from_tokens(Arc::from(program))
}

impl<S: StringLabel> BoolExpr<S> {
    /// Parse a boolean expression from a string.
    ///
    /// Supports standard boolean operators, in precedence order (lowest to highest):
    /// - `+` or `|` for OR
    /// - `^` for XOR
    /// - `*` or `&` for AND
    /// - `~` or `!` for NOT
    /// - Parentheses for grouping
    /// - Constants: `0`, `1`, `true`, `false`
    ///
    /// All binary operators are left-associative (both the `*`/`+`/`~` and `&`/`|`/`!` spellings lower
    /// to the same canonical operator set). The stored label type `S` follows the binding or turbofish,
    /// falling back to [`Symbol`](crate::Symbol) where an annotation pins it there; the equivalent annotation-driven
    /// [`str::parse`] path is `"a & b".parse::<BoolExpr<String>>()`.
    pub fn parse<N: AsRef<str>>(input: N) -> Result<Self, ParseBoolExprError> {
        let program = parse_program(input.as_ref())?;
        Ok(program_into_expr::<S>(program))
    }
}

/// Parse a boolean expression from a string, so `"a + b".parse::<BoolExpr>()` and generic `FromStr`
/// bounds work. This is the annotation-constrained generic text-construction path: the target label
/// type `S` comes from the `parse` turbofish or the binding's type, e.g.
/// `"a & b".parse::<BoolExpr<String>>()`.
impl<S: StringLabel> std::str::FromStr for BoolExpr<S> {
    type Err = ParseBoolExprError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let program = parse_program(s)?;
        Ok(program_into_expr::<S>(program))
    }
}
