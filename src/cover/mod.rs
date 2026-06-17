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
//! Unlike the low-level API, a [`Cover`] grows its dimensions automatically instead of needing
//! them fixed up front — but *how* it grows depends on the label type:
//!
//! - An **anonymous** [`Cover<Anonymous, Anonymous>`](Cover) (built with [`Cover::<Anonymous, Anonymous>::anonymous`]) grows
//!   **positionally**: [`push`](Cover::push) / [`from_cubes`](Cover::from_cubes) widen the cover to
//!   the widest cube seen, matching variables by index.
//! - A **labelled** `Cover<I, O>` (e.g. the default `Cover<Arc<str>, Arc<str>>` built with
//!   [`Cover::new`] + [`add_expr`](Cover::add_expr), [`with_labels`](Cover::with_labels), or a PLA
//!   file) grows by **merging variable names**: new labels extend the header, shared labels line up
//!   by identity.
//!
//! The two modes never mix implicitly — converting between them is the explicit
//! [`relabel`](Cover::relabel) / [`anonymize`](Cover::anonymize).
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```
//! use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};
//!
//! // Create a cover for XOR function
//! let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
//! cover.push(Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F));   // 01 -> 1
//! cover.push(Cube::anonymous(&[Some(true), Some(false)], &[true], CubeType::F));   // 10 -> 1
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
mod label;
mod minimisation;
mod minterm;
pub mod pla;
mod symbols;

// Public re-exports - core types
pub use cubes::{Cube, CubeType};
pub use error::{AddExprError, CoverError, ToExprError};
pub use iterators::{CubesIter, ToExprs};
pub use label::{Anonymous, Label};
pub use minimisation::Minimizable;
pub use minterm::Minterm;
pub use symbols::Symbols;

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
/// # Generic over the label types
///
/// `Cover<I, O>` is generic over its **input** label type `I` and **output** label type `O`, both
/// defaulting to `Arc<str>` (so plain `Cover` is the string-labelled form). The two are independent:
/// a cover can have, e.g., labelled inputs and an anonymous output (`Cover<Arc<str>, Anonymous>`). The
/// anonymous form [`Cover<Anonymous, Anonymous>`](Cover) carries no names and is purely positional. Label types are
/// kept apart by the type system — see [`relabel`](Cover::relabel) /
/// [`relabel_inputs`](Cover::relabel_inputs) / [`relabel_outputs`](Cover::relabel_outputs) /
/// [`anonymize`](Cover::anonymize) for explicit conversion.
///
/// # Dynamic Dimensions
///
/// Unlike the low-level API, a `Cover` grows its dimensions automatically as cubes are added, so
/// there is no need to pre-declare or track them; existing cubes are padded with don't-cares when
/// the cover widens. *How* it grows depends on the label type:
///
/// - An **anonymous** [`Cover<Anonymous, Anonymous>`](Cover) grows **positionally** via
///   [`push`](Cover::push) / [`from_cubes`](Cover::from_cubes) (variables matched by index).
/// - A **labelled** `Cover<I, O>` grows by **merging variable names** via
///   [`add_expr`](Cover::add_expr) / [`with_labels`](Cover::with_labels) / PLA input.
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
/// use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};
///
/// // XOR function
/// let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
/// cover.push(Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F));   // 01 -> 1
/// cover.push(Cube::anonymous(&[Some(true), Some(false)], &[true], CubeType::F));   // 10 -> 1
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
pub struct Cover<I = Arc<str>, O = Arc<str>> {
    /// Canonical input symbol table, shared by every cube's input minterm.
    ///
    /// Always has one name per input position (auto-generated `x0, x1, …` when unlabeled), so it
    /// can serve as the shared `Arc` for the minterm fast-comparison path.
    input_symbols: Arc<Symbols<I>>,
    /// Canonical output symbol table, shared by every cube's output minterm.
    output_symbols: Arc<Symbols<O>>,
    /// Whether input names were explicitly supplied (vs. auto-generated); controls PLA `.ilb`.
    input_labeled: bool,
    /// Whether output names were explicitly supplied; controls PLA `.ob`.
    output_labeled: bool,
    /// Cubes (merged tri-state product terms).
    pub(crate) cubes: Vec<Cube<I, O>>,
    /// Cover type (F, FD, FR, or FDR)
    pub(crate) cover_type: CoverType,
}

impl Cover<Arc<str>, Arc<str>> {
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
            input_symbols: Symbols::empty(),
            output_symbols: Symbols::empty(),
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
            input_symbols: Symbols::new(input_vars),
            output_symbols: Symbols::new(output_vars),
            cubes: Vec::new(),
            cover_type,
        }
    }
}

impl<I, O> Cover<I, O> {
    /// Create a new empty **anonymous** cover (no variable labels) of the given type.
    ///
    /// Positions are purely positional; dimensions grow as cubes are added. This is the generic
    /// constructor for any label types (e.g. `Cover::<Anonymous, Anonymous>::anonymous(CoverType::F)`); for named
    /// `Arc<str>` covers use [`Cover::new`] / [`Cover::with_labels`].
    pub fn anonymous(cover_type: CoverType) -> Self {
        Cover {
            input_symbols: Symbols::empty(),
            output_symbols: Symbols::empty(),
            input_labeled: false,
            output_labeled: false,
            cubes: Vec::new(),
            cover_type,
        }
    }

    /// Re-express this cover over different label types, position-for-position.
    ///
    /// This is the **explicit** way to relabel or anonymise a cover — labelling and anonymisation
    /// never happen implicitly. The new symbol tables must have the same arities as this cover.
    /// To change only one side, use [`relabel_inputs`](Self::relabel_inputs) /
    /// [`relabel_outputs`](Self::relabel_outputs).
    pub fn relabel<I2, O2>(
        self,
        input_symbols: Arc<Symbols<I2>>,
        output_symbols: Arc<Symbols<O2>>,
    ) -> Cover<I2, O2> {
        assert_eq!(
            input_symbols.arity(),
            self.num_inputs(),
            "relabel: input arity mismatch"
        );
        assert_eq!(
            output_symbols.arity(),
            self.num_outputs(),
            "relabel: output arity mismatch"
        );
        let input_labeled = input_symbols.is_labeled();
        let output_labeled = output_symbols.is_labeled();
        let cubes = self
            .cubes
            .into_iter()
            .map(|cube| {
                Cube::new(
                    Minterm::from_symbols(Arc::clone(&input_symbols), cube.inputs.iter()),
                    Minterm::from_symbols(Arc::clone(&output_symbols), cube.outputs.iter()),
                    cube.cube_type(),
                )
            })
            .collect();
        Cover {
            input_symbols,
            output_symbols,
            input_labeled,
            output_labeled,
            cubes,
            cover_type: self.cover_type,
        }
    }

    /// Re-express only the **input** variables over a new label type, keeping the outputs as-is.
    ///
    /// The new input table must have the same input arity as this cover.
    pub fn relabel_inputs<I2>(self, input_symbols: Arc<Symbols<I2>>) -> Cover<I2, O> {
        assert_eq!(
            input_symbols.arity(),
            self.num_inputs(),
            "relabel_inputs: input arity mismatch"
        );
        let input_labeled = input_symbols.is_labeled();
        let cubes = self
            .cubes
            .into_iter()
            .map(|cube| {
                Cube::new(
                    Minterm::from_symbols(Arc::clone(&input_symbols), cube.inputs.iter()),
                    cube.outputs,
                    cube.set,
                )
            })
            .collect();
        Cover {
            input_symbols,
            output_symbols: self.output_symbols,
            input_labeled,
            output_labeled: self.output_labeled,
            cubes,
            cover_type: self.cover_type,
        }
    }

    /// Re-express only the **output** variables over a new label type, keeping the inputs as-is.
    ///
    /// The new output table must have the same output arity as this cover.
    pub fn relabel_outputs<O2>(self, output_symbols: Arc<Symbols<O2>>) -> Cover<I, O2> {
        assert_eq!(
            output_symbols.arity(),
            self.num_outputs(),
            "relabel_outputs: output arity mismatch"
        );
        let output_labeled = output_symbols.is_labeled();
        let cubes = self
            .cubes
            .into_iter()
            .map(|cube| {
                Cube::new(
                    cube.inputs,
                    Minterm::from_symbols(Arc::clone(&output_symbols), cube.outputs.iter()),
                    cube.set,
                )
            })
            .collect();
        Cover {
            input_symbols: self.input_symbols,
            output_symbols,
            input_labeled: self.input_labeled,
            output_labeled,
            cubes,
            cover_type: self.cover_type,
        }
    }

    /// Drop all labels, yielding a positional [`Cover<Anonymous, Anonymous>`](Cover) (explicit anonymisation).
    pub fn anonymize(self) -> Cover<Anonymous, Anonymous> {
        let (ni, no) = (self.num_inputs(), self.num_outputs());
        self.relabel(Symbols::anonymous(ni), Symbols::anonymous(no))
    }

    /// Get the number of inputs
    pub fn num_inputs(&self) -> usize {
        self.input_symbols.arity()
    }

    /// Get the number of outputs
    pub fn num_outputs(&self) -> usize {
        self.output_symbols.arity()
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

    /// The shared input symbol table.
    pub(crate) fn input_symbols(&self) -> &Arc<Symbols<I>> {
        &self.input_symbols
    }

    /// The shared output symbol table.
    pub(crate) fn output_symbols(&self) -> &Arc<Symbols<O>> {
        &self.output_symbols
    }

    /// Iterate over cubes as `Cube` references
    ///
    /// Returns an iterator over `&Cube` objects.
    ///
    /// # Example
    ///
    /// ```
    /// use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType};
    ///
    /// let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    /// cover.push(Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F));
    ///
    /// for cube in cover.cubes() {
    ///     println!("Inputs: {:?}, Outputs: {:?}", cube.inputs(), cube.outputs());
    /// }
    /// ```
    pub fn cubes(&self) -> CubesIter<'_, &Cube<I, O>> {
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
}

