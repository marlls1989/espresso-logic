//! Cover types and traits for Boolean function minimisation
//!
//! This module provides the [`Cover`] type for working with covers - sum-of-products
//! (truth table) representations of Boolean functions.
//!
//! # What is a Cover?
//!
//! A **cover** represents a Boolean function as a set of **cubes** (product terms). Each cube
//! specifies input conditions and corresponding output values. Covers are the fundamental
//! representation used by the Espresso minimisation algorithm.
//!
//! ## Key Concepts
//!
//! - **Cube**: A product term - one row in a truth table
//! - **Input pattern**: Binary values (0, 1) or don't-cares (-) for input variables
//! - **Output pattern**: Binary values showing which outputs are active
//! - **Cover type**: Specifies which sets are included (F, FD, FR, or FDR)
//!
//! ## Cover Types
//!
//! - **F Type** (ON-set only) - Specifies where outputs are 1
//! - **FD Type** (ON-set + Don't-cares) - Adds flexibility for optimisation
//! - **FR Type** (ON-set + OFF-set) - Specifies both 1s and 0s explicitly
//! - **FDR Type** (Complete) - ON-set + Don't-cares + OFF-set
//!
//! # When to Use Cover vs BoolExpr
//!
//! Use **[`Cover`]** when you need:
//! - Manual truth table construction
//! - Direct cube manipulation
//! - Multi-output functions
//! - Fine control over don't-care and off-sets
//!
//! Use **[`BoolExpr`](crate::BoolExpr)** when you need:
//! - Expression parsing or composition
//! - High-level boolean operations
//! - Automatic BDD-based simplification
//! - Single-output functions
//!
//! # Dynamic Dimensions
//!
//! Unlike the low-level API, [`Cover`] has **dynamic dimensions** that grow automatically
//! as cubes are added. This eliminates the need for manual dimension tracking.
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```
//! use espresso_logic::{Cover, CoverType, Minimizable};
//!
//! // Create a cover for XOR function
//! let mut cover = Cover::new(CoverType::F);
//! cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);   // 01 -> 1
//! cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);   // 10 -> 1
//!
//! println!("Before: {} cubes", cover.num_cubes());
//!
//! // Minimise
//! let minimised = cover.minimize().unwrap();
//! println!("After: {} cubes", minimised.num_cubes());
//! ```
//!
//! ## With Boolean Expressions
//!
//! ```
//! use espresso_logic::{BoolExpr, Cover, CoverType, Minimizable};
//!
//! # fn main() -> std::io::Result<()> {
//! let expr = BoolExpr::parse("a * b + a * b * c")?;
//!
//! // Convert expression to cover
//! let mut cover = Cover::new(CoverType::F);
//! cover.add_expr(&expr, "output")?;
//!
//! // Minimise
//! let minimised = cover.minimize()?;
//!
//! // Convert back to expression
//! let result = minimised.to_expr("output")?;
//! println!("Result: {}", result);
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`CoverType`] - Different types of covers (F, FD, FR, FDR)
//! - [`Cube`] - Individual product terms in a cover
//! - [`Minimizable`] - Trait for minimisation operations
//! - [`pla`] - PLA file I/O for reading/writing covers in original Espresso format

// Module declarations
mod conversions;
mod cubes;
mod dnf;
pub mod error;
mod expressions;
mod iterators;
mod labels;
mod minimisation;
pub mod pla;

// Public re-exports - core types
pub use cubes::{Cube, CubeData, CubeType};
pub use dnf::Dnf;
pub use error::{AddExprError, CoverError, ToExprError};
pub use iterators::{CubesIter, ToExprs};
pub use minimisation::Minimizable;

// Import internal types for Cover implementation
use labels::LabelManager;
use std::sync::Arc;

/// Represents the type of cover (F, FD, FR, or FDR)
///
/// This type determines which sets are included in the cover:
/// - F: ON-set only
/// - FD: ON-set + Don't-care set
/// - FR: ON-set + OFF-set  
/// - FDR: ON-set + Don't-care set + OFF-set
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverType {
    /// On-set only (F)
    F = 1,
    /// On-set and don't-care set (FD)
    FD = 3,
    /// On-set and off-set (FR)
    FR = 5,
    /// On-set, don't-care set, and off-set (FDR)
    FDR = 7,
}

impl CoverType {
    /// Check if this type includes F (ON-set)
    pub fn has_f(&self) -> bool {
        matches!(
            self,
            CoverType::F | CoverType::FD | CoverType::FR | CoverType::FDR
        )
    }

