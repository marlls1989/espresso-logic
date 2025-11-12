//! Error types for PLA format parsing and validation

use std::fmt;
use std::io;
use std::sync::Arc;

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
        value: Arc<str>,
    },
    /// Invalid value in .o directive
    InvalidOutputDirective {
        /// The invalid value string
        value: Arc<str>,
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
        label_type: Arc<str>,
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

    #[test]
    fn test_pla_error_to_io_error() {
        let err = PLAError::MissingInputDirective;
        let io_err: io::Error = err.into();
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