impl<I: Clone, O> Cover<I, O> {
    /// Input minterms of the F cubes that assert `output_idx` (the product terms of that output).
    pub(crate) fn output_product_terms(&self, output_idx: usize) -> Arc<[Minterm<I>]> {
        self.cubes
            .iter()
            .filter(|cube| cube.cube_type() == CubeType::F && cube.asserts(output_idx))
            .map(|cube| cube.inputs().clone())
            .collect()
    }
}

/// Re-point an anonymous cube positionally onto target symbol tables: inputs beyond the cube's own
/// arity become don't-care, output membership beyond it becomes unasserted (`Some(false)`).
fn repoint_anonymous(
    cube: &Cube<Anonymous, Anonymous>,
    input_symbols: &Arc<Symbols<Anonymous>>,
    output_symbols: &Arc<Symbols<Anonymous>>,
) -> Cube<Anonymous, Anonymous> {
    let ni = input_symbols.arity();
    let no = output_symbols.arity();
    let im = Minterm::from_symbols(
        Arc::clone(input_symbols),
        (0..ni).map(|i| cube.inputs().value_at(i)),
    );
    let om = Minterm::from_symbols(
        Arc::clone(output_symbols),
        (0..no).map(|i| Some(cube.asserts(i))),
    );
    Cube::new(im, om, cube.cube_type())
}