    /// Check if this type includes D (don't-care set)
    pub fn has_d(&self) -> bool {
        matches!(self, CoverType::FD | CoverType::FDR)
    }

    /// Check if this type includes R (OFF-set)
    pub fn has_r(&self) -> bool {
        matches!(self, CoverType::FR | CoverType::FDR)
    }
}

/// A cover representing a Boolean function as sum-of-products (truth table)
///
/// `Cover` is the primary type for working with truth tables and PLA files. It represents
/// Boolean functions as a collection of **cubes** (product terms), where each cube specifies
/// input patterns and corresponding output values.
///
/// # Structure
///
/// A cover consists of:
///
/// - **Inputs** - Boolean variables (columns in truth table)
/// - **Outputs** - Function outputs (can have multiple outputs)
/// - **Cubes** - Product terms, each specifying an input→output mapping
/// - **Cover Type** - Which sets are included (F, FD, FR, or FDR)
/// - **Labels** - Optional variable names for inputs/outputs
///
/// # Dynamic Dimensions
///
/// Unlike the low-level API, `Cover` has **dynamic dimensions** that automatically grow
/// as cubes are added. This means:
///
/// - Start with an empty cover (0 inputs, 0 outputs)
/// - Add cubes of any size - dimensions expand automatically
/// - No need to pre-declare or track dimensions
/// - Existing cubes are padded with don't-cares when dimensions grow
///
/// This makes `Cover` much easier to use than the low-level [`crate::espresso::EspressoCover`].
///
/// # Cover Types
///
/// Four types specify which sets the cover contains:
///
/// - **F** - ON-set only (where outputs are 1)
/// - **FD** - ON-set + Don't-cares (flexibility for minimisation)
/// - **FR** - ON-set + OFF-set (explicit 0s and 1s)
/// - **FDR** - Complete (all three sets)
///
/// See [`CoverType`] for details.
///
/// # Input/Output Encoding
///
/// **Inputs** use three-valued logic:
/// - `Some(true)` or `1` - Variable must be 1
/// - `Some(false)` or `0` - Variable must be 0
/// - `None` or `-` - Don't care (variable can be either)
///
/// **Outputs** specify membership in F/D/R sets:
/// - `Some(true)` - Bit set in F cube (ON-set)
/// - `Some(false)` - Bit set in R cube (OFF-set, only if cover type includes R)
/// - `None` - Bit set in D cube (Don't-care, only if cover type includes D)
///
/// # Thread Safety
///
/// `Cover` is `Send` and `Sync`, allowing it to be freely moved between and shared across threads.
/// Unlike the low-level API, `Cover` doesn't hold a thread-local Espresso instance - it only
/// creates one temporarily when `.minimize()` is called, then releases it immediately after.
/// This makes `Cover` ideal for concurrent applications.
///
/// # Examples
///
/// ## Basic Truth Table
///
/// ```
/// use espresso_logic::{Cover, CoverType, Minimizable};
///
/// // XOR function
/// let mut cover = Cover::new(CoverType::F);
/// cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);   // 01 -> 1
/// cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);   // 10 -> 1
///
/// println!("Before: {} cubes", cover.num_cubes());
/// let minimised = cover.minimize().unwrap();
/// println!("After: {} cubes", minimised.num_cubes());
/// ```
///
/// ## With Labels
///
/// ```
/// use espresso_logic::{Cover, CoverType};
///
/// let mut cover = Cover::with_labels(
///     CoverType::F,
///     &["a", "b", "c"],
///     &["sum", "carry"],
/// );
///
/// println!("Inputs: {:?}", cover.input_labels());
/// println!("Outputs: {:?}", cover.output_labels());
/// ```
///
/// ## From Boolean Expression
///
/// ```
/// use espresso_logic::{BoolExpr, Cover, CoverType, Minimizable};
///
/// # fn main() -> std::io::Result<()> {
/// let expr = BoolExpr::parse("a * b + b * c")?;
/// let mut cover = Cover::new(CoverType::F);
/// cover.add_expr(&expr, "output")?;
///
/// let minimised = cover.minimize()?;
/// let result = minimised.to_expr("output")?;
/// println!("{}", result);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Cover {
    /// Number of input variables
    num_inputs: usize,
    /// Number of output variables
    num_outputs: usize,
    /// Input label manager (prefix: 'x')
    input_labels: LabelManager<'x'>,
    /// Output label manager (prefix: 'y')
    output_labels: LabelManager<'y'>,
    /// Cubes with their type (F/D/R) and data
    cubes: Vec<Cube>,
    /// Cover type (F, FD, FR, or FDR)
    cover_type: CoverType,
}

