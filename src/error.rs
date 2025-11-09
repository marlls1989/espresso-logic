//! Error types for the Espresso logic minimizer
//!
//! This module provides comprehensive error types organized by source and operation.
//! Each error source has its own enum with specific variants, and operations have
//! wrapper enums that combine only the errors they can produce.

use std::fmt;
use std::io;

// ============================================================================
// Source-Level Error Enums
// ============================================================================

/// Errors related to Espresso instance management
///
/// These errors occur when trying to create or use Espresso instances with
/// conflicting dimensions or configurations on the same thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceError {
    /// The requested dimensions don't match the existing thread-local instance
    DimensionMismatch {
        /// The requested dimensions (num_inputs, num_outputs)
        requested: (usize, usize),
        /// The existing instance's dimensions (num_inputs, num_outputs)
        existing: (usize, usize),
    },
    /// The requested configuration doesn't match the existing thread-local instance
    ConfigMismatch {
        /// The requested dimensions (num_inputs, num_outputs)
        requested: (usize, usize),
        /// The existing instance's dimensions (num_inputs, num_outputs)
        existing: (usize, usize),
    },
}

impl fmt::Display for InstanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstanceError::DimensionMismatch {
                requested,
                existing,
            } => write!(
                f,
                "Cannot create Espresso instance with dimensions {:?} because a \
                 thread-local instance with dimensions {:?} already exists. \
                 Drop all existing covers and handles first.",
                requested, existing
            ),
            InstanceError::ConfigMismatch {
                requested,
                existing,
            } => write!(
                f,
                "Cannot create Espresso instance with different configuration while a \
                 thread-local instance with dimensions {:?} already exists (requested {:?}). \
                 Drop all existing covers and handles first.",
                existing, requested
            ),
        }
    }
}

impl std::error::Error for InstanceError {}

impl From<InstanceError> for io::Error {
    fn from(err: InstanceError) -> Self {
        io::Error::new(io::ErrorKind::Other, err)
    }
}

/// Errors related to cube validation
///
/// These errors occur when invalid cube values are provided during cover creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CubeError {
    /// Invalid cube value encountered
    ///
    /// Cube input values must be 0 (low), 1 (high), or 2 (don't care).
    InvalidValue {
        /// The invalid value that was encountered
        value: u8,
        /// The position in the input vector where the invalid value occurred
        position: usize,
    },
}

impl fmt::Display for CubeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CubeError::InvalidValue { value, position } => write!(
                f,
                "Invalid cube value {} at position {}. Expected 0 (low), 1 (high), or 2 (don't care).",
                value, position
            ),
        }
    }
}

impl std::error::Error for CubeError {}

impl From<CubeError> for io::Error {
    fn from(err: CubeError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

/// Errors related to boolean expression parsing
///
/// These errors occur when parsing a boolean expression string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionParseError {
    /// Failed to parse a boolean expression due to invalid syntax
    InvalidSyntax {
        /// The error message from the parser
        message: String,
        /// The original input string that failed to parse
        input: String,
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

/// Errors related to cover operations
///
/// These errors occur during cover manipulation, such as adding expressions
/// or accessing outputs by name or index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverError {
    /// Attempted to add an expression to an output name that already exists
    OutputAlreadyExists {
        /// The name of the output that already exists
        name: String,
    },
    /// Attempted to access an output by name that doesn't exist
    OutputNotFound {
        /// The name of the output that was not found
        name: String,
    },
    /// Attempted to access an output by an index that is out of bounds
    OutputIndexOutOfBounds {
        /// The index that was requested
        index: usize,
        /// The maximum valid index (number of outputs - 1)
        max: usize,
    },
}

impl fmt::Display for CoverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoverError::OutputAlreadyExists { name } => {
                write!(f, "Output '{}' already exists in cover", name)
            }
            CoverError::OutputNotFound { name } => {
                write!(f, "Output '{}' not found in cover", name)
            }
            CoverError::OutputIndexOutOfBounds { index, max } => write!(
                f,
                "Output index {} out of bounds (valid range: 0..={})",
                index, max
            ),
        }
    }
}

impl std::error::Error for CoverError {}

impl From<CoverError> for io::Error {
    fn from(err: CoverError) -> Self {
        io::Error::new(io::ErrorKind::InvalidInput, err)
    }
}