impl Cover<Anonymous, Anonymous> {
    /// Build an **anonymous** cover from a collection of typed [`Cube<Anonymous, Anonymous>`](Cube)s.
    ///
    /// The cover's dimensions are the widest input/output arity seen across `cubes`; each cube is
    /// re-pointed positionally onto the shared anonymous tables (shorter inputs padded with
    /// don't-cares, shorter membership masks padded unasserted). Each cube keeps its own
    /// [`CubeType`] (F/D/R); build them with [`Cube::anonymous`].
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cover, CoverType, Cube, CubeType};
    ///
    /// // XOR: 01 -> 1, 10 -> 1.
    /// let cover = Cover::from_cubes(CoverType::F, [
    ///     Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F),
    ///     Cube::anonymous(&[Some(true), Some(false)], &[true], CubeType::F),
    /// ]);
    /// assert_eq!(cover.num_inputs(), 2);
    /// assert_eq!(cover.num_cubes(), 2);
    /// ```
    pub fn from_cubes(
        cover_type: CoverType,
        cubes: impl IntoIterator<Item = Cube<Anonymous, Anonymous>>,
    ) -> Cover<Anonymous, Anonymous> {
        let cubes: Vec<Cube<Anonymous, Anonymous>> = cubes.into_iter().collect();
        let ni = cubes
            .iter()
            .map(|c| c.inputs().num_vars())
            .max()
            .unwrap_or(0);
        let no = cubes
            .iter()
            .map(|c| c.outputs().num_vars())
            .max()
            .unwrap_or(0);
        let input_symbols = Symbols::anonymous(ni);
        let output_symbols = Symbols::anonymous(no);
        let cubes = cubes
            .iter()
            .map(|cube| repoint_anonymous(cube, &input_symbols, &output_symbols))
            .collect();
        Cover {
            input_symbols,
            output_symbols,
            input_labeled: false,
            output_labeled: false,
            cubes,
            cover_type,
        }
    }

    /// Append a single typed [`Cube<Anonymous, Anonymous>`](Cube) to this anonymous cover, growing its dimensions
    /// to fit (shorter inputs become don't-care, new output columns unasserted).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType};
    ///
    /// let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    /// cover.push(Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F));
    /// assert_eq!(cover.num_inputs(), 2);
    ///
    /// // A wider cube extends the dimensions position-wise.
    /// cover.push(Cube::anonymous(&[Some(true), Some(false), Some(true)], &[true], CubeType::F));
    /// assert_eq!(cover.num_inputs(), 3);
    /// ```
    pub fn push(&mut self, cube: Cube<Anonymous, Anonymous>) {
        self.grow_to_fit(cube.inputs().num_vars(), cube.outputs().num_vars());
        let repointed = repoint_anonymous(&cube, &self.input_symbols, &self.output_symbols);
        self.cubes.push(repointed);
    }

    /// Positionally widen this anonymous cover to at least the given dimensions (new input positions
    /// are don't-care, new output positions unasserted). No labels are synthesised.
    fn grow_to_fit(&mut self, min_inputs: usize, min_outputs: usize) {
        if min_inputs > self.num_inputs() {
            // A `Cover<Anonymous, Anonymous>` is always anonymous, so widening is just a wider anonymous table.
            let new_syms = Symbols::anonymous(min_inputs);
            for cube in &mut self.cubes {
                cube.inputs = Minterm::from_symbols(
                    Arc::clone(&new_syms),
                    (0..min_inputs).map(|i| cube.inputs.value_at(i)),
                );
            }
            self.input_symbols = new_syms;
        }

        if min_outputs > self.num_outputs() {
            let new_syms = Symbols::anonymous(min_outputs);
            for cube in &mut self.cubes {
                let old = cube.outputs.num_vars();
                cube.outputs = Minterm::from_symbols(
                    Arc::clone(&new_syms),
                    (0..min_outputs).map(|i| {
                        if i < old {
                            cube.outputs.value_at(i)
                        } else {
                            Some(false)
                        }
                    }),
                );
            }
            self.output_symbols = new_syms;
        }
    }
}

