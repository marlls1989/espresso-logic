//! Cube types and definitions for Boolean function minimization
//!
//! This module provides the core cube-related types used in PLA covers:
//! - [`CubeType`]: Distinguishes between ON-set, don't-care, and OFF-set cubes
//! - [`Cube`]: Represents a single cube (product term) in a cover
//! - [`CubeData`]: Type alias for cube input/output data

use std::sync::Arc;

/// Type alias for cube data as owned vectors (inputs, outputs)
pub type CubeData = (Vec<Option<bool>>, Vec<Option<bool>>);

/// Type of a cube (ON-set, DC-set, or OFF-set)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubeType {
    /// ON-set cube (where the function is 1)
    F,
    /// Don't-care set cube (can be either 0 or 1)
    D,
    /// OFF-set cube (where the function is 0)
    R,
}

/// A cube in a PLA cover
#[derive(Clone, Debug)]
pub struct Cube {
    pub(crate) inputs: Arc<[Option<bool>]>,
    pub(crate) outputs: Arc<[bool]>, // Simplified: true = bit set, false = bit not set
    pub(crate) cube_type: CubeType,
}

impl Cube {
    pub(crate) fn new(inputs: &[Option<bool>], outputs: &[bool], cube_type: CubeType) -> Self {
        Cube {
            inputs: inputs.into(),
            outputs: outputs.into(),
            cube_type,
        }
    }

    /// Get the inputs of this cube
    ///
    /// Returns a slice where each element represents an input variable:
    /// - `Some(false)` - input must be 0
    /// - `Some(true)` - input must be 1
    /// - `None` - don't care (can be 0 or 1)
    pub fn inputs(&self) -> &[Option<bool>] {
        &self.inputs
    }

    /// Get the outputs of this cube
    ///
    /// Returns a slice where each element represents an output variable:
    /// - `true` - output is 1
    /// - `false` - output is 0
    pub fn outputs(&self) -> &[bool] {
        &self.outputs
    }

    /// Get the type of this cube (F, D, or R)
    pub fn cube_type(&self) -> CubeType {
        self.cube_type
    }
}
