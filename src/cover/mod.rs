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
//! - A **labelled** `Cover<I, O>` (e.g. a `Cover<Symbol, Symbol>` built with
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
mod output_set;
pub mod pla;
mod symbols;

// Public re-exports - core types
pub use cubes::{Cube, CubeType};
pub use error::{AddExprError, ArityMismatch, CoverError, DuplicateLabel, ToExprError};
pub use iterators::{CubesIter, ToExprs};
pub use label::{Anonymous, Label, ReconcilableLabel, StringLabel};
pub use minimisation::Minimizable;
pub use minterm::{Minterm, MintermIter};
pub use output_set::OutputSet;
pub use symbols::Symbols;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Represents the type of cover (F, FD, FR, or FDR)
///
/// This type determines which sets are included in the cover:
/// - F: ON-set only
/// - FD: ON-set + Don't-care set
/// - FR: ON-set + OFF-set  
/// - FDR: ON-set + Don't-care set + OFF-set
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CoverType {
    /// On-set only (F)
    #[default]
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
    #[must_use]
    pub fn has_f(&self) -> bool {
        matches!(
            self,
            CoverType::F | CoverType::FD | CoverType::FR | CoverType::FDR
        )
    }

    /// Check if this type includes D (don't-care set)
    #[must_use]
    pub fn has_d(&self) -> bool {
        matches!(self, CoverType::FD | CoverType::FDR)
    }

    /// Check if this type includes R (OFF-set)
    #[must_use]
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
/// `Cover<I, O>` is generic over its **input** label type `I` and **output** label type `O`, with no
/// privileged label type — `Symbol`, `String`, `Arc<str>`, and `u32` are all on equal footing. The two
/// sides are independent: a cover can have, e.g., labelled inputs and an anonymous output
/// (`Cover<Symbol, Anonymous>`). The
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
/// This makes `Cover` suitable for concurrent applications.
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
/// use espresso_logic::{Cover, CoverType, Symbol};
///
/// let mut cover: Cover<Symbol, Symbol> = Cover::with_labels(
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
pub struct Cover<I, O> {
    /// Canonical input symbol table, shared by every cube's input minterm. One label per input
    /// position; whether those labels are *names* is the label type's business (`Symbol`/`String` are
    /// names, `Anonymous` is positional), so there is no separate "is it labelled" flag.
    input_symbols: Arc<Symbols<I>>,
    /// Canonical output symbol table, shared by every cube's output minterm.
    output_symbols: Arc<Symbols<O>>,
    /// Cubes (merged tri-state product terms).
    pub(crate) cubes: Vec<Cube<I, O>>,
    /// Cover type (F, FD, FR, or FDR)
    pub(crate) cover_type: CoverType,
}

/// Two covers are equal when they have the same cover type, the same input and output headers
/// (position-for-position, compared by label [`identity`](Label::identity)), and the same cubes in the
/// same order. Cube comparison is identity-based (see [`Cube`]'s `PartialEq`).
impl<I: Label, O: Label> PartialEq for Cover<I, O> {
    fn eq(&self, other: &Self) -> bool {
        self.cover_type == other.cover_type
            && self.input_symbols == other.input_symbols
            && self.output_symbols == other.output_symbols
            && self.cubes == other.cubes
    }
}

impl<I: Label, O: Label> Eq for Cover<I, O> {}

/// Hashes the same fields the [`PartialEq`] impl compares (cover type, both headers, and the cubes in
/// order), keeping the `Hash`/`Eq` contract so a `Cover` can key a `HashMap`/`HashSet`.
impl<I: Label, O: Label> std::hash::Hash for Cover<I, O> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.cover_type.hash(state);
        self.input_symbols.hash(state);
        self.output_symbols.hash(state);
        self.cubes.hash(state);
    }
}