impl Cover {
    /// Create a new empty cover with the specified type
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cover, CoverType};
    ///
    /// let cover = Cover::new(CoverType::F);
    /// assert_eq!(cover.num_inputs(), 0);
    /// assert_eq!(cover.num_outputs(), 0);
    /// ```
    pub fn new(cover_type: CoverType) -> Self {
        Cover {
            num_inputs: 0,
            num_outputs: 0,
            input_labels: LabelManager::new(),
            output_labels: LabelManager::new(),
            cubes: Vec::new(),
            cover_type,
        }
    }

    /// Create a new cover with pre-defined labels
    ///
    /// This is useful when you know the variable names in advance.
    /// The dimensions are set based on the label counts.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cover, CoverType};
    ///
    /// let cover = Cover::with_labels(
    ///     CoverType::F,
    ///     &["a", "b", "c"],
    ///     &["out"],
    /// );
    /// assert_eq!(cover.num_inputs(), 3);
    /// assert_eq!(cover.num_outputs(), 1);
    /// ```
    pub fn with_labels<S: AsRef<str>>(
        cover_type: CoverType,
        input_labels: &[S],
        output_labels: &[S],
    ) -> Self {
        let input_label_vec: Vec<Arc<str>> =
            input_labels.iter().map(|s| Arc::from(s.as_ref())).collect();
        let output_label_vec: Vec<Arc<str>> = output_labels
            .iter()
            .map(|s| Arc::from(s.as_ref()))
            .collect();

        Cover {
            num_inputs: input_label_vec.len(),
            num_outputs: output_label_vec.len(),
            input_labels: LabelManager::from_labels(input_label_vec),
            output_labels: LabelManager::from_labels(output_label_vec),
            cubes: Vec::new(),
            cover_type,
        }
    }

    /// Get the number of inputs
    pub fn num_inputs(&self) -> usize {
        self.num_inputs
    }

    /// Get the number of outputs
    pub fn num_outputs(&self) -> usize {
        self.num_outputs
    }

    /// Get the number of cubes (for F/FD types, only counts F cubes; for FR/FDR, counts all)
    pub fn num_cubes(&self) -> usize {
        if self.cover_type.has_r() {
            self.cubes.len()
        } else {
            // F/FD: only count F cubes
            self.cubes
                .iter()
                .filter(|cube| cube.cube_type() == CubeType::F)
                .count()
        }
    }

    /// Get the cover type (F, FD, FR, or FDR)
    pub fn cover_type(&self) -> CoverType {
        self.cover_type
    }

    /// Get input variable labels
    ///
    /// Returns a slice of `Arc<str>` for efficient access to variable names.
    pub fn input_labels(&self) -> &[Arc<str>] {
        self.input_labels.as_slice()
    }

    /// Get output variable labels
    ///
    /// Returns a slice of `Arc<str>` for efficient access to variable names.
    pub fn output_labels(&self) -> &[Arc<str>] {
        self.output_labels.as_slice()
    }

    /// Iterate over cubes as `Cube` references
    ///
    /// Returns an iterator over `&Cube` objects.
    ///
    /// # Example
    ///
    /// ```
    /// use espresso_logic::{Cover, CoverType};
    ///
    /// let mut cover = Cover::new(CoverType::F);
    /// cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
    ///
    /// for cube in cover.cubes() {
    ///     println!("Inputs: {:?}, Outputs: {:?}", cube.inputs(), cube.outputs());
    /// }
    /// ```
    pub fn cubes(&self) -> CubesIter<'_, &Cube> {
        // For F-type covers, only return F cubes; for FD/FR/FDR, return all
        let cover_type = self.cover_type;
        CubesIter {
            iter: Box::new(
                self.cubes.iter().filter(move |cube| {
                    cover_type != CoverType::F || cube.cube_type() == CubeType::F
                }),
            ),
        }
    }

    /// Iterate over cubes (inputs, outputs)
    ///
    /// Returns cubes in a format compatible with add_cube (owned vecs for easy use)
    pub fn cubes_iter(&self) -> CubesIter<'_, CubeData> {
        let cover_type = self.cover_type;
        CubesIter {
            iter: Box::new(
                self.cubes
                    .iter()
                    .filter(move |cube| {
                        cover_type != CoverType::F || cube.cube_type() == CubeType::F
                    })
                    .map(|cube| {
                        let inputs = cube.inputs().to_vec();
                        let outputs: Vec<Option<bool>> =
                            cube.outputs().iter().map(|&b| Some(b)).collect();
                        (inputs, outputs)
                    }),
            ),
        }
    }

    /// Add a cube to the cover
    ///
    /// The cover dimensions grow automatically if the cube is larger.
    /// Outputs use PLA-style notation:
    /// - `Some(true)` or `'1'` → bit set in F cube (ON-set)
    /// - `Some(false)` or `'0'` → bit set in R cube (OFF-set, only if cover type includes R)
    /// - `None` or `'-'` → bit set in D cube (Don't-care, only if cover type includes D)
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cover, CoverType};
    ///
    /// let mut cover = Cover::new(CoverType::F);
    /// cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
    /// assert_eq!(cover.num_inputs(), 2);
    /// assert_eq!(cover.num_outputs(), 1);
    ///
    /// // Add a larger cube - dimensions grow automatically
    /// cover.add_cube(&[Some(true), Some(false), Some(true)], &[Some(true)]);
    /// assert_eq!(cover.num_inputs(), 3);
    /// ```
    pub fn add_cube(&mut self, inputs: &[Option<bool>], outputs: &[Option<bool>]) {
        // Grow dimensions if needed
        self.grow_to_fit(inputs.len(), outputs.len());

        // Pad inputs/outputs if they're smaller than current dimensions
        let mut padded_inputs = inputs.to_vec();
        padded_inputs.resize(self.num_inputs, None);

        let mut padded_outputs = outputs.to_vec();
        padded_outputs.resize(self.num_outputs, None);

        // Parse outputs following Espresso C convention
        // Create separate F, D, R cubes from a single line based on output values
        let mut f_outputs = Vec::with_capacity(self.num_outputs);
        let mut d_outputs = Vec::with_capacity(self.num_outputs);
        let mut r_outputs = Vec::with_capacity(self.num_outputs);
        let mut has_f = false;
        let mut has_d = false;
        let mut has_r = false;

        for &out in padded_outputs.iter() {
            match out {
                Some(true) if self.cover_type.has_f() => {
                    // '1' → bit set in F cube
                    f_outputs.push(true);
                    d_outputs.push(false);
                    r_outputs.push(false);
                    has_f = true;
                }
                Some(false) if self.cover_type.has_r() => {
                    // '0' → bit set in R cube
                    f_outputs.push(false);
                    d_outputs.push(false);
                    r_outputs.push(true);
                    has_r = true;
                }
                None if self.cover_type.has_d() => {
                    // None/'-' → bit set in D cube
                    f_outputs.push(false);
                    d_outputs.push(true);
                    r_outputs.push(false);
                    has_d = true;
                }
                _ => {
                    // Type not supported or unset bit
                    f_outputs.push(false);
                    d_outputs.push(false);
                    r_outputs.push(false);
                }
            }
        }

        // Add cubes only if they have meaningful outputs
        if has_f {
            self.cubes
                .push(Cube::new(&padded_inputs, &f_outputs, CubeType::F));
        }
        if has_d {
            self.cubes
                .push(Cube::new(&padded_inputs, &d_outputs, CubeType::D));
        }
        if has_r {
            self.cubes
                .push(Cube::new(&padded_inputs, &r_outputs, CubeType::R));
        }
    }

    /// Grow the cover to fit at least the specified dimensions
    ///
    /// This extends all existing cubes. If the cover already has labels (from expressions
    /// or from `with_labels`), new labels are auto-generated to maintain consistency.
    /// If the cover has no labels, it remains unlabeled.
    fn grow_to_fit(&mut self, min_inputs: usize, min_outputs: usize) {
        // Grow inputs if needed
        if min_inputs > self.num_inputs {
            self.num_inputs = min_inputs;

            // Extend all existing cubes
            for cube in &mut self.cubes {
                let mut new_inputs = cube.inputs.to_vec();
                new_inputs.resize(self.num_inputs, None);
                cube.inputs = new_inputs.into();
            }

            // If the cover already has labels, extend them to maintain consistency
            if !self.input_labels.is_empty() {
                self.input_labels.backfill_to(self.num_inputs);
            }
        }

        // Grow outputs if needed
        if min_outputs > self.num_outputs {
            self.num_outputs = min_outputs;

            // Extend all existing cubes
            for cube in &mut self.cubes {
                let mut new_outputs = cube.outputs.to_vec();
                new_outputs.resize(self.num_outputs, false);
                cube.outputs = new_outputs.into();
            }

            // If the cover already has labels, extend them to maintain consistency
            if !self.output_labels.is_empty() {
                self.output_labels.backfill_to(self.num_outputs);
            }
        }
    }
}

impl Default for Cover {
    fn default() -> Self {
        Self::new(CoverType::F)
    }
}

#[cfg(test)]
mod tests;