/// Errors related to PLA format parsing and validation
///
/// These errors occur when reading or parsing PLA files with invalid format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PLAError {
    /// PLA file is missing the .i (inputs) directive
    MissingInputDirective,
    /// PLA file is missing the .o (outputs) directive
    MissingOutputDirective,
    /// Invalid value in .i directive
    InvalidInputDirective {
        /// The invalid value string
        value: String,
    },
    /// Invalid value in .o directive
    InvalidOutputDirective {
        /// The invalid value string
        value: String,
    },
    /// Invalid character in input portion of a cube
    InvalidInputCharacter {
        /// The invalid character
        character: char,
        /// Position in the input string
        position: usize,
    },
    /// Invalid character in output portion of a cube
    InvalidOutputCharacter {
        /// The invalid character
        character: char,
        /// Position in the output string
        position: usize,
    },
    /// Cube dimensions don't match declared dimensions
    CubeDimensionMismatch {
        /// Expected number of inputs
        expected_inputs: usize,
        /// Actual number of inputs in the cube
        actual_inputs: usize,
        /// Expected number of outputs
        expected_outputs: usize,
        /// Actual number of outputs in the cube
        actual_outputs: usize,
    },
    /// Label count doesn't match dimension count
    LabelCountMismatch {
        /// Type of label ("input" or "output")
        label_type: String,
        /// Expected number of labels
        expected: usize,
        /// Actual number of labels provided
        actual: usize,
    },
    /// PLA file has no dimension information (no .i/.o and no cubes to infer from)
    MissingDimensions,
}

impl fmt::Display for PLAError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PLAError::MissingInputDirective => {
                write!(f, "PLA file missing .i directive and no cubes to infer from")
            }
            PLAError::MissingOutputDirective => {
                write!(f, "PLA file missing .o directive and no cubes to infer from")
            }
            PLAError::InvalidInputDirective { value } => {
                write!(f, "Invalid .i directive value: '{}'", value)
            }
            PLAError::InvalidOutputDirective { value } => {
                write!(f, "Invalid .o directive value: '{}'", value)
            }
            PLAError::InvalidInputCharacter { character, position } => {
                write!(f, "Invalid input character '{}' at position {}", character, position)
            }
            PLAError::InvalidOutputCharacter { character, position } => {
                write!(f, "Invalid output character '{}' at position {}", character, position)
            }
            PLAError::CubeDimensionMismatch {
                expected_inputs,
                actual_inputs,
                expected_outputs,
                actual_outputs,
            } => write!(
                f,
                "Cube dimensions (inputs: {}, outputs: {}) don't match declared dimensions (inputs: {}, outputs: {})",
                actual_inputs, actual_outputs, expected_inputs, expected_outputs
            ),
            PLAError::LabelCountMismatch { label_type, expected, actual } => write!(
                f,
                "{} label count ({}) doesn't match {} count ({})",
                label_type, actual, label_type, expected
            ),
            PLAError::MissingDimensions => {
                write!(f, "PLA file has no dimension information")
            }
        }
    }
}

impl std::error::Error for PLAError {}

impl From<PLAError> for io::Error {
    fn from(err: PLAError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

// ============================================================================
// Operation-Level Error Enums
// ============================================================================

/// Errors that can occur during minimization operations
///
/// This error type is returned by `Cover::minimize()` and `BoolExpr::minimize()`.
#[derive(Debug)]
pub enum MinimizationError {
    /// Instance management error
    Instance(InstanceError),
    /// Cube validation error
    Cube(CubeError),
    /// IO error during minimization
    Io(io::Error),
}

impl fmt::Display for MinimizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MinimizationError::Instance(e) => write!(f, "Instance error: {}", e),
            MinimizationError::Cube(e) => write!(f, "Cube error: {}", e),
            MinimizationError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for MinimizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MinimizationError::Instance(e) => Some(e),
            MinimizationError::Cube(e) => Some(e),
            MinimizationError::Io(e) => Some(e),
        }
    }
}

impl From<InstanceError> for MinimizationError {
    fn from(err: InstanceError) -> Self {
        MinimizationError::Instance(err)
    }
}

impl From<CubeError> for MinimizationError {
    fn from(err: CubeError) -> Self {
        MinimizationError::Cube(err)
    }
}