// ===== Cover combination (`extend` / `merge`) =====
//
// The by-name vs positional behaviour is keyed on the *concrete* label type: `Anonymous` inputs align
// by position, `Arc<str>` inputs by name — so the strategies live in separate concrete impls sharing
// the `combine_*` / `rebuild_output` helpers below.

/// Build a cube's new output-membership minterm over `new_output`: for each old output position the
/// cube asserts, set the mapped new position; everything else is unasserted (`Some(false)`).
fn rebuild_output<I, O>(
    cube: &Cube<I, O>,
    new_output: &Arc<Symbols<O>>,
    out_map: &[usize],
    new_no: usize,
) -> Minterm<O> {
    let mut mask = vec![false; new_no];
    for (old, &newp) in out_map.iter().enumerate() {
        if cube.asserts(old) {
            mask[newp] = true;
        }
    }
    Minterm::from_symbols(Arc::clone(new_output), mask.into_iter().map(Some))
}

/// `a`'s and `b`'s output index → new index for **append** (extend, anonymous outputs): `a` keeps its
/// columns, `b`'s are appended after them.
fn append_output_maps(a_no: usize, b_no: usize) -> (Vec<usize>, Vec<usize>) {
    ((0..a_no).collect(), (0..b_no).map(|j| a_no + j).collect())
}

