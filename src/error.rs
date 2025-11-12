//! Error types for the Espresso logic minimizer
//!
//! This module re-exports error types from their respective submodules where each error
//! is colocated with the functionality it relates to:
//!
//! - [`espresso::error`](crate::espresso::error) - Instance and cube errors
//! - [`cover::error`](crate::cover::error) - Cover operation errors  
//! - [`expression::error`](crate::expression::error) - Expression parsing errors
//! - [`cover::pla::error`](crate::cover::pla::error) - PLA format errors
//!
//! # Organization
//!
//! Each error type lives in the module it's most relevant to:
//!
//! ## Espresso Module Errors
//!
//! - [`InstanceError`] - Espresso instance dimension/config conflicts
//! - [`CubeError`] - Invalid cube values during cover creation
//! - [`MinimizationError`] - Errors during minimization (combines Instance, Cube, IO)
//!
//! ## Cover Module Errors
//!
//! - [`CoverError`] - Cover operations (output access, bounds)
//! - [`AddExprError`] - Adding expressions to covers
//! - [`ToExprError`] - Converting covers to expressions
//!
//! ## Expression Module Errors
//!
//! - [`ExpressionParseError`] - Boolean expression parsing failures
//! - [`ParseBoolExprError`] - Operation wrapper for expression parsing
//!
//! ## PLA Module Errors
//!
//! - [`PLAError`] - PLA format validation
//! - [`PLAReadError`] - PLA reading operations
//! - [`PLAWriteError`] - PLA writing operations

// Re-export error types from submodules for backward compatibility
pub use crate::cover::error::{AddExprError, CoverError, ToExprError};
pub use crate::cover::pla::error::{PLAError, PLAReadError, PLAWriteError};
pub use crate::espresso::error::{CubeError, InstanceError, MinimizationError};
pub use crate::expression::error::{ExpressionParseError, ParseBoolExprError};