impl From<io::Error> for MinimizationError {
    fn from(err: io::Error) -> Self {
        MinimizationError::Io(err)
    }
}

impl From<MinimizationError> for io::Error {
    fn from(err: MinimizationError) -> Self {
        match err {
            // If it's already an IO error, return it directly
            MinimizationError::Io(e) => e,
            // Otherwise, wrap it as Other
            MinimizationError::Instance(e) => io::Error::new(io::ErrorKind::Other, e),
            MinimizationError::Cube(e) => io::Error::new(io::ErrorKind::InvalidData, e),
        }
    }
}

/// Errors that can occur when adding an expression to a cover
///
/// This error type is returned by `Cover::add_expr()`.
#[derive(Debug)]
pub enum AddExprError {
    /// Cover operation error
    Cover(CoverError),
}

impl fmt::Display for AddExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddExprError::Cover(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for AddExprError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AddExprError::Cover(e) => Some(e),
        }
    }
}

impl From<CoverError> for AddExprError {
    fn from(err: CoverError) -> Self {
        AddExprError::Cover(err)
    }
}

impl From<AddExprError> for io::Error {
    fn from(err: AddExprError) -> Self {
        match err {
            AddExprError::Cover(e) => io::Error::new(io::ErrorKind::InvalidInput, e),
        }
    }
}

/// Errors that can occur when converting a cover to an expression
///
/// This error type is returned by `Cover::to_expr()` and `Cover::to_expr_by_index()`.
#[derive(Debug)]
pub enum ToExprError {
    /// Cover operation error
    Cover(CoverError),
}

impl fmt::Display for ToExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToExprError::Cover(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for ToExprError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ToExprError::Cover(e) => Some(e),
        }
    }
}

impl From<CoverError> for ToExprError {
    fn from(err: CoverError) -> Self {
        ToExprError::Cover(err)
    }
}

impl From<ToExprError> for io::Error {
    fn from(err: ToExprError) -> Self {
        match err {
            ToExprError::Cover(e) => io::Error::new(io::ErrorKind::InvalidInput, e),
        }
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

/// Errors that can occur when reading PLA format data
///
/// This error type is returned by `Cover::from_pla_*` methods.
#[derive(Debug)]
pub enum PLAReadError {
    /// PLA format error
    PLA(PLAError),
    /// IO error during reading
    Io(io::Error),
}

impl fmt::Display for PLAReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PLAReadError::PLA(e) => write!(f, "PLA format error: {}", e),
            PLAReadError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for PLAReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PLAReadError::PLA(e) => Some(e),
            PLAReadError::Io(e) => Some(e),
        }
    }
}

impl From<PLAError> for PLAReadError {
    fn from(err: PLAError) -> Self {
        PLAReadError::PLA(err)
    }
}

impl From<io::Error> for PLAReadError {
    fn from(err: io::Error) -> Self {
        PLAReadError::Io(err)
    }
}

impl From<PLAReadError> for io::Error {
    fn from(err: PLAReadError) -> Self {
        match err {
            // If it's already an IO error, return it directly
            PLAReadError::Io(e) => e,
            // Otherwise, wrap it as InvalidData
            PLAReadError::PLA(e) => io::Error::new(io::ErrorKind::InvalidData, e),
        }
    }
}

/// Errors that can occur when writing PLA format data
///
/// This error type is returned by `Cover::to_pla_*` methods.
#[derive(Debug)]
pub enum PLAWriteError {
    /// IO error during writing
    Io(io::Error),
}

impl fmt::Display for PLAWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PLAWriteError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for PLAWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PLAWriteError::Io(e) => Some(e),
        }
    }
}

impl From<io::Error> for PLAWriteError {
    fn from(err: io::Error) -> Self {
        PLAWriteError::Io(err)
    }
}

