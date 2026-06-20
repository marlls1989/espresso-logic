//! Error types for cover operations

use crate::Symbol;
use std::fmt;
use std::io;

/// Errors related to cover operations
///
/// These errors occur during cover manipulation, such as adding expressions
/// or accessing outputs by name or index.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CoverError {
    /// Attempted to add an expression to an output name that already exists
    OutputAlreadyExists {
        /// The name of the output that already exists
        name: Symbol,
    },
    /// Attempted to access an output by name that doesn't exist
    OutputNotFound {
        /// The name of the output that was not found
        name: Symbol,
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

/// Errors that can occur when adding an expression to a cover
///
/// This error type is returned by `Cover::add_expr()`.
///
/// Kept distinct from [`ToExprError`] (both are currently single `Cover` wrappers) on purpose: they
/// name different operations and are `#[non_exhaustive]`, so either can grow operation-specific
/// variants without churning the other's call sites.
#[derive(Debug)]
#[non_exhaustive]
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
#[non_exhaustive]
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

/// A new symbol table's arity did not match the cover it was being applied to.
///
/// Returned by [`Cover::relabel`](crate::Cover::relabel),
/// [`relabel_inputs`](crate::Cover::relabel_inputs) and
/// [`relabel_outputs`](crate::Cover::relabel_outputs): re-labelling is position-for-position, so the
/// replacement table must have exactly as many labels as the side it replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ArityMismatch {
    /// The new input table's arity differs from the cover's input arity.
    Inputs {
        /// The cover's input arity.
        expected: usize,
        /// The replacement input table's arity.
        actual: usize,
    },
    /// The new output table's arity differs from the cover's output arity.
    Outputs {
        /// The cover's output arity.
        expected: usize,
        /// The replacement output table's arity.
        actual: usize,
    },
}

impl fmt::Display for ArityMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArityMismatch::Inputs { expected, actual } => write!(
                f,
                "input arity mismatch: cover has {} input(s) but the new label table has {}",
                expected, actual
            ),
            ArityMismatch::Outputs { expected, actual } => write!(
                f,
                "output arity mismatch: cover has {} output(s) but the new label table has {}",
                expected, actual
            ),
        }
    }
}

impl std::error::Error for ArityMismatch {}

impl From<ArityMismatch> for io::Error {
    fn from(err: ArityMismatch) -> Self {
        io::Error::new(io::ErrorKind::InvalidInput, err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arity_mismatch_displays_side_and_counts() {
        let err = ArityMismatch::Inputs {
            expected: 3,
            actual: 2,
        };
        let msg = err.to_string();
        assert!(msg.contains("input arity mismatch"));
        assert!(msg.contains('3'));
        assert!(msg.contains('2'));
    }

    #[test]
    fn test_cover_error_output_already_exists() {
        let err = CoverError::OutputAlreadyExists {
            name: Symbol::from("result"),
        };
        let msg = err.to_string();
        assert!(msg.contains("Output 'result' already exists"));
    }

    #[test]
    fn test_cover_error_output_not_found() {
        let err = CoverError::OutputNotFound {
            name: Symbol::from("missing"),
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
    fn test_add_expr_error_from_cover_error() {
        let cover_err = CoverError::OutputAlreadyExists {
            name: Symbol::from("test"),
        };
        let add_err: AddExprError = cover_err.into();
        assert!(matches!(add_err, AddExprError::Cover(_)));
    }

    #[test]
    fn test_to_expr_error_from_cover_error() {
        let cover_err = CoverError::OutputNotFound {
            name: Symbol::from("test"),
        };
        let to_expr_err: ToExprError = cover_err.into();
        assert!(matches!(to_expr_err, ToExprError::Cover(_)));
    }

    #[test]
    fn test_cover_error_to_io_error() {
        let err = CoverError::OutputNotFound {
            name: Symbol::from("test"),
        };
        let io_err: io::Error = err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_add_expr_error_to_io_error() {
        let cover_err = CoverError::OutputAlreadyExists {
            name: Symbol::from("test"),
        };
        let add_err = AddExprError::Cover(cover_err);
        let io_err: io::Error = add_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_to_expr_error_to_io_error() {
        let cover_err = CoverError::OutputNotFound {
            name: Symbol::from("test"),
        };
        let to_expr_err = ToExprError::Cover(cover_err);
        let io_err: io::Error = to_expr_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidInput);
    }
}
