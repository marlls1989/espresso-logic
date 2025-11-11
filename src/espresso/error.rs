//! Error types for Espresso instance management and cube operations

use std::fmt;
use std::io;

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
        io::Error::other(err)
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
            MinimizationError::Instance(e) => io::Error::other(e),
            MinimizationError::Cube(e) => io::Error::new(io::ErrorKind::InvalidData, e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

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
}