impl<I, O> Cover<I, O>
where
    I: StringLabel,
    O: StringLabel,
{
    /// Create a new cover with pre-defined labels.
    ///
    /// Useful when you know the variable names in advance. The dimensions are set from the label
    /// counts. The label types are inferred from context (e.g. `Cover::<Symbol, Symbol>::with_labels`
    /// or `Cover::<String, String>::with_labels`) — any label type constructible from `&str` works.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cover, CoverType, Symbol};
    ///
    /// let cover: Cover<Symbol, Symbol> = Cover::with_labels(
    ///     CoverType::F,
    ///     &["a", "b", "c"],
    ///     &["out"],
    /// );
    /// assert_eq!(cover.num_inputs(), 3);
    /// assert_eq!(cover.num_outputs(), 1);
    /// ```
    #[must_use]
    pub fn with_labels<SI: AsRef<str>, SO: AsRef<str>>(
        cover_type: CoverType,
        input_labels: &[SI],
        output_labels: &[SO],
    ) -> Self {
        let input_vars: Arc<[I]> = input_labels.iter().map(|s| I::from(s.as_ref())).collect();
        let output_vars: Arc<[O]> = output_labels.iter().map(|s| O::from(s.as_ref())).collect();

        Cover {
            input_symbols: Symbols::new(input_vars),
            output_symbols: Symbols::new(output_vars),
            cubes: Vec::new(),
            cover_type,
        }
    }
}

impl<I: Label, O: Clone> Cover<I, O> {
    /// Expand every cube into its fully-assigned minterms over the explicit header `vars`.
    ///
    /// The inverse of minimisation ("maximise"): each cube's input pattern is re-expressed over `vars`
    /// and every don't-care (and every variable of `vars` absent from the cube) is split into both
    /// polarities, so each surviving cube assigns **every** variable in `vars`. `vars` MAY be a superset
    /// of the cover's inputs (absent variables widen into both polarities); input variables not in `vars`
    /// are dropped. Output columns and per-cube set tags are preserved; duplicate cubes are removed
    /// (first-seen order kept). The result shares one canonical input header, so its minterms stay on the
    /// fast-comparison path. Maximising an already-maximal cover over the same `vars` is a no-op.
    ///
    /// See [`Cube::expand_to`] / [`Minterm::expand_over`] for the per-cube primitive.
    #[must_use]
    pub fn maximize(&self, vars: &[I]) -> Cover<I, O> {
        let target = Symbols::new(vars.iter().cloned().collect());
        let mut seen: HashSet<Cube<I, O>> = HashSet::new();
        let mut cubes = Vec::new();
        for cube in &self.cubes {
            for inputs in cube.inputs.expand_over(&target) {
                let new = Cube::new(inputs, cube.outputs.clone(), cube.set);
                if seen.insert(new.clone()) {
                    cubes.push(new);
                }
            }
        }
        Cover {
            input_symbols: target,
            output_symbols: Arc::clone(&self.output_symbols),
            cubes,
            cover_type: self.cover_type,
        }
    }
}

impl<I, O> Cover<I, O> {
    /// Create a new empty cover of the given type, for **any** label types.
    ///
    /// The cover starts with no variables; dimensions grow as cubes/expressions are added. Works for
    /// every label type — `Cover::<Symbol, Symbol>::new(..)`, `Cover::<String, String>::new(..)`, or
    /// `Cover::<Anonymous, Anonymous>::new(..)` for a positional cover. The label types are inferred
    /// from later use where possible, else annotated.
    #[must_use]
    pub fn new(cover_type: CoverType) -> Self {
        Cover {
            input_symbols: Symbols::empty(),
            output_symbols: Symbols::empty(),
            cubes: Vec::new(),
            cover_type,
        }
    }

    /// Create a new empty **anonymous** cover (no variable labels) of the given type.
    ///
    /// Equivalent to `Cover::<Anonymous, Anonymous>::new(..)`; kept for readability at call sites that
    /// build positionally.
    #[must_use]
    pub fn anonymous(cover_type: CoverType) -> Self {
        Self::new(cover_type)
    }

