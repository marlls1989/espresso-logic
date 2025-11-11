//! Parsing support for boolean expressions

use super::error::{ExpressionParseError, ParseBoolExprError};
use super::BoolExpr;
use std::sync::Arc;

// Lalrpop-generated parser module (generated in OUT_DIR at build time)
#[allow(clippy::all)]
mod parser_impl {
    #![allow(clippy::all)]
    #![allow(dead_code)]
    #![allow(unused_variables)]
    #![allow(unused_imports)]
    #![allow(non_snake_case)]
    #![allow(non_camel_case_types)]
    #![allow(non_upper_case_globals)]
    include!(concat!(env!("OUT_DIR"), "/expression/bool_expr.rs"));
}

impl BoolExpr {
    /// Parse a boolean expression from a string
    ///
    /// Supports standard boolean operators:
    /// - `+` or `|` for OR
    /// - `*` or `&` for AND  
    /// - `~` or `!` for NOT
    /// - Parentheses for grouping
    /// - Constants: `0`, `1`, `true`, `false`
    pub fn parse(input: &str) -> Result<Self, ParseBoolExprError> {
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