impl From<PLAWriteError> for io::Error {
    fn from(err: PLAWriteError) -> Self {
        match err {
            // PLAWriteError only contains IO errors, so return it directly
            PLAWriteError::Io(e) => e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    // ========================================================================
    // Source-Level Error Tests
    // ========================================================================

    #[test]
    fn test_instance_error_dimension_mismatch() {
        let err = InstanceError::DimensionMismatch {
            requested: (2, 1),
            existing: (3, 2),
        };
        let msg = err.to_string();
        assert!(msg.contains("Cannot create Espresso instance"));
        assert!(msg.contains("(2, 1)"));
        assert!(msg.contains("(3, 2)"));
    }

    #[test]
    fn test_instance_error_config_mismatch() {
        let err = InstanceError::ConfigMismatch {
            requested: (2, 1),
            existing: (2, 1),
        };
        let msg = err.to_string();
        assert!(msg.contains("different configuration"));
        assert!(msg.contains("(2, 1)"));
    }

    #[test]
    fn test_cube_error_invalid_value() {
        let err = CubeError::InvalidValue {
            value: 5,
            position: 2,
        };
        let msg = err.to_string();
        assert!(msg.contains("Invalid cube value 5"));
        assert!(msg.contains("position 2"));
    }

    #[test]
    fn test_expression_parse_error_with_position() {
        let err = ExpressionParseError::InvalidSyntax {
            message: "unexpected token".to_string(),
            input: "a * b ++".to_string(),
            position: Some(6),
        };
        let msg = err.to_string();
        assert!(msg.contains("position 6"));
        assert!(msg.contains("unexpected token"));
    }

    #[test]
    fn test_expression_parse_error_without_position() {
        let err = ExpressionParseError::InvalidSyntax {
            message: "unexpected end".to_string(),
            input: "a * b +".to_string(),
            position: None,
        };
        let msg = err.to_string();
        assert!(!msg.contains("position"));
        assert!(msg.contains("unexpected end"));
    }

    #[test]
    fn test_cover_error_output_already_exists() {
        let err = CoverError::OutputAlreadyExists {
            name: "result".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Output 'result' already exists"));
    }

    #[test]
    fn test_cover_error_output_not_found() {
        let err = CoverError::OutputNotFound {
            name: "missing".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Output 'missing' not found"));
    }

    #[test]
    fn test_cover_error_output_index_out_of_bounds() {
        let err = CoverError::OutputIndexOutOfBounds { index: 5, max: 2 };
        let msg = err.to_string();
        assert!(msg.contains("index 5"));
        assert!(msg.contains("0..=2"));
    }

    #[test]
    fn test_pla_error_missing_input_directive() {
        let err = PLAError::MissingInputDirective;
        let msg = err.to_string();
        assert!(msg.contains("missing .i directive"));
    }

    #[test]
    fn test_pla_error_invalid_input_character() {
        let err = PLAError::InvalidInputCharacter {
            character: 'x',
            position: 3,
        };
        let msg = err.to_string();
        assert!(msg.contains("'x'"));
        assert!(msg.contains("position 3"));
    }

    #[test]
    fn test_pla_error_cube_dimension_mismatch() {
        let err = PLAError::CubeDimensionMismatch {
            expected_inputs: 3,
            actual_inputs: 2,
            expected_outputs: 1,
            actual_outputs: 1,
        };
        let msg = err.to_string();
        assert!(msg.contains("inputs: 2"));
        assert!(msg.contains("inputs: 3"));
    }

    // ========================================================================
    // Operation-Level Error Tests
    // ========================================================================

    #[test]
    fn test_minimization_error_from_instance_error() {
        let inst_err = InstanceError::DimensionMismatch {
            requested: (2, 1),
            existing: (3, 2),
        };
        let min_err: MinimizationError = inst_err.into();
        assert!(matches!(min_err, MinimizationError::Instance(_)));
        assert!(min_err.source().is_some());
    }

    #[test]
    fn test_minimization_error_from_cube_error() {
        let cube_err = CubeError::InvalidValue {
            value: 5,
            position: 2,
        };
        let min_err: MinimizationError = cube_err.into();
        assert!(matches!(min_err, MinimizationError::Cube(_)));
    }

    #[test]
    fn test_minimization_error_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let min_err: MinimizationError = io_err.into();
        assert!(matches!(min_err, MinimizationError::Io(_)));
    }

    #[test]
    fn test_add_expr_error_from_cover_error() {
        let cover_err = CoverError::OutputAlreadyExists {
            name: "test".to_string(),
        };
        let add_err: AddExprError = cover_err.into();
        assert!(matches!(add_err, AddExprError::Cover(_)));
    }

    #[test]
    fn test_to_expr_error_from_cover_error() {
        let cover_err = CoverError::OutputNotFound {
            name: "test".to_string(),
        };
        let to_expr_err: ToExprError = cover_err.into();
        assert!(matches!(to_expr_err, ToExprError::Cover(_)));
    }

    #[test]
    fn test_parse_bool_expr_error() {
        let parse_err = ExpressionParseError::InvalidSyntax {
            message: "test".to_string(),
            input: "bad input".to_string(),
            position: Some(5),
        };
        let bool_err: ParseBoolExprError = parse_err.into();
        assert!(matches!(bool_err, ParseBoolExprError::Parse(_)));
    }

    #[test]
    fn test_pla_read_error_from_pla_error() {
        let pla_err = PLAError::MissingInputDirective;
        let read_err: PLAReadError = pla_err.into();
        assert!(matches!(read_err, PLAReadError::PLA(_)));
    }

    #[test]
    fn test_pla_read_error_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let read_err: PLAReadError = io_err.into();
        assert!(matches!(read_err, PLAReadError::Io(_)));
    }

    #[test]
    fn test_pla_write_error_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "permission denied");
        let write_err: PLAWriteError = io_err.into();
        assert!(matches!(write_err, PLAWriteError::Io(_)));
    }