    /// Re-express this cover over different label types, position-for-position.
    ///
    /// This is the **explicit** way to relabel or anonymise a cover — labelling and anonymisation
    /// never happen implicitly. The new symbol tables must have the same arities as this cover.
    /// To change only one side, use [`relabel_inputs`](Self::relabel_inputs) /
    /// [`relabel_outputs`](Self::relabel_outputs).
    ///
    /// # Errors
    ///
    /// Returns [`ArityMismatch`] if either replacement table's arity differs from this cover's
    /// corresponding arity (re-labelling is position-for-position).
    pub fn relabel<I2: Label, O2: Label>(
        self,
        input_symbols: Arc<Symbols<I2>>,
        output_symbols: Arc<Symbols<O2>>,
    ) -> Result<Cover<I2, O2>, ArityMismatch> {
        if input_symbols.arity() != self.num_inputs() {
            return Err(ArityMismatch::Inputs {
                expected: self.num_inputs(),
                actual: input_symbols.arity(),
            });
        }
        if output_symbols.arity() != self.num_outputs() {
            return Err(ArityMismatch::Outputs {
                expected: self.num_outputs(),
                actual: output_symbols.arity(),
            });
        }
        let cubes = self
            .cubes
            .into_iter()
            .map(|cube| {
                Cube::new(
                    Minterm::from_symbols(Arc::clone(&input_symbols), cube.inputs.iter()),
                    OutputSet::from_symbols(Arc::clone(&output_symbols), cube.outputs.iter()),
                    cube.cube_type(),
                )
            })
            .collect();
        Ok(Cover {
            input_symbols,
            output_symbols,
            cubes,
            cover_type: self.cover_type,
        })
    }

    /// Re-express only the **input** variables over a new label type, keeping the outputs as-is.
    ///
    /// # Errors
    ///
    /// Returns [`ArityMismatch`] if the new input table's arity differs from this cover's input arity.
    pub fn relabel_inputs<I2: Label>(
        self,
        input_symbols: Arc<Symbols<I2>>,
    ) -> Result<Cover<I2, O>, ArityMismatch> {
        if input_symbols.arity() != self.num_inputs() {
            return Err(ArityMismatch::Inputs {
                expected: self.num_inputs(),
                actual: input_symbols.arity(),
            });
        }
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
        Ok(Cover {
            input_symbols,
            output_symbols: self.output_symbols,
            cubes,
            cover_type: self.cover_type,
        })
    }

    /// Re-express only the **output** variables over a new label type, keeping the inputs as-is.
    ///
    /// # Errors
    ///
    /// Returns [`ArityMismatch`] if the new output table's arity differs from this cover's output arity.
    pub fn relabel_outputs<O2: Label>(
        self,
        output_symbols: Arc<Symbols<O2>>,
    ) -> Result<Cover<I, O2>, ArityMismatch> {
        if output_symbols.arity() != self.num_outputs() {
            return Err(ArityMismatch::Outputs {
                expected: self.num_outputs(),
                actual: output_symbols.arity(),
            });
        }
        let cubes = self
            .cubes
            .into_iter()
            .map(|cube| {
                Cube::new(
                    cube.inputs,
                    OutputSet::from_symbols(Arc::clone(&output_symbols), cube.outputs.iter()),
                    cube.set,
                )
            })
            .collect();
        Ok(Cover {
            input_symbols: self.input_symbols,
            output_symbols,
            cubes,
            cover_type: self.cover_type,
        })
    }

    /// Drop all labels, yielding a positional [`Cover<Anonymous, Anonymous>`](Cover) (explicit anonymisation).
    ///
    /// Infallible: the anonymous tables are built at this cover's own arities, so they always match.
    #[must_use = "anonymize returns a new cover; the original is consumed"]
    pub fn anonymize(self) -> Cover<Anonymous, Anonymous> {
        let (ni, no) = (self.num_inputs(), self.num_outputs());
        self.relabel(
            Symbols::<Anonymous>::anonymous(ni),
            Symbols::<Anonymous>::anonymous(no),
        )
        .expect("anonymous tables are built at the cover's own arities")
    }