/// Output index maps for **overlay** (merge, anonymous outputs): same position ⇒ same output.
fn overlay_output_maps(a_no: usize, b_no: usize) -> (Vec<usize>, Vec<usize>) {
    ((0..a_no).collect(), (0..b_no).collect())
}

/// Combine two covers with **positional** input alignment (`I = Anonymous`): inputs grow to the wider arity,
/// padding don't-cares. Outputs are governed by the caller-supplied `new_output` table and index maps.
fn combine_positional_inputs<O>(
    a: &Cover<Anonymous, O>,
    b: &Cover<Anonymous, O>,
    new_output: Arc<Symbols<O>>,
    a_out_map: &[usize],
    b_out_map: &[usize],
) -> Cover<Anonymous, O> {
    let new_ni = a.num_inputs().max(b.num_inputs());
    let new_input = Symbols::anonymous(new_ni);
    let new_no = new_output.arity();
    let rebuild_in = |m: &Minterm<Anonymous>| {
        Minterm::from_symbols(Arc::clone(&new_input), (0..new_ni).map(|i| m.value_at(i)))
    };
    let cubes = a
        .cubes
        .iter()
        .map(|c| {
            Cube::new(
                rebuild_in(c.inputs()),
                rebuild_output(c, &new_output, a_out_map, new_no),
                c.set,
            )
        })
        .chain(b.cubes.iter().map(|c| {
            Cube::new(
                rebuild_in(c.inputs()),
                rebuild_output(c, &new_output, b_out_map, new_no),
                c.set,
            )
        }))
        .collect();
    Cover {
        input_labeled: false,
        output_labeled: new_output.is_labeled(),
        input_symbols: new_input,
        output_symbols: new_output,
        cubes,
        cover_type: a.cover_type,
    }
}

/// Combine two covers with **by-name** input alignment (`I = Arc<str>`): the new input header is `a`'s
/// names followed by `b`'s new ones; every cube is reprojected by variable identity (missing inputs
/// become don't-care). Outputs are governed by the caller-supplied `new_output` table and index maps.
fn combine_named_inputs<O>(
    a: &Cover<Arc<str>, O>,
    b: &Cover<Arc<str>, O>,
    new_output: Arc<Symbols<O>>,
    a_out_map: &[usize],
    b_out_map: &[usize],
) -> Cover<Arc<str>, O> {
    let mut header: Vec<Arc<str>> = a.input_symbols.labels().to_vec();
    for name in b.input_symbols.labels() {
        if !header.iter().any(|n| n == name) {
            header.push(Arc::clone(name));
        }
    }
    let new_input = Symbols::new(header.into());
    let new_no = new_output.arity();
    let rebuild_in = |m: &Minterm<Arc<str>>| m.project_onto(&new_input);
    let cubes = a
        .cubes
        .iter()
        .map(|c| {
            Cube::new(
                rebuild_in(c.inputs()),
                rebuild_output(c, &new_output, a_out_map, new_no),
                c.set,
            )
        })
        .chain(b.cubes.iter().map(|c| {
            Cube::new(
                rebuild_in(c.inputs()),
                rebuild_output(c, &new_output, b_out_map, new_no),
                c.set,
            )
        }))
        .collect();
    Cover {
        input_labeled: new_input.is_labeled(),
        output_labeled: new_output.is_labeled(),
        input_symbols: new_input,
        output_symbols: new_output,
        cubes,
        cover_type: a.cover_type,
    }
}

impl Cover<Anonymous, Anonymous> {
    /// Combine `other` into this anonymous cover by **appending** its outputs after this cover's.
    ///
    /// Inputs are aligned positionally (grown to the wider arity, padded don't-care); the result has
    /// `self.num_outputs() + other.num_outputs()` outputs. Use this to stack two functions into one
    /// multi-output cover. (Contrast [`merge`](Self::merge), which overlays outputs by position.)
    pub fn extend(&mut self, other: &Cover<Anonymous, Anonymous>) {
        let (a_map, b_map) = append_output_maps(self.num_outputs(), other.num_outputs());
        let new_output = Symbols::anonymous(self.num_outputs() + other.num_outputs());
        *self = combine_positional_inputs(self, other, new_output, &a_map, &b_map);
    }

