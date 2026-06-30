//! Parsing support for boolean expressions.
//!
//! The lalrpop grammar (`bool_expr.lalrpop`) emits a reverse-Polish [`Token`] program directly, which
//! [`BoolExpr::parse`] wraps into an owned [`BoolExpr`].

use super::error::{ExpressionParseError, ParseBoolExprError};
use super::rpn::Token;
use super::BoolExpr;
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
        let message = e.to_string();
        // Try to extract position from lalrpop error message
        let position = extract_position_from_error(&message);
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

/// Helper function to extract position information from lalrpop error messages
///
/// Lalrpop errors often contain position information in the form "at line X column Y"
/// or similar patterns. This function attempts to extract that information.
fn extract_position_from_error(error_msg: &str) -> Option<usize> {
    // Try to find patterns like "at 5" or "position 5" or similar
    // Lalrpop errors typically have format like "Unrecognized token `+` at line 1 column 7"

    // Look for "column N" pattern
    if let Some(col_idx) = error_msg.find("column ") {
        let after_col = &error_msg[col_idx + 7..];
        if let Some(end_idx) = after_col.find(|c: char| !c.is_ascii_digit()) {
            if let Ok(col) = after_col[..end_idx].parse::<usize>() {
                return Some(col.saturating_sub(1)); // Convert to 0-indexed
            }
        }
    }

    // Look for "at N" pattern
    if let Some(at_idx) = error_msg.rfind(" at ") {
        let after_at = &error_msg[at_idx + 4..];
        if let Some(end_idx) = after_at.find(|c: char| !c.is_ascii_digit()) {
            if let Ok(pos) = after_at[..end_idx].parse::<usize>() {
                return Some(pos);
            }
        }
        // Try parsing until end if no non-digit found
        if let Ok(pos) = after_at.trim().parse::<usize>() {
            return Some(pos);
        }
    }

    None
}
