//! Error types for boolean expression parsing

use std::fmt;
use std::io;
use std::sync::Arc;

/// Errors related to boolean expression parsing
///
/// These errors occur when parsing a boolean expression string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionParseError {
    /// Failed to parse a boolean expression due to invalid syntax
    InvalidSyntax {
        /// The error message from the parser
        message: Arc<str>,
        /// The original input string that failed to parse
        input: Arc<str>,
        /// Optional position in the input where the error occurred
        position: Option<usize>,
    },
}

impl fmt::Display for ExpressionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpressionParseError::InvalidSyntax {
                message,
                input,
                position,
            } => {
                if let Some(pos) = position {
                    write!(
                        f,
                        "Failed to parse boolean expression at position {}: {}. Input: {:?}",
                        pos, message, input
                    )
                } else {
                    write!(
                        f,
                        "Failed to parse boolean expression: {}. Input: {:?}",
                        message, input
                    )
                }
            }
        }
    }
}

impl std::error::Error for ExpressionParseError {}

impl From<ExpressionParseError> for io::Error {
    fn from(err: ExpressionParseError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

/// Errors that can occur when parsing a boolean expression
///
/// This error type is returned by `BoolExpr::parse()`.
#[derive(Debug)]
pub enum ParseBoolExprError {
    /// Expression parsing error
    Parse(ExpressionParseError),
}

impl fmt::Display for ParseBoolExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseBoolExprError::Parse(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for ParseBoolExprError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParseBoolExprError::Parse(e) => Some(e),
        }
    }
}

impl From<ExpressionParseError> for ParseBoolExprError {
    fn from(err: ExpressionParseError) -> Self {
        ParseBoolExprError::Parse(err)
    }
}

impl From<ParseBoolExprError> for io::Error {
    fn from(err: ParseBoolExprError) -> Self {
        match err {
            ParseBoolExprError::Parse(e) => io::Error::new(io::ErrorKind::InvalidData, e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expression_parse_error_with_position() {
        let err = ExpressionParseError::InvalidSyntax {
            message: Arc::from("unexpected token"),
            input: Arc::from("a * b ++"),
            position: Some(6),
        };
        let msg = err.to_string();
        assert!(msg.contains("position 6"));
        assert!(msg.contains("unexpected token"));
    }

    #[test]
    fn test_expression_parse_error_without_position() {
        let err = ExpressionParseError::InvalidSyntax {
            message: Arc::from("unexpected end"),
            input: Arc::from("a * b +"),
            position: None,
        };
        let msg = err.to_string();
        assert!(!msg.contains("position"));
        assert!(msg.contains("unexpected end"));
    }

    #[test]
    fn test_parse_bool_expr_error() {
        let parse_err = ExpressionParseError::InvalidSyntax {
            message: Arc::from("test"),
            input: Arc::from("bad input"),
            position: Some(5),
        };
        let bool_err: ParseBoolExprError = parse_err.into();
        assert!(matches!(bool_err, ParseBoolExprError::Parse(_)));
    }

    #[test]
    fn test_expression_parse_error_to_io_error() {
        let err = ExpressionParseError::InvalidSyntax {
            message: Arc::from("test"),
            input: Arc::from("bad input"),
            position: Some(5),
        };
        let io_err: io::Error = err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_parse_bool_expr_error_to_io_error() {
        let parse_err = ExpressionParseError::InvalidSyntax {
            message: Arc::from("test"),
            input: Arc::from("bad input"),
            position: Some(5),
        };
        let bool_err = ParseBoolExprError::Parse(parse_err);
        let io_err: io::Error = bool_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    }
}
