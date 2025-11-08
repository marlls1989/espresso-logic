//! Error types for the Espresso logic minimizer
//!
//! This module provides comprehensive error types that can be distinguished programmatically,
//! replacing string-based errors throughout the codebase.

use std::fmt;
use std::io;

/// The main error type for the Espresso logic minimizer
///
/// This enum covers all error cases that can occur when using the library,
/// providing programmatically distinguishable variants with detailed context.
#[derive(Debug)]
pub enum EspressoError {
    /// Conflict with existing thread-local Espresso instance
    ///
    /// This occurs when trying to create an Espresso instance with dimensions or
    /// configuration that conflicts with an existing instance on the same thread.
    /// Each thread can only have one Espresso instance at a time due to thread-local
    /// state management in the underlying C library.
    InstanceConflict {
        /// The requested dimensions (num_inputs, num_outputs)
        requested: (usize, usize),
        /// The existing instance's dimensions (num_inputs, num_outputs)
        existing: (usize, usize),
        /// The specific reason for the conflict
        reason: ConflictReason,
    },

    /// Invalid cube value encountered during cover creation
    ///
    /// Cube input values must be 0 (low), 1 (high), or 2 (don't care).
    InvalidCubeValue {
        /// The invalid value that was encountered
        value: u8,
        /// The position in the input vector where the invalid value occurred
        position: usize,
    },

    /// Failed to parse a boolean expression
    ///
    /// This error occurs when parsing a boolean expression string fails,
    /// providing the original input and position information when available.
    ParseError {
        /// The error message from the parser
        message: String,
        /// The original input string that failed to parse
        input: String,
        /// Optional position in the input where the error occurred
        position: Option<usize>,
    },

    /// Invalid input provided to a function
    ///
    /// This error is used for general input validation failures, such as
    /// attempting to add an expression to an output name that already exists,
    /// or accessing an output that doesn't exist.
    InvalidInput {
        /// Description of what was invalid
        message: String,
    },

    /// IO error wrapper
    ///
    /// Wraps standard IO errors that occur during file operations or writing.
    Io(io::Error),
}

/// The reason for an Espresso instance conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictReason {
    /// The requested dimensions don't match the existing instance
    DimensionMismatch,
    /// The requested configuration doesn't match the existing instance
    ConfigMismatch,
}

impl fmt::Display for EspressoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EspressoError::InstanceConflict {
                requested,
                existing,
                reason,
            } => match reason {
                ConflictReason::DimensionMismatch => write!(
                    f,
                    "Cannot create Espresso instance with dimensions {:?} because a \
                     thread-local instance with dimensions {:?} already exists. \
                     Drop all existing covers and handles first.",
                    requested, existing
                ),
                ConflictReason::ConfigMismatch => write!(
                    f,
                    "Cannot create Espresso instance with different configuration while a \
                     thread-local instance with dimensions {:?} already exists. \
                     Drop all existing covers and handles first.",
                    existing
                ),
            },
            EspressoError::InvalidCubeValue { value, position } => write!(
                f,
                "Invalid cube value {} at position {}. Expected 0 (low), 1 (high), or 2 (don't care).",
                value, position
            ),
            EspressoError::ParseError {
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
            EspressoError::InvalidInput { message } => write!(f, "{}", message),
            EspressoError::Io(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for EspressoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EspressoError::Io(err) => Some(err),
            _ => None,
        }
    }
}

// Conversion from io::Error to EspressoError
impl From<io::Error> for EspressoError {
    fn from(err: io::Error) -> Self {
        EspressoError::Io(err)
    }
}

// Conversion from EspressoError to io::Error for backwards compatibility
impl From<EspressoError> for io::Error {
    fn from(err: EspressoError) -> Self {
        match err {
            EspressoError::Io(io_err) => io_err,
            other => io::Error::other(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_dimension_mismatch_display() {
        let err = EspressoError::InstanceConflict {
            requested: (2, 1),
            existing: (3, 2),
            reason: ConflictReason::DimensionMismatch,
        };
        let msg = err.to_string();
        assert!(msg.contains("Cannot create Espresso instance"));
        assert!(msg.contains("(2, 1)"));
        assert!(msg.contains("(3, 2)"));
    }

    #[test]
    fn test_config_mismatch_display() {
        let err = EspressoError::InstanceConflict {
            requested: (2, 1),
            existing: (2, 1),
            reason: ConflictReason::ConfigMismatch,
        };
        let msg = err.to_string();
        assert!(msg.contains("different configuration"));
        assert!(msg.contains("(2, 1)"));
    }

    #[test]
    fn test_invalid_cube_value_display() {
        let err = EspressoError::InvalidCubeValue {
            value: 5,
            position: 2,
        };
        let msg = err.to_string();
        assert!(msg.contains("Invalid cube value 5"));
        assert!(msg.contains("position 2"));
    }

    #[test]
    fn test_parse_error_with_position() {
        let err = EspressoError::ParseError {
            message: "unexpected token".to_string(),
            input: "a * b ++".to_string(),
            position: Some(6),
        };
        let msg = err.to_string();
        assert!(msg.contains("position 6"));
        assert!(msg.contains("unexpected token"));
    }

    #[test]
    fn test_parse_error_without_position() {
        let err = EspressoError::ParseError {
            message: "unexpected end".to_string(),
            input: "a * b +".to_string(),
            position: None,
        };
        let msg = err.to_string();
        assert!(!msg.contains("position"));
        assert!(msg.contains("unexpected end"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let esp_err: EspressoError = io_err.into();
        let msg = esp_err.to_string();
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn test_error_trait_source() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        let esp_err = EspressoError::Io(io_err);
        assert!(esp_err.source().is_some());

        let parse_err = EspressoError::ParseError {
            message: "test".to_string(),
            input: "test".to_string(),
            position: None,
        };
        assert!(parse_err.source().is_none());
    }
}