    /// Get the number of inputs
    #[must_use]
    pub fn num_inputs(&self) -> usize {
        self.input_symbols.arity()
    }

    /// Get the number of outputs
    #[must_use]
    pub fn num_outputs(&self) -> usize {
        self.output_symbols.arity()
    }

    /// Get the number of cubes (for F/FD types, only counts F cubes; for FR/FDR, counts all)
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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

/// `for cube in &cover` iterates the cover's cubes by reference, same as [`Cover::cubes`] (so it
/// honours the cover-type filter).
impl<'a, I, O> IntoIterator for &'a Cover<I, O> {
    type Item = &'a Cube<I, O>;
    type IntoIter = CubesIter<'a, &'a Cube<I, O>>;

    fn into_iter(self) -> Self::IntoIter {
        self.cubes()
    }
}

impl<I: Label, O: Label> Cover<I, O> {
    /// Build a cover from a collection of typed [`Cube`]s, aligning them by variable
    /// [identity](Label) — by name for labelled cubes, by position for anonymous ones.
    ///
    /// The cover's headers are the identity **union** of the cubes' headers: a variable carried by some
    /// cubes but not others becomes a don't-care (inputs) / unasserted (outputs) in the cubes that lack
    /// it. For anonymous cubes identity is position, so this is the widest input/output arity with
    /// shorter cubes padded — the original positional behaviour. Each cube keeps its own [`CubeType`]
    /// (F/D/R); build anonymous cubes with [`Cube::anonymous`], labelled ones with
    /// [`Cube::labeled`]/[`Cube::with_labels`].
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
    #[must_use]
    pub fn from_cubes(
        cover_type: CoverType,
        cubes: impl IntoIterator<Item = Cube<I, O>>,
    ) -> Cover<I, O> {
        let cubes: Vec<Cube<I, O>> = cubes.into_iter().collect();
        // Headers are the identity union of every cube's headers (position union for anonymous).
        let mut input_symbols: Arc<Symbols<I>> = Symbols::empty();
        let mut output_symbols: Arc<Symbols<O>> = Symbols::empty();
        for c in &cubes {
            input_symbols = union_inputs(&input_symbols, c.inputs().symbols());
            output_symbols = overlay_outputs(&output_symbols, c.outputs().symbols()).0;
        }
        let new_no = output_symbols.arity();
        let cubes = cubes
            .iter()
            .map(|c| {
                // `output_symbols` already holds every identity of `c` (built in the fold above), so
                // each output column is found directly by identity — no per-cube union rebuild.
                Cube::new(
                    c.inputs().project_onto(&input_symbols),
                    assert_mask(c, &output_symbols, new_no, c.outputs().num_vars(), |old| {
                        identity_position(&output_symbols, c.outputs().symbols(), old)
                    }),
                    c.set,
                )
            })
            .collect();
        Cover {
            input_symbols,
            output_symbols,
            cubes,
            cover_type,
        }
    }