    // ========================================================================
    // IO Error Conversion Tests
    // ========================================================================

    #[test]
    fn test_instance_error_to_io_error() {
        let err = InstanceError::DimensionMismatch {
            requested: (2, 1),
            existing: (3, 2),
        };
        let io_err: io::Error = err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn test_cube_error_to_io_error() {
        let err = CubeError::InvalidValue {
            value: 5,
            position: 2,
        };
        let io_err: io::Error = err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_expression_parse_error_to_io_error() {
        let err = ExpressionParseError::InvalidSyntax {
            message: "test".to_string(),
            input: "bad input".to_string(),
            position: Some(5),
        };
        let io_err: io::Error = err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_cover_error_to_io_error() {
        let err = CoverError::OutputNotFound {
            name: "test".to_string(),
        };
        let io_err: io::Error = err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_pla_error_to_io_error() {
        let err = PLAError::MissingInputDirective;
        let io_err: io::Error = err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_minimization_error_to_io_error_preserves_io_error() {
        let original_io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let min_err = MinimizationError::Io(original_io_err);
        let io_err: io::Error = min_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::NotFound);
        assert_eq!(io_err.to_string(), "file not found");
    }

    #[test]
    fn test_minimization_error_instance_to_io_error() {
        let inst_err = InstanceError::DimensionMismatch {
            requested: (2, 1),
            existing: (3, 2),
        };
        let min_err = MinimizationError::Instance(inst_err);
        let io_err: io::Error = min_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn test_minimization_error_cube_to_io_error() {
        let cube_err = CubeError::InvalidValue {
            value: 5,
            position: 2,
        };
        let min_err = MinimizationError::Cube(cube_err);
        let io_err: io::Error = min_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_add_expr_error_to_io_error() {
        let cover_err = CoverError::OutputAlreadyExists {
            name: "test".to_string(),
        };
        let add_err = AddExprError::Cover(cover_err);
        let io_err: io::Error = add_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_to_expr_error_to_io_error() {
        let cover_err = CoverError::OutputNotFound {
            name: "test".to_string(),
        };
        let to_expr_err = ToExprError::Cover(cover_err);
        let io_err: io::Error = to_expr_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_parse_bool_expr_error_to_io_error() {
        let parse_err = ExpressionParseError::InvalidSyntax {
            message: "test".to_string(),
            input: "bad input".to_string(),
            position: Some(5),
        };
        let bool_err = ParseBoolExprError::Parse(parse_err);
        let io_err: io::Error = bool_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_pla_read_error_to_io_error_preserves_io_error() {
        let original_io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let read_err = PLAReadError::Io(original_io_err);
        let io_err: io::Error = read_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::NotFound);
        assert_eq!(io_err.to_string(), "file not found");
    }

    #[test]
    fn test_pla_read_error_pla_to_io_error() {
        let pla_err = PLAError::MissingInputDirective;
        let read_err = PLAReadError::PLA(pla_err);
        let io_err: io::Error = read_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_pla_write_error_to_io_error() {
        let original_io_err = io::Error::new(io::ErrorKind::PermissionDenied, "permission denied");
        let write_err = PLAWriteError::Io(original_io_err);
        let io_err: io::Error = write_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(io_err.to_string(), "permission denied");
    }
}
