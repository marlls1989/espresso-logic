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
pub mod error;
mod expressions;
mod iterators;
mod minimisation;
mod minterm;
pub mod pla;

// Public re-exports - core types
pub use cubes::{Cube, CubeType};
pub use error::{AddExprError, CoverError, ToExprError};
pub use iterators::{CubesIter, ToExprs};
pub use minimisation::Minimizable;
pub use minterm::Minterm;

use minterm::Minterm as InternalMinterm;
use std::sync::Arc;

/// Build a variable header of length `target_len`, extending `current` with auto-generated
/// `{prefix}{n}` names that avoid colliding with names already present.
pub(crate) fn extend_header(
    current: &[Arc<str>],
    target_len: usize,
    prefix: char,
) -> Arc<[Arc<str>]> {
    let mut names: Vec<Arc<str>> = current.to_vec();
    while names.len() < target_len {
        let mut n = names.len();
        let label = loop {
            let candidate = format!("{prefix}{n}");
            if !names.iter().any(|existing| existing.as_ref() == candidate) {
                break candidate;
            }
            n += 1;
        };
        names.push(Arc::from(label.as_str()));
    }
    names.into()
}

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
    /// Canonical input variable header, shared by every cube's input minterm.
    ///
    /// Always has one name per input position (auto-generated `x0, x1, …` when unlabeled), so it
    /// can serve as the shared `Arc` for the minterm fast-comparison path.
    input_vars: Arc<[Arc<str>]>,
    /// Canonical output variable header, shared by every cube's output minterm.
    output_vars: Arc<[Arc<str>]>,
    /// Whether input names were explicitly supplied (vs. auto-generated); controls PLA `.ilb`.
    input_labeled: bool,
    /// Whether output names were explicitly supplied; controls PLA `.ob`.
    output_labeled: bool,
    /// Cubes (merged tri-state product terms).
    pub(crate) cubes: Vec<Cube>,
    /// Cover type (F, FD, FR, or FDR)
    pub(crate) cover_type: CoverType,
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
            input_vars: Vec::new().into(),
            output_vars: Vec::new().into(),
            input_labeled: false,
            output_labeled: false,
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
        let input_vars: Arc<[Arc<str>]> =
            input_labels.iter().map(|s| Arc::from(s.as_ref())).collect();
        let output_vars: Arc<[Arc<str>]> = output_labels
            .iter()
            .map(|s| Arc::from(s.as_ref()))
            .collect();

        Cover {
            input_labeled: !input_vars.is_empty(),
            output_labeled: !output_vars.is_empty(),
            input_vars,
            output_vars,
            cubes: Vec::new(),
            cover_type,
        }
    }

    /// Get the number of inputs
    pub fn num_inputs(&self) -> usize {
        self.input_vars.len()
    }

    /// Get the number of outputs
    pub fn num_outputs(&self) -> usize {
        self.output_vars.len()
    }

    /// Get the number of cubes (for F/FD types, only counts F cubes; for FR/FDR, counts all)
    pub fn num_cubes(&self) -> usize {
        if self.cover_type.has_r() {
            self.cubes.len()
        } else {
            // F/FD: only count F cubes.
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
    /// Returns a slice of `Arc<str>` for efficient access to variable names. The slice is empty
    /// for an unlabeled cover (even though inputs are internally named `x0, x1, …`).
    pub fn input_labels(&self) -> &[Arc<str>] {
        if self.input_labeled {
            &self.input_vars
        } else {
            &[]
        }
    }

    /// Get output variable labels
    ///
    /// Returns a slice of `Arc<str>` for efficient access to variable names. The slice is empty
    /// for an unlabeled cover.
    pub fn output_labels(&self) -> &[Arc<str>] {
        if self.output_labeled {
            &self.output_vars
        } else {
            &[]
        }
    }

    /// The shared input variable header (one name per input, auto-generated when unlabeled).
    pub(crate) fn input_vars(&self) -> &Arc<[Arc<str>]> {
        &self.input_vars
    }

    /// The shared output variable header.
    pub(crate) fn output_vars(&self) -> &Arc<[Arc<str>]> {
        &self.output_vars
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
                self.cubes
                    .iter()
                    .filter(move |cube| match cube.cube_type() {
                        CubeType::D => cover_type.has_d(),
                        CubeType::R => cover_type.has_r(),
                        CubeType::F => cover_type.has_f(),
                    }),
            ),
        }
    }

    /// Input minterms of the F cubes that assert `output_idx`.
    ///
    /// These are the product terms of a single output (the sum-of-products for that output).
    /// Each minterm carries the cover's shared input header.
    pub(crate) fn output_product_terms(&self, output_idx: usize) -> Vec<Minterm> {
        self.cubes
            .iter()
            .filter(|cube| cube.cube_type() == CubeType::F && cube.asserts(output_idx))
            .map(|cube| cube.inputs().clone())
            .collect()
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

        // Pad outputs to current dimensions.
        let mut padded_outputs = outputs.to_vec();
        padded_outputs.resize(self.num_outputs(), None);

        // Parse outputs following the Espresso C convention: split a single line into separate
        // F, D, R cubes based on the per-output values.
        let mut f_outputs = Vec::with_capacity(self.num_outputs());
        let mut d_outputs = Vec::with_capacity(self.num_outputs());
        let mut r_outputs = Vec::with_capacity(self.num_outputs());
        let mut has_f = false;
        let mut has_d = false;
        let mut has_r = false;

        for &out in padded_outputs.iter() {
            match out {
                Some(true) if self.cover_type.has_f() => {
                    f_outputs.push(true);
                    d_outputs.push(false);
                    r_outputs.push(false);
                    has_f = true;
                }
                Some(false) if self.cover_type.has_r() => {
                    f_outputs.push(false);
                    d_outputs.push(false);
                    r_outputs.push(true);
                    has_r = true;
                }
                None if self.cover_type.has_d() => {
                    f_outputs.push(false);
                    d_outputs.push(true);
                    r_outputs.push(false);
                    has_d = true;
                }
                _ => {
                    f_outputs.push(false);
                    d_outputs.push(false);
                    r_outputs.push(false);
                }
            }
        }

        let inputs_minterm = self.input_minterm(inputs);
        if has_f {
            let cube = Cube::new(
                inputs_minterm.clone(),
                self.membership_minterm(&f_outputs),
                CubeType::F,
            );
            self.cubes.push(cube);
        }
        if has_d {
            let cube = Cube::new(
                inputs_minterm.clone(),
                self.membership_minterm(&d_outputs),
                CubeType::D,
            );
            self.cubes.push(cube);
        }
        if has_r {
            let cube = Cube::new(
                inputs_minterm,
                self.membership_minterm(&r_outputs),
                CubeType::R,
            );
            self.cubes.push(cube);
        }
    }

    /// Build an input minterm (padded to the current input dimension) on the shared input header.
    pub(crate) fn input_minterm(&self, raw: &[Option<bool>]) -> InternalMinterm {
        let mut values = raw.to_vec();
        values.resize(self.num_inputs(), None);
        InternalMinterm::from_values(Arc::clone(&self.input_vars), values)
    }

    /// Build an output-membership minterm (`Some(true)`=asserted) on the shared output header.
    fn membership_minterm(&self, mask: &[bool]) -> InternalMinterm {
        InternalMinterm::from_values(Arc::clone(&self.output_vars), mask.iter().map(|&b| Some(b)))
    }

    /// Grow the cover to fit at least the specified dimensions.
    ///
    /// Extends the shared headers (auto-generating labels that avoid collisions) and re-points every
    /// existing cube's minterms onto the new headers. New inputs become don't-care; new outputs are
    /// unasserted.
    fn grow_to_fit(&mut self, min_inputs: usize, min_outputs: usize) {
        if min_inputs > self.num_inputs() {
            let new_vars = extend_header(&self.input_vars, min_inputs, 'x');
            for cube in &mut self.cubes {
                cube.inputs = cube.inputs.project_onto(&new_vars);
            }
            self.input_vars = new_vars;
        }

        if min_outputs > self.num_outputs() {
            let new_vars = extend_header(&self.output_vars, min_outputs, 'y');
            for cube in &mut self.cubes {
                // Membership grows with `Some(false)` (unasserted), not don't-care.
                let mut mask: Vec<Option<bool>> = cube.outputs.iter().collect();
                mask.resize(new_vars.len(), Some(false));
                cube.outputs = InternalMinterm::from_values(Arc::clone(&new_vars), mask);
            }
            self.output_vars = new_vars;
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