    /// Append a single typed [`Cube`] to this cover, aligning it by variable [identity](Label).
    ///
    /// The cube's variables line up with the cover's existing columns by identity — by name for
    /// labelled covers, by position for anonymous ones. A variable the cube carries but the cover lacks
    /// **widens the cover by that identity**: every existing cube gains a don't-care (inputs) /
    /// unasserted (outputs) column, exactly as [`merge`](Self::merge) does. For an anonymous cover that
    /// means a wider cube extends the dimensions position-wise (the original behaviour).
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
    pub fn push(&mut self, cube: Cube<I, O>) {
        // Probe (no allocation) whether the cube carries an identity the cover lacks. Only then must the
        // existing cubes be re-homed onto wider headers; the common append allocates no `Symbols` and
        // touches just the new cube.
        let grows_inputs = cube
            .inputs()
            .symbols()
            .labels()
            .iter()
            .enumerate()
            .any(|(i, l)| {
                self.input_symbols
                    .position_of_identity(&l.identity(i))
                    .is_none()
            });
        let grows_outputs = cube
            .outputs()
            .symbols()
            .labels()
            .iter()
            .enumerate()
            .any(|(j, l)| {
                self.output_symbols
                    .position_of_identity(&l.identity(j))
                    .is_none()
            });

        if grows_inputs || grows_outputs {
            let new_input = union_inputs(&self.input_symbols, cube.inputs().symbols());
            let (new_output, self_out_map, _) =
                overlay_outputs(&self.output_symbols, cube.outputs().symbols());
            let new_no = new_output.arity();
            let old_no = self.num_outputs();
            for c in &mut self.cubes {
                let inputs = c.inputs().project_onto(&new_input);
                let outputs = assert_mask(c, &new_output, new_no, old_no, |old| self_out_map[old]);
                c.inputs = inputs;
                c.outputs = outputs;
            }
            self.input_symbols = new_input;
            self.output_symbols = new_output;
        }

        // Re-home the new cube onto the cover's current headers (now a superset of its identities).
        let inputs = cube.inputs().project_onto(&self.input_symbols);
        let outputs = assert_mask(
            &cube,
            &self.output_symbols,
            self.num_outputs(),
            cube.outputs().num_vars(),
            |old| identity_position(&self.output_symbols, cube.outputs().symbols(), old),
        );
        self.cubes.push(Cube::new(inputs, outputs, cube.set));
    }
}

// ===== Cover combination (`extend` / `merge`) =====
//
// Inputs always union by variable *identity* ([`Label::identity`]) for both operations: shared inputs
// line up (by name when labelled, by position when anonymous) and never get renamed. The two operations
// differ only in how the OUTPUT columns combine, and each is consistent across every label type:
//   - `merge` overlays outputs by identity ([`overlay_outputs`]) — `b`'s output of an identity already
//     in `a` lands on it; new identities extend the header.
//   - `extend` always appends `b`'s outputs as distinct columns ([`append_outputs`]), reconciling name
//     clashes via [`ReconcilableLabel`] (string `f`→`f0`, `Anonymous` fresh position).
// There is no runtime labelled-vs-anonymous branch: the per-label-type behaviour lives entirely in the
// `Label`/`ReconcilableLabel` impls.

/// Build a cube's output-membership minterm over `new_output` (width `new_no`): for each old output
/// position `0..old_count` the cube asserts, set the new position `map(old)`; everything else is
/// unasserted (`Some(false)`). Shared by the per-cube re-home in `push`/`from_cubes` and the
/// `merge`/`extend` rebuild, each supplying its own per-output `map`.
fn assert_mask<I, O, M>(
    cube: &Cube<I, O>,
    new_output: &Arc<Symbols<M>>,
    new_no: usize,
    old_count: usize,
    map: impl Fn(usize) -> usize,
) -> OutputSet<M> {
    let mut mask = vec![false; new_no];
    for old in 0..old_count {
        if cube.asserts(old) {
            mask[map(old)] = true;
        }
    }
    OutputSet::from_symbols(Arc::clone(new_output), mask)
}

/// Position in `target` of the variable that `source`'s column `pos` carries, found by identity.
/// `target` must contain that identity (it is always a superset by construction at the call sites).
fn identity_position<L: Label>(target: &Symbols<L>, source: &Symbols<L>, pos: usize) -> usize {
    let id = source.labels()[pos].identity(pos);
    target
        .position_of_identity(&id)
        .expect("target header contains the source column's identity") as usize
}

/// Union two headers by variable identity: `a`'s labels, then each of `b`'s labels whose identity is
/// new. Returns the combined header plus each side's old→new position map — `a` maps to itself
/// (`0..a_no`), and `b`'s label reuses the position of a matching identity (in `a` or an earlier `b`
/// column) or extends the header. Alignment is by name when labelled, by position when anonymous
/// (`Anonymous`'s identity is its index).
///
/// O(n + m): membership is probed through a `HashMap` keyed on [`Identity`](Label::Identity) (which is
/// `Hash`), not the former per-label linear scan of the growing header.
fn identity_union<L: Label>(
    a: &Symbols<L>,
    b: &Symbols<L>,
) -> (Arc<Symbols<L>>, Vec<usize>, Vec<usize>) {
    let a_no = a.arity();
    let mut header: Vec<L> = a.labels().to_vec();
    let mut pos_by_id: HashMap<L::Identity, usize> = a
        .labels()
        .iter()
        .enumerate()
        .map(|(k, la)| (la.identity(k), k))
        .collect();
    let b_map = b
        .labels()
        .iter()
        .enumerate()
        .map(|(j, lb)| {
            *pos_by_id.entry(lb.identity(j)).or_insert_with(|| {
                header.push(lb.clone());
                header.len() - 1
            })
        })
        .collect();
    (Symbols::new(header.into()), (0..a_no).collect(), b_map)
}

/// The union **input** header of two covers, aligned by identity (the header from [`identity_union`];
/// the position maps are unused for inputs, which re-point via [`project_onto`](Minterm::project_onto)).
fn union_inputs<I: Label>(a: &Symbols<I>, b: &Symbols<I>) -> Arc<Symbols<I>> {
    identity_union(a, b).0
}

/// Output header for `merge`: overlay `b`'s outputs onto `a`'s by identity (an identity already in `a`
/// reuses that column; new identities extend the header). Exactly [`identity_union`].
fn overlay_outputs<O: Label>(
    a: &Symbols<O>,
    b: &Symbols<O>,
) -> (Arc<Symbols<O>>, Vec<usize>, Vec<usize>) {
    identity_union(a, b)
}

/// Output header for `extend`: append **all** of `b`'s outputs after `a`'s as distinct columns,
/// reconciling clashes via [`ReconcilableLabel`]. `b`'s output `j` maps to `a_no + j` (contiguous).
fn append_outputs<O: ReconcilableLabel>(
    a: &Symbols<O>,
    b: &Symbols<O>,
) -> (Arc<Symbols<O>>, Vec<usize>, Vec<usize>) {
    let a_no = a.arity();
    let b_no = b.arity();
    let mut header: Vec<O> = a.labels().to_vec();
    header.extend(O::reconcile(a.labels(), b.labels()));
    (
        Symbols::new(header.into()),
        (0..a_no).collect(),
        (a_no..a_no + b_no).collect(),
    )
}

/// Re-point both covers' cubes onto the given combined headers. Inputs union by identity (via
/// [`project_onto`](Minterm::project_onto)); outputs follow the supplied per-side maps. Each cube keeps
/// its [`CubeType`]. Shared by `extend` and `merge` — only the output header/maps differ.
fn assemble<I: Label, O: Label>(
    a: &Cover<I, O>,
    b: &Cover<I, O>,
    new_output: Arc<Symbols<O>>,
    a_out_map: Vec<usize>,
    b_out_map: Vec<usize>,
) -> Cover<I, O> {
    let new_input = union_inputs(&a.input_symbols, &b.input_symbols);
    let new_no = new_output.arity();
    let rebuild = |c: &Cube<I, O>, out_map: &[usize]| {
        Cube::new(
            c.inputs().project_onto(&new_input),
            assert_mask(c, &new_output, new_no, out_map.len(), |old| out_map[old]),
            c.set,
        )
    };
    let cubes = a
        .cubes
        .iter()
        .map(|c| rebuild(c, &a_out_map))
        .chain(b.cubes.iter().map(|c| rebuild(c, &b_out_map)))
        .collect();
    Cover {
        input_symbols: new_input,
        output_symbols: new_output,
        cubes,
        cover_type: a.cover_type,
    }
}

impl<I: Label, O: Label> Cover<I, O> {
    /// Combine `other` into this cover, **overlaying** outputs by identity.
    ///
    /// Inputs union by variable identity (by name when labelled, by position when anonymous — missing
    /// inputs pad don't-care). Outputs overlay by identity: `other`'s output of an identity already in
    /// `self` lands on the same column; new identities extend the header. For an anonymous output that
    /// means output `i` of `other` overlays output `i` of `self`; the result has
    /// `max(self.num_outputs(), other.num_outputs())` outputs. Consistent across every label type.
    pub fn merge(&mut self, other: &Cover<I, O>) {
        // Probe (no allocation) whether `other` carries an identity `self` lacks. Only then must `self`'s
        // cubes be re-homed onto wider headers; otherwise `self` keeps its headers and cubes, and
        // `other`'s cubes are appended in place — no union `Symbols`, no rebuild of `self`.
        let grows_inputs = other
            .input_symbols
            .labels()
            .iter()
            .enumerate()
            .any(|(i, l)| {
                self.input_symbols
                    .position_of_identity(&l.identity(i))
                    .is_none()
            });
        let grows_outputs = other
            .output_symbols
            .labels()
            .iter()
            .enumerate()
            .any(|(j, l)| {
                self.output_symbols
                    .position_of_identity(&l.identity(j))
                    .is_none()
            });

        if grows_inputs || grows_outputs {
            let (new_output, a_map, b_map) =
                overlay_outputs(&self.output_symbols, &other.output_symbols);
            *self = assemble(self, other, new_output, a_map, b_map);
            return;
        }

        let no = self.num_outputs();
        self.cubes.reserve(other.cubes.len());
        for c in &other.cubes {
            let inputs = c.inputs().project_onto(&self.input_symbols);
            let outputs = assert_mask(c, &self.output_symbols, no, c.outputs().num_vars(), |old| {
                identity_position(&self.output_symbols, c.outputs().symbols(), old)
            });
            self.cubes.push(Cube::new(inputs, outputs, c.set));
        }
    }
}

impl<I: Label, O: ReconcilableLabel> Cover<I, O> {
    /// Combine `other` into this cover by **appending** its outputs after this cover's, as distinct
    /// columns.
    ///
    /// Inputs union by identity exactly as for [`merge`](Self::merge). Every one of `other`'s outputs
    /// is appended (the result always has `self.num_outputs() + other.num_outputs()` outputs), so use
    /// this to stack two functions into one multi-output cover. A clashing output **name** is reconciled
    /// by [`ReconcilableLabel`] (string `f` → `f0`); an anonymous output appends a fresh position.
    /// Consistent across every label type — unlike `merge`, never overlays.
    pub fn extend(&mut self, other: &Cover<I, O>) {
        let (new_output, a_map, b_map) =
            append_outputs(&self.output_symbols, &other.output_symbols);
        *self = assemble(self, other, new_output, a_map, b_map);
    }
}

impl<I: AsRef<str>, O> Cover<I, O> {
    /// Get input variable labels.
    ///
    /// Returns the input labels (one per input position). Available for any string-like input label
    /// type whatever the output label type is; a positional `Cover<Anonymous, _>` has no such method.
    #[must_use]
    pub fn input_labels(&self) -> &[I] {
        self.input_symbols.labels()
    }
}

impl<I, O: AsRef<str>> Cover<I, O> {
    /// Get output variable labels.
    ///
    /// Returns the output labels (one per output position). Available for any string-like output label
    /// type whatever the input label type is; a positional `Cover<_, Anonymous>` has no such method.
    #[must_use]
    pub fn output_labels(&self) -> &[O] {
        self.output_symbols.labels()
    }
}

/// An empty `F`-type cover, for any label types. (An empty cover carries no labels, so this is generic
/// — `Symbol` is not privileged.)
impl<I, O> Default for Cover<I, O> {
    fn default() -> Self {
        Self::new(CoverType::F)
    }
}

#[cfg(test)]
mod tests;