    /// Combine `other` into this anonymous cover, **overlaying** outputs by position: output `i` of
    /// `other` is the same output `i` of `self`. Inputs align positionally; the result has
    /// `max(self.num_outputs(), other.num_outputs())` outputs.
    pub fn merge(&mut self, other: &Cover<Anonymous, Anonymous>) {
        let (a_map, b_map) = overlay_output_maps(self.num_outputs(), other.num_outputs());
        let new_output = Symbols::anonymous(self.num_outputs().max(other.num_outputs()));
        *self = combine_positional_inputs(self, other, new_output, &a_map, &b_map);
    }
}

impl Cover<Arc<str>, Anonymous> {
    /// Like [`Cover::<Anonymous, Anonymous>::extend`](Cover::extend) but inputs align by **name**; outputs (anonymous)
    /// are appended.
    pub fn extend(&mut self, other: &Cover<Arc<str>, Anonymous>) {
        let (a_map, b_map) = append_output_maps(self.num_outputs(), other.num_outputs());
        let new_output = Symbols::anonymous(self.num_outputs() + other.num_outputs());
        *self = combine_named_inputs(self, other, new_output, &a_map, &b_map);
    }

    /// Like [`Cover::<Anonymous, Anonymous>::merge`](Cover::merge) but inputs align by **name**; outputs (anonymous)
    /// overlay by position.
    pub fn merge(&mut self, other: &Cover<Arc<str>, Anonymous>) {
        let (a_map, b_map) = overlay_output_maps(self.num_outputs(), other.num_outputs());
        let new_output = Symbols::anonymous(self.num_outputs().max(other.num_outputs()));
        *self = combine_named_inputs(self, other, new_output, &a_map, &b_map);
    }
}

impl Cover<Arc<str>, Arc<str>> {
    /// Combine `other` into this fully-labelled cover by **variable name**: shared input/output names
    /// line up, new ones are appended. With named outputs, appending and overlaying coincide, so
    /// [`extend`](Self::extend) and [`merge`](Self::merge) are identical here.
    fn combine_by_name(&mut self, other: &Cover<Arc<str>, Arc<str>>) {
        let mut out_header: Vec<Arc<str>> = self.output_symbols.labels().to_vec();
        for name in other.output_symbols.labels() {
            if !out_header.iter().any(|n| n == name) {
                out_header.push(Arc::clone(name));
            }
        }
        let new_output = Symbols::new(out_header.into());
        let a_map: Vec<usize> = (0..self.num_outputs()).collect();
        let b_map: Vec<usize> = other
            .output_symbols
            .labels()
            .iter()
            .map(|name| new_output.index_of(name).expect("output name in union") as usize)
            .collect();
        *self = combine_named_inputs(self, other, new_output, &a_map, &b_map);
    }

    /// Combine `other` by variable name (shared names overlay, new names extend). Identical to
    /// [`merge`](Self::merge) for fully-labelled covers.
    pub fn extend(&mut self, other: &Cover<Arc<str>, Arc<str>>) {
        self.combine_by_name(other);
    }

    /// Combine `other` by variable name (shared names overlay, new names extend). Identical to
    /// [`extend`](Self::extend) for fully-labelled covers.
    pub fn merge(&mut self, other: &Cover<Arc<str>, Arc<str>>) {
        self.combine_by_name(other);
    }
}

impl<O> Cover<Arc<str>, O> {
    /// Get input variable labels.
    ///
    /// Returns a slice of `Arc<str>`; empty for an unlabeled/anonymous cover. Available whatever the
    /// output label type is.
    pub fn input_labels(&self) -> &[Arc<str>] {
        if self.input_labeled {
            self.input_symbols.labels()
        } else {
            &[]
        }
    }
}

impl<I> Cover<I, Arc<str>> {
    /// Get output variable labels.
    ///
    /// Returns a slice of `Arc<str>`; empty for an unlabeled/anonymous cover. Available whatever the
    /// input label type is.
    pub fn output_labels(&self) -> &[Arc<str>] {
        if self.output_labeled {
            self.output_symbols.labels()
        } else {
            &[]
        }
    }
}

impl Default for Cover<Arc<str>, Arc<str>> {
    fn default() -> Self {
        Self::new(CoverType::F)
    }
}

#[cfg(test)]
mod tests;
