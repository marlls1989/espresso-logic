//! Parsing support for boolean expressions.
//!
//! The lalrpop grammar (`bool_expr.lalrpop`) emits a reverse-Polish [`Token`] program directly, which
//! [`BoolExpr::parse`] wraps into an owned [`BoolExpr`].

use super::error::{ExpressionParseError, ParseBoolExprError};
use super::rpn::Token;
use super::BoolExpr;
use lalrpop_util::ParseError;
use std::sync::Arc;

// The lalrpop-generated parser, included from OUT_DIR via the library's own macro (the documented
// idiom). `clippy::all` blankets the generated code's clippy lints; it triggers no rustc warnings.
lalrpop_util::lalrpop_mod!(
    #[allow(clippy::all)]
    parser_impl,
    "/expression/bool_expr.rs"
);

/// Parse a string into a reverse-Polish [`Token`] program.
fn parse_program(input: &str) -> Result<Vec<Token>, ParseBoolExprError> {
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

impl BoolExpr {
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
    /// All binary operators are left-associative. The result is the owned, syntactic [`BoolExpr`] of
    /// the parsed text (both the `*`/`+`/`~` and `&`/`|`/`!` spellings lower to the same canonical
    /// operator set).
    pub fn parse<S: AsRef<str>>(input: S) -> Result<Self, ParseBoolExprError> {
        let program = parse_program(input.as_ref())?;
        Ok(BoolExpr::from_tokens(Arc::from(program)))
    }
}

/// Parse a boolean expression from a string, so `"a + b".parse::<BoolExpr>()` and generic `FromStr`
/// bounds work. Delegates to the inherent [`BoolExpr::parse`].
impl std::str::FromStr for BoolExpr {
    type Err = ParseBoolExprError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        BoolExpr::parse(s)
    }
}
