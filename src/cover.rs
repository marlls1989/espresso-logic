//! Cover types and traits for Boolean function minimization
//!
//! This module provides the unified Cover type for working with covers (sum-of-products representations
//! of Boolean functions). The Cover type supports dynamic dimensions that grow as cubes are added,
//! and can work with both manually constructed cubes and boolean expressions.

pub mod dnf;
pub use dnf::Dnf;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::Arc;

use crate::error::{AddExprError, CoverError, MinimizationError, ToExprError};
use crate::expression::BoolExpr;
use crate::EspressoConfig;

/// Generic label manager for input/output variables with configurable prefix
///
/// Maintains both ordered labels (Vec) and fast name->index lookup (HashMap).
/// Handles conflict resolution by finding next available sequential label.
#[derive(Clone, Debug)]
struct LabelManager<const PREFIX: char> {
    /// Ordered labels by position
    labels: Vec<Arc<str>>,
    /// Fast lookup: label name -> position index
    label_map: HashMap<Arc<str>, usize>,
}

impl<const PREFIX: char> LabelManager<PREFIX> {
    /// Create a new empty label manager
    fn new() -> Self {
        Self {
            labels: Vec::new(),
            label_map: HashMap::new(),
        }
    }

    /// Create from existing labels
    fn from_labels(labels: Vec<Arc<str>>) -> Self {
        let label_map = labels
            .iter()
            .enumerate()
            .map(|(i, label)| (Arc::clone(label), i))
            .collect();
        Self { labels, label_map }
    }

    /// Get the number of labels
    #[allow(dead_code)]
    fn len(&self) -> usize {
        self.labels.len()
    }

    /// Check if empty
    fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }

    /// Get label at position
    fn get(&self, index: usize) -> Option<&Arc<str>> {
        self.labels.get(index)
    }

    /// Get labels slice
    fn as_slice(&self) -> &[Arc<str>] {
        &self.labels
    }

    /// Find position by label name (O(1) lookup)
    fn find_position(&self, name: &str) -> Option<usize> {
        let key: Arc<str> = Arc::from(name);
        self.label_map.get(&key).copied()
    }

    /// Check if label exists
    fn contains(&self, name: &str) -> bool {
        let key: Arc<str> = Arc::from(name);
        self.label_map.contains_key(&key)
    }

    /// Find the next available sequential label index starting from `start`
    /// E.g., if x0, x1, x3 exist and start=2, returns 2 (first available from start)
    fn next_available_index(&self, start: usize) -> usize {
        let mut n = start;
        loop {
            let candidate = Arc::from(format!("{}{}", PREFIX, n).as_str());
            if !self.label_map.contains_key(&candidate) {
                return n;
            }
            n += 1;
        }
    }

    /// Add a label at the given position, checking for conflicts
    /// If conflict, finds next available sequential label starting from position
    fn add_with_conflict_resolution(&mut self, position: usize) {
        // Try natural label first (e.g., x2 for position 2)
        let natural_label = Arc::from(format!("{}{}", PREFIX, position).as_str());
        let label = if !self.label_map.contains_key(&natural_label) {
            natural_label
        } else {
            // Conflict - find next available sequential label starting from position
            let n = self.next_available_index(position);
            Arc::from(format!("{}{}", PREFIX, n).as_str())
        };
        self.label_map.insert(Arc::clone(&label), position);
        self.labels.push(label);
    }

    /// Add a specific label at the given position
    fn add(&mut self, label: Arc<str>, position: usize) {
        self.label_map.insert(Arc::clone(&label), position);
        self.labels.push(label);
    }

    /// Backfill missing labels up to target size
    fn backfill_to(&mut self, target_size: usize) {
        while self.labels.len() < target_size {
            let position = self.labels.len();
            self.add_with_conflict_resolution(position);
        }
    }
}

/// Type alias for cube data as owned vectors (inputs, outputs)
pub type CubeData = (Vec<Option<bool>>, Vec<Option<bool>>);

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

/// Iterator over filtered cubes with generic yield type
///
/// This iterator wraps a filtered cube iterator and can yield different types
/// depending on how the cubes are transformed (references, owned data, etc.).
pub struct CubesIter<'a, T> {
    iter: Box<dyn Iterator<Item = T> + 'a>,
}

impl<'a, T> Iterator for CubesIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Iterator over output expressions from a Cover
///
/// This iterator uses the visitor pattern to generate boolean expressions
/// on-demand for each output in the cover. It maintains state (current index)
/// and calls the cover's conversion method during iteration.
pub struct ToExprs<'a> {
    cover: &'a Cover,
    current_idx: usize,
}

impl<'a> Iterator for ToExprs<'a> {
    type Item = (Arc<str>, BoolExpr);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_idx >= self.cover.num_outputs {
            return None;
        }
        let idx = self.current_idx;
        self.current_idx += 1;

        // Use provided label or generate default
        let name = if let Some(label) = self.cover.output_labels.get(idx) {
            Arc::clone(label)
        } else {
            Arc::from(format!("y{}", idx).as_str())
        };

        let expr = self
            .cover
            .to_expr_by_index(idx)
            .unwrap_or_else(|_| BoolExpr::constant(false));
        Some((name, expr))
    }
}

/// Public trait for types that can be minimized using Espresso
///
/// This trait provides a **transparent, uniform interface** for minimizing boolean functions
/// using the Espresso algorithm. All methods take `&self` and return a new minimized instance,
/// following an immutable functional style.
///
/// # Transparent Minimization
///
/// The beauty of this trait is that minimization works the same way regardless of input type.
/// Just call `.minimize()` on any supported type and get back a minimized version of the same type:
///
/// ```
/// use espresso_logic::{BoolExpr, Cover, CoverType, Minimizable};
///
/// # fn main() -> std::io::Result<()> {
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let c = BoolExpr::variable("c");
/// let redundant = a.and(&b).or(&a.and(&b).and(&c));
///
/// // Works on BoolExpr - returns BoolExpr
/// let min_expr = redundant.minimize()?;
/// println!("Minimized expression: {}", min_expr);
///
/// // Works on Cover - returns Cover
/// let mut cover = Cover::new(CoverType::F);
/// cover.add_expr(&redundant, "out")?;
/// let min_cover = cover.minimize()?;
/// println!("Minimized cover has {} cubes", min_cover.num_cubes());
///
/// // Both produce equivalent minimized results!
/// # Ok(())
/// # }
/// ```
///
/// # Implementations
///
/// - **[`Cover`]**: Direct implementation - minimizes cubes directly with Espresso
/// - **Blanket implementation** (v3.1+): For `T where &T: Into<Dnf>, T: From<Dnf>` (defined in `dnf` module)
///   - Automatically covers [`BoolExpr`] and [`expression::bdd::Bdd`]
///   - Workflow: Expression → Dnf (via BDD for canonical form) → Cover cubes → Espresso → minimized Cover → Dnf → Expression
///   - DNF serves as the intermediary representation, with BDD ensuring efficient conversion
///
/// [`expression::bdd::Bdd`]: crate::expression::bdd::Bdd
///
/// [`BoolExpr`]: crate::expression::BoolExpr
/// [`Cover`]: crate::Cover
///
/// # Immutable Design
///
/// All minimization methods preserve the original and return a new minimized instance:
///
/// ```
/// use espresso_logic::{BoolExpr, Minimizable};
///
/// # fn main() -> std::io::Result<()> {
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let c = BoolExpr::variable("c");
///
/// let original = a.and(&b).or(&a.and(&b).and(&c));
/// let minimized = original.minimize()?;
///
/// // Original is unchanged
/// println!("Original: {}", original);
/// println!("Minimized: {}", minimized);
///
/// // Can continue using original
/// let bdd = original.to_bdd();
/// # Ok(())
/// # }
/// ```
pub trait Minimizable {
    /// Minimize using the heuristic Espresso algorithm
    ///
    /// Returns a new minimized instance without modifying the original.
    /// This is fast and produces near-optimal results (~99% optimal in practice).
    ///
    /// Default implementation calls `minimize_with_config` with default config.
    fn minimize(&self) -> Result<Self, MinimizationError>
    where
        Self: Sized,
    {
        let config = EspressoConfig::default();
        self.minimize_with_config(&config)
    }

    /// Minimize using the heuristic algorithm with custom configuration
    ///
    /// Returns a new minimized instance without modifying the original.
    ///
    /// This is the primary method that implementations must provide.
    fn minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError>
    where
        Self: Sized;

    /// Minimize using exact minimization
    ///
    /// Returns a new minimized instance without modifying the original.
    /// This guarantees minimal results but may be slower for large expressions.
    ///
    /// Default implementation calls `minimize_exact_with_config` with default config.
    fn minimize_exact(&self) -> Result<Self, MinimizationError>
    where
        Self: Sized,
    {
        let config = EspressoConfig::default();
        self.minimize_exact_with_config(&config)
    }

    /// Minimize using exact minimization with custom configuration
    ///
    /// Returns a new minimized instance without modifying the original.
    ///
    /// This is the primary method that implementations must provide.
    fn minimize_exact_with_config(
        &self,
        config: &EspressoConfig,
    ) -> Result<Self, MinimizationError>
    where
        Self: Sized;
}

/// A unified cover type with dynamic dimensions
///
/// The `Cover` type represents a Boolean function as a sum-of-products (cover).
/// It supports dynamic sizing - dimensions grow automatically as cubes are added.
/// It can work with manually constructed cubes or boolean expressions.
///
/// # Examples
///
/// ```
/// use espresso_logic::{Cover, CoverType, Minimizable};
///
/// // Create an empty cover
/// let mut cover = Cover::new(CoverType::F);
///
/// // Add cubes (dimensions grow automatically)
/// cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
/// cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
///
/// // Minimize it (returns new instance)
/// cover = cover.minimize().unwrap();
///
/// println!("Minimized to {} cubes", cover.num_cubes());
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
                .filter(|cube| cube.cube_type == CubeType::F)
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
                    cover_type != CoverType::F || cube.cube_type == CubeType::F
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
                    .filter(move |cube| cover_type != CoverType::F || cube.cube_type == CubeType::F)
                    .map(|cube| {
                        let inputs = cube.inputs.to_vec();
                        let outputs: Vec<Option<bool>> =
                            cube.outputs.iter().map(|&b| Some(b)).collect();
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

    /// Add a boolean function to a named output
    ///
    /// This generic method accepts any type that can be converted to DNF:
    /// - `&BoolExpr` - Boolean expressions
    /// - `&Bdd` - Binary Decision Diagrams
    /// - `&Dnf` - Direct DNF representation
    ///
    /// Input variables are matched by name with existing variables,
    /// and new variables are appended in alphabetical order.
    ///
    /// Returns an error if the output name already exists (to prevent accidental overwrite).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cover, BoolExpr, CoverType};
    ///
    /// let mut cover = Cover::new(CoverType::F);
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b);
    ///
    /// // Works with BoolExpr
    /// cover.add_expr(&expr, "output1").unwrap();
    /// assert_eq!(cover.num_inputs(), 2);
    /// assert_eq!(cover.num_outputs(), 1);
    ///
    /// // Also works with Bdd
    /// let bdd = b.or(&a).to_bdd();
    /// cover.add_expr(&bdd, "output2").unwrap();
    /// ```
    pub fn add_expr<T>(&mut self, expr: &T, output_name: &str) -> Result<(), AddExprError>
    where
        for<'a> &'a T: Into<Dnf>,
    {
        // Convert to DNF (goes through BDD for canonical form and optimizations)
        let dnf: Dnf = expr.into();

        // Backfill output labels if needed to check for conflicts
        let output_was_unlabeled = self.output_labels.is_empty();
        if output_was_unlabeled && self.num_outputs > 0 {
            self.output_labels.backfill_to(self.num_outputs);
        }

        // Check if output already exists (fail fast before doing any work)
        if self.output_labels.contains(output_name) {
            return Err(CoverError::OutputAlreadyExists {
                name: output_name.to_string(),
            }
            .into());
        }

        // Backfill input labels if we're transitioning from unlabeled to labeled inputs
        let input_was_unlabeled = self.input_labels.is_empty();
        if input_was_unlabeled && self.num_inputs > 0 {
            self.input_labels.backfill_to(self.num_inputs);
        }

        // Extract cubes from DNF
        let cubes = dnf.cubes();

        // Collect all variables from cubes (in sorted order for consistency)
        let mut dnf_variables = BTreeSet::new();
        for product_term in cubes {
            for var in product_term.keys() {
                dnf_variables.insert(Arc::clone(var));
            }
        }

        // Build variable mapping: dnf variable -> cover input index
        let mut var_to_index: BTreeMap<Arc<str>, usize> = BTreeMap::new();
        let mut num_new_variables = 0;

        for dnf_var in &dnf_variables {
            // Check if variable already exists in cover (using HashMap for O(1) lookup)
            if let Some(pos) = self.input_labels.find_position(dnf_var.as_ref()) {
                var_to_index.insert(Arc::clone(dnf_var), pos);
            } else {
                // New variable - add to cover
                let new_index = self.num_inputs + num_new_variables;
                let label = Arc::clone(dnf_var);
                self.input_labels.add(label, new_index);
                var_to_index.insert(Arc::clone(dnf_var), new_index);
                num_new_variables += 1;
            }
        }

        // Pad all existing cubes once with all new variables
        if num_new_variables > 0 {
            for cube in &mut self.cubes {
                let mut new_inputs = cube.inputs.to_vec();
                new_inputs.resize(new_inputs.len() + num_new_variables, None);
                cube.inputs = new_inputs.into();
            }
            self.num_inputs += num_new_variables;
        }

        // Add new output
        let output_index = self.num_outputs;
        self.output_labels.add(Arc::from(output_name), output_index);
        self.num_outputs += 1;

        // Extend all existing cubes with false for new output
        for cube in &mut self.cubes {
            let mut new_outputs = cube.outputs.to_vec();
            new_outputs.push(false);
            cube.outputs = new_outputs.into();
        }

        // Convert cubes to cover format and add to cover
        for product_term in cubes {
            // Build input vector based on variable mapping
            let mut inputs = vec![None; self.num_inputs];
            for (var, &polarity) in product_term {
                if let Some(&idx) = var_to_index.get(var) {
                    inputs[idx] = Some(polarity);
                }
            }

            // Build output vector with only this output set
            let mut outputs = vec![false; self.num_outputs];
            outputs[output_index] = true;

            self.cubes.push(Cube::new(&inputs, &outputs, CubeType::F));
        }

        Ok(())
    }

    /// Convert all outputs to boolean expressions
    ///
    /// Returns an iterator over (output_name, expression) tuples, one for each output.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cover, BoolExpr, CoverType};
    ///
    /// let mut cover = Cover::new(CoverType::F);
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    ///
    /// cover.add_expr(&a, "out1").unwrap();
    /// cover.add_expr(&b, "out2").unwrap();
    ///
    /// for (name, expr) in cover.to_exprs() {
    ///     println!("{}: {}", name, expr);
    /// }
    /// ```
    pub fn to_exprs(&self) -> ToExprs<'_> {
        ToExprs {
            cover: self,
            current_idx: 0,
        }
    }

    /// Convert a specific named output to a boolean expression
    ///
    /// Returns an error if the output name doesn't exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cover, BoolExpr, CoverType};
    ///
    /// let mut cover = Cover::new(CoverType::F);
    /// let a = BoolExpr::variable("a");
    ///
    /// cover.add_expr(&a, "result").unwrap();
    /// let expr = cover.to_expr("result").unwrap();
    /// println!("result: {}", expr);
    /// ```
    pub fn to_expr(&self, output_name: &str) -> Result<BoolExpr, ToExprError> {
        // Use HashMap for O(1) lookup
        let output_idx = self
            .output_labels
            .find_position(output_name)
            .ok_or_else(|| CoverError::OutputNotFound {
                name: output_name.to_string(),
            })?;

        self.to_expr_by_index(output_idx)
    }

    /// Convert a specific output index to a boolean expression
    ///
    /// Returns an error if the index is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cover, BoolExpr, CoverType};
    ///
    /// let mut cover = Cover::new(CoverType::F);
    /// let a = BoolExpr::variable("a");
    ///
    /// cover.add_expr(&a, "out").unwrap();
    /// let expr = cover.to_expr_by_index(0).unwrap();
    /// println!("Output 0: {}", expr);
    /// ```
    pub fn to_expr_by_index(&self, output_idx: usize) -> Result<BoolExpr, ToExprError> {
        if output_idx >= self.num_outputs {
            return Err(CoverError::OutputIndexOutOfBounds {
                index: output_idx,
                max: if self.num_outputs > 0 {
                    self.num_outputs - 1
                } else {
                    0
                },
            }
            .into());
        }

        // Filter cubes for this output (check if output bit is set)
        let relevant_cubes: Vec<&Cube> = self
            .cubes
            .iter()
            .filter(|cube| {
                // Only F cubes contribute to the expression
                cube.cube_type == CubeType::F
                    && output_idx < cube.outputs.len()
                    && cube.outputs[output_idx]
            })
            .collect();

        Ok(cubes_to_expr(
            &relevant_cubes,
            self.input_labels.as_slice(),
            self.num_inputs,
        ))
    }
}

/// Convert cube references back to a boolean expression
///
/// If `variables` is empty or shorter than `num_inputs`, generates default variable names (x0, x1, ...).
fn cubes_to_expr(cubes: &[&Cube], variables: &[Arc<str>], num_inputs: usize) -> BoolExpr {
    if cubes.is_empty() {
        return BoolExpr::constant(false);
    }

    let mut terms = Vec::new();

    for cube in cubes.iter() {
        // Build product term for this cube
        let mut factors = Vec::new();

        for i in 0..num_inputs {
            // Get variable name - use provided label or generate default
            let var_name: Arc<str> = if i < variables.len() {
                Arc::clone(&variables[i])
            } else {
                Arc::from(format!("x{}", i).as_str())
            };

            match cube.inputs.get(i) {
                Some(Some(true)) => {
                    // Positive literal
                    factors.push(BoolExpr::variable(&var_name));
                }
                Some(Some(false)) => {
                    // Negative literal
                    factors.push(BoolExpr::variable(&var_name).not());
                }
                Some(None) | None => {
                    // Don't care - skip this variable
                }
            }
        }

        // AND all factors together
        if factors.is_empty() {
            // No literals means tautology (true)
            terms.push(BoolExpr::constant(true));
        } else {
            let product = factors.into_iter().reduce(|acc, f| acc.and(&f)).unwrap();
            terms.push(product);
        }
    }

    // OR all terms together
    if terms.is_empty() {
        BoolExpr::constant(false)
    } else {
        terms.into_iter().reduce(|acc, t| acc.or(&t)).unwrap()
    }
}

// Implement public Minimizable trait for Cover
impl Minimizable for Cover {
    fn minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError> {
        use crate::espresso::{Espresso, EspressoCover};

        // Split cubes into F, D, R sets based on cube type
        let mut f_cubes = Vec::new();
        let mut d_cubes = Vec::new();
        let mut r_cubes = Vec::new();

        for cube in self.cubes.iter() {
            let input_vec: Vec<u8> = cube
                .inputs
                .iter()
                .map(|&opt| match opt {
                    Some(false) => 0,
                    Some(true) => 1,
                    None => 2,
                })
                .collect();

            // Convert outputs: true → 1, false → 0
            let output_vec: Vec<u8> = cube
                .outputs
                .iter()
                .map(|&b| if b { 1 } else { 0 })
                .collect();

            // Send to appropriate set based on cube type
            match cube.cube_type {
                CubeType::F => f_cubes.push((input_vec, output_vec)),
                CubeType::D => d_cubes.push((input_vec, output_vec)),
                CubeType::R => r_cubes.push((input_vec, output_vec)),
            }
        }

        // Direct C calls - thread-safe via thread-local storage
        let esp = Espresso::new(self.num_inputs(), self.num_outputs(), config);

        // Build covers from cube data
        let f_cover = EspressoCover::from_cubes(f_cubes, self.num_inputs(), self.num_outputs())?;
        let d_cover = if !d_cubes.is_empty() {
            Some(EspressoCover::from_cubes(
                d_cubes,
                self.num_inputs(),
                self.num_outputs(),
            )?)
        } else {
            None
        };
        let r_cover = if !r_cubes.is_empty() {
            Some(EspressoCover::from_cubes(
                r_cubes,
                self.num_inputs(),
                self.num_outputs(),
            )?)
        } else {
            None
        };

        // Minimize
        let (f_result, d_result, r_result) =
            esp.minimize(&f_cover, d_cover.as_ref(), r_cover.as_ref());

        // Extract minimized cubes
        let mut minimized_cubes = Vec::new();
        minimized_cubes.extend(f_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::F,
        ));
        minimized_cubes.extend(d_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::D,
        ));
        minimized_cubes.extend(r_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::R,
        ));

        // Build new cover with minimized cubes - only clone labels (Arc, cheap)
        Ok(Cover {
            num_inputs: self.num_inputs,
            num_outputs: self.num_outputs,
            input_labels: self.input_labels.clone(),
            output_labels: self.output_labels.clone(),
            cubes: minimized_cubes,
            cover_type: self.cover_type,
        })
    }

    fn minimize_exact_with_config(
        &self,
        config: &EspressoConfig,
    ) -> Result<Self, MinimizationError> {
        use crate::espresso::{Espresso, EspressoCover};

        // Split cubes into F, D, R sets based on cube type
        let mut f_cubes = Vec::new();
        let mut d_cubes = Vec::new();
        let mut r_cubes = Vec::new();

        for cube in self.cubes.iter() {
            let input_vec: Vec<u8> = cube
                .inputs
                .iter()
                .map(|&opt| match opt {
                    Some(false) => 0,
                    Some(true) => 1,
                    None => 2,
                })
                .collect();

            // Convert outputs: true → 1, false → 0
            let output_vec: Vec<u8> = cube
                .outputs
                .iter()
                .map(|&b| if b { 1 } else { 0 })
                .collect();

            // Send to appropriate set based on cube type
            match cube.cube_type {
                CubeType::F => f_cubes.push((input_vec, output_vec)),
                CubeType::D => d_cubes.push((input_vec, output_vec)),
                CubeType::R => r_cubes.push((input_vec, output_vec)),
            }
        }

        // Direct C calls - thread-safe via thread-local storage
        let esp = Espresso::new(self.num_inputs(), self.num_outputs(), config);

        // Build covers from cube data
        let f_cover = EspressoCover::from_cubes(f_cubes, self.num_inputs(), self.num_outputs())?;
        let d_cover = if !d_cubes.is_empty() {
            Some(EspressoCover::from_cubes(
                d_cubes,
                self.num_inputs(),
                self.num_outputs(),
            )?)
        } else {
            None
        };
        let r_cover = if !r_cubes.is_empty() {
            Some(EspressoCover::from_cubes(
                r_cubes,
                self.num_inputs(),
                self.num_outputs(),
            )?)
        } else {
            None
        };

        // Minimize using exact algorithm
        let (f_result, d_result, r_result) =
            esp.minimize_exact(&f_cover, d_cover.as_ref(), r_cover.as_ref());

        // Extract minimized cubes
        let mut minimized_cubes = Vec::new();
        minimized_cubes.extend(f_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::F,
        ));
        minimized_cubes.extend(d_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::D,
        ));
        minimized_cubes.extend(r_result.to_cubes(
            self.num_inputs(),
            self.num_outputs(),
            CubeType::R,
        ));

        // Build new cover with minimized cubes - only clone labels (Arc, cheap)
        Ok(Cover {
            num_inputs: self.num_inputs,
            num_outputs: self.num_outputs,
            input_labels: self.input_labels.clone(),
            output_labels: self.output_labels.clone(),
            cubes: minimized_cubes,
            cover_type: self.cover_type,
        })
    }
}

// Note: Blanket implementation of Minimizable for types convertible to/from Bdd
// is provided in the bdd module (src/expression/bdd.rs)

// Implement PLASerialisable for Cover (used for PLA I/O)
impl crate::pla::PLASerialisable for Cover {
    type CubesIter<'a> = std::slice::Iter<'a, Cube>;

    fn num_inputs(&self) -> usize {
        self.num_inputs
    }

    fn num_outputs(&self) -> usize {
        self.num_outputs
    }

    fn internal_cubes_iter(&self) -> Self::CubesIter<'_> {
        self.cubes.iter()
    }

    fn get_input_labels(&self) -> Option<&[Arc<str>]> {
        if self.input_labels.is_empty() {
            None
        } else {
            Some(self.input_labels.as_slice())
        }
    }

    fn get_output_labels(&self) -> Option<&[Arc<str>]> {
        if self.output_labels.is_empty() {
            None
        } else {
            Some(self.output_labels.as_slice())
        }
    }

    fn create_from_pla_parts(
        num_inputs: usize,
        num_outputs: usize,
        input_labels: Vec<Arc<str>>,
        output_labels: Vec<Arc<str>>,
        cubes: Vec<Cube>,
        cover_type: CoverType,
    ) -> Self {
        Cover {
            num_inputs,
            num_outputs,
            input_labels: LabelManager::from_labels(input_labels),
            output_labels: LabelManager::from_labels(output_labels),
            cubes,
            cover_type,
        }
    }
}

impl Default for Cover {
    fn default() -> Self {
        Self::new(CoverType::F)
    }
}

/// Convert a `BoolExpr` into a `Cover` with a single output named "out"
///
/// This conversion uses the BDD representation for efficient DNF extraction.
///
/// # Examples
///
/// ```
/// use espresso_logic::{BoolExpr, Cover};
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = a.and(&b);
///
/// let cover: Cover = expr.into();
/// assert_eq!(cover.num_outputs(), 1);
/// ```
impl From<crate::expression::BoolExpr> for Cover {
    fn from(expr: crate::expression::BoolExpr) -> Self {
        let mut cover = Cover::new(CoverType::F);
        cover
            .add_expr(&expr, "out")
            .expect("Adding expression to new cover should not fail");
        cover
    }
}

/// Convert a `&Bdd` into a `Cover` with a single output named "out"
///
/// This conversion extracts the cubes from the BDD representation without
/// requiring ownership of the BDD.
///
/// # Examples
///
/// ```
/// use espresso_logic::{BoolExpr, Cover};
///
/// let a = BoolExpr::variable("a");
/// let bdd = a.to_bdd();
///
/// let cover = Cover::from(&bdd);
/// assert_eq!(cover.num_outputs(), 1);
/// ```
impl From<&crate::expression::bdd::Bdd> for Cover {
    fn from(bdd: &crate::expression::bdd::Bdd) -> Self {
        use std::collections::BTreeSet;

        // Convert to DNF
        let dnf = crate::cover::dnf::Dnf::from(bdd);
        let cubes = dnf.cubes();

        // Collect all variables from the DNF cubes
        let mut all_vars = BTreeSet::new();
        for product_term in cubes {
            for var in product_term.keys() {
                all_vars.insert(Arc::clone(var));
            }
        }

        // Create cover with proper dimensions
        let var_vec: Vec<Arc<str>> = all_vars.into_iter().collect();
        let var_refs: Vec<&str> = var_vec.iter().map(|s| s.as_ref()).collect();
        let mut cover = Cover::with_labels(CoverType::F, &var_refs, &["out"]);

        // Add cubes to cover
        for product_term in cubes {
            let mut inputs = vec![None; cover.num_inputs()];
            for (var, &polarity) in product_term {
                if let Some(idx) = cover.input_labels.find_position(var) {
                    inputs[idx] = Some(polarity);
                }
            }

            let outputs = vec![true; cover.num_outputs()];
            cover.cubes.push(Cube::new(&inputs, &outputs, CubeType::F));
        }

        cover
    }
}

impl fmt::Debug for Cover {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cover")
            .field("num_inputs", &self.num_inputs)
            .field("num_outputs", &self.num_outputs)
            .field("cover_type", &self.cover_type)
            .field("num_cubes", &self.num_cubes())
            .field("input_labels", &self.input_labels)
            .field("output_labels", &self.output_labels)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cover_creation() {
        let cover = Cover::new(CoverType::F);
        assert_eq!(cover.num_inputs(), 0);
        assert_eq!(cover.num_outputs(), 0);
        assert_eq!(cover.num_cubes(), 0);
    }

    #[test]
    fn test_cover_with_labels() {
        let cover = Cover::with_labels(CoverType::F, &["a", "b", "c"], &["out"]);
        assert_eq!(cover.num_inputs(), 3);
        assert_eq!(cover.num_outputs(), 1);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");
        assert_eq!(cover.input_labels()[2].as_ref(), "c");
        assert_eq!(cover.output_labels()[0].as_ref(), "out");
    }

    #[test]
    fn test_add_cube() {
        let mut cover = Cover::new(CoverType::F);
        cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.num_outputs(), 1);
        assert_eq!(cover.num_cubes(), 1);
    }

    #[test]
    fn test_dynamic_growth() {
        let mut cover = Cover::new(CoverType::F);
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.num_outputs(), 1);

        // Add larger cube
        cover.add_cube(
            &[Some(true), Some(false), Some(true)],
            &[Some(true), Some(false)],
        );
        assert_eq!(cover.num_inputs(), 3);
        assert_eq!(cover.num_outputs(), 2);

        // Labels should NOT be auto-generated
        assert_eq!(cover.input_labels().len(), 0);
        assert_eq!(cover.output_labels().len(), 0);
    }

    #[test]
    fn test_minimize() {
        let mut cover = Cover::new(CoverType::F);
        cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        cover = cover.minimize().unwrap();
        // XOR cannot be minimized
        assert_eq!(cover.num_cubes(), 2);
    }

    // ===== Dynamic Growth Tests =====

    #[test]
    fn test_dynamic_growth_inputs_only() {
        let mut cover = Cover::new(CoverType::F);

        // Start with 2 inputs
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.num_outputs(), 1);

        // Grow to 5 inputs
        cover.add_cube(
            &[Some(true), None, Some(false), None, Some(true)],
            &[Some(true)],
        );
        assert_eq!(cover.num_inputs(), 5);
        assert_eq!(cover.num_outputs(), 1);

        // Verify all cubes have consistent dimensions
        for cube in cover.cubes() {
            assert_eq!(cube.inputs().len(), 5);
            assert_eq!(cube.outputs().len(), 1);
        }
    }

    #[test]
    fn test_dynamic_growth_outputs_only() {
        let mut cover = Cover::new(CoverType::F);

        // Start with 1 output
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.num_outputs(), 1);

        // Grow to 3 outputs
        cover.add_cube(&[Some(true), None], &[Some(true), Some(false), Some(true)]);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.num_outputs(), 3);

        // Verify all cubes have consistent dimensions
        for cube in cover.cubes() {
            assert_eq!(cube.inputs().len(), 2);
            assert_eq!(cube.outputs().len(), 3);
        }
    }

    #[test]
    fn test_dynamic_growth_both_dimensions() {
        let mut cover = Cover::new(CoverType::F);

        // Start small
        cover.add_cube(&[Some(true)], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 1);
        assert_eq!(cover.num_outputs(), 1);

        // Grow both dimensions
        cover.add_cube(&[Some(true), Some(false), None], &[Some(true), Some(false)]);
        assert_eq!(cover.num_inputs(), 3);
        assert_eq!(cover.num_outputs(), 2);

        // Add another with even more dimensions
        cover.add_cube(
            &[Some(true), Some(false), None, Some(true)],
            &[Some(true), Some(false), Some(true)],
        );
        assert_eq!(cover.num_inputs(), 4);
        assert_eq!(cover.num_outputs(), 3);

        // All cubes should have been padded
        assert_eq!(cover.num_cubes(), 3);
        for cube in cover.cubes() {
            assert_eq!(cube.inputs().len(), 4);
            assert_eq!(cube.outputs().len(), 3);
        }
    }

    #[test]
    fn test_dynamic_growth_preserves_existing_cubes() {
        let mut cover = Cover::new(CoverType::F);

        // Add first cube
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

        // Get the first cube's data before growth
        let first_cube_inputs: Vec<_> = cover.cubes().next().unwrap().inputs().to_vec();
        assert_eq!(first_cube_inputs, vec![Some(true), Some(false)]);

        // Grow dimensions
        cover.add_cube(&[Some(true), Some(false), Some(true)], &[Some(true)]);

        // First cube should be padded with None
        let first_cube_after: Vec<_> = cover.cubes().next().unwrap().inputs().to_vec();
        assert_eq!(first_cube_after, vec![Some(true), Some(false), None]);
    }

    // ===== Auto-Generated Label Tests =====

    #[test]
    fn test_auto_generated_input_labels() {
        let mut cover = Cover::new(CoverType::F);

        // Add cube with 5 inputs
        cover.add_cube(
            &[Some(true), Some(false), None, Some(true), Some(false)],
            &[Some(true)],
        );

        // Labels should NOT be auto-generated when adding cubes
        assert_eq!(cover.input_labels().len(), 0);

        // But when converting to expressions, default labels should be used
        let expr = cover.to_expr_by_index(0).unwrap();
        let vars = expr.collect_variables();
        assert_eq!(vars.len(), 4); // 4 non-don't-care inputs

        // Variable names should be x0, x1, x3, x4 (x2 is don't care so not in expr)
        let var_names: Vec<&str> = vars.iter().map(|v| v.as_ref()).collect();
        assert!(var_names.contains(&"x0"));
        assert!(var_names.contains(&"x1"));
        assert!(var_names.contains(&"x3"));
        assert!(var_names.contains(&"x4"));
    }

    #[test]
    fn test_auto_generated_output_labels() {
        let mut cover = Cover::new(CoverType::F);

        // Add cube with 4 outputs
        cover.add_cube(
            &[Some(true), Some(false)],
            &[Some(true), Some(false), Some(true), Some(false)],
        );

        // Labels should NOT be auto-generated when adding cubes
        assert_eq!(cover.output_labels().len(), 0);

        // But when using to_exprs iterator, default output names should be generated
        let exprs: Vec<_> = cover.to_exprs().collect();
        assert_eq!(exprs.len(), 4);
        assert_eq!(exprs[0].0.as_ref(), "y0");
        assert_eq!(exprs[1].0.as_ref(), "y1");
        assert_eq!(exprs[2].0.as_ref(), "y2");
        assert_eq!(exprs[3].0.as_ref(), "y3");
    }

    #[test]
    fn test_label_uniqueness_on_growth() {
        let mut cover = Cover::new(CoverType::F);

        // Add cubes causing growth
        cover.add_cube(&[Some(true)], &[Some(true)]);
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        cover.add_cube(&[Some(true), Some(false), None], &[Some(true)]);

        // Labels should NOT be auto-generated
        assert_eq!(cover.input_labels().len(), 0);

        // When converting to expression, default labels should be used
        let expr = cover.to_expr_by_index(0).unwrap();
        let vars = expr.collect_variables();
        assert_eq!(vars.len(), 2); // x0 and x1 (x2 is don't care in the 3rd cube)
    }

    #[test]
    fn test_mixed_labels_and_growth() {
        // Start with labeled cover
        let mut cover = Cover::with_labels(CoverType::F, &["a", "b"], &["out1"]);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.num_outputs(), 1);

        // Grow inputs - labels SHOULD be auto-added since cover is already labeled
        cover.add_cube(&[Some(true), Some(false), None, Some(true)], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 4);
        // All 4 input labels should exist: a, b, x2, x3
        assert_eq!(cover.input_labels().len(), 4);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");
        assert_eq!(cover.input_labels()[2].as_ref(), "x2"); // Auto-generated
        assert_eq!(cover.input_labels()[3].as_ref(), "x3"); // Auto-generated

        // Grow outputs - labels SHOULD be auto-added since cover is already labeled
        cover.add_cube(
            &[Some(true), Some(false)],
            &[Some(true), Some(false), Some(true)],
        );
        assert_eq!(cover.num_outputs(), 3);
        // All 3 output labels should exist: out1, y1, y2
        assert_eq!(cover.output_labels().len(), 3);
        assert_eq!(cover.output_labels()[0].as_ref(), "out1");
        assert_eq!(cover.output_labels()[1].as_ref(), "y1"); // Auto-generated
        assert_eq!(cover.output_labels()[2].as_ref(), "y2"); // Auto-generated

        // Verify labels are properly used in expressions
        let expr = cover.to_expr_by_index(0).unwrap();
        let vars = expr.collect_variables();
        // Should have some variables from the cover
        assert!(!vars.is_empty());
    }

    // ===== Expression Addition Tests =====

    #[test]
    fn test_add_expr_basic() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");
        let expr = a.and(&b);

        cover.add_expr(&expr, "output").unwrap();

        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.num_outputs(), 1);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");
        assert_eq!(cover.output_labels()[0].as_ref(), "output");
        assert!(cover.num_cubes() > 0);
    }

    #[test]
    fn test_add_expr_variable_matching() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");
        let c = crate::BoolExpr::variable("c");

        // Add first expression with variables a and b
        cover.add_expr(&a.and(&b), "out1").unwrap();
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");

        // Add second expression with variables b and c (b should match, c appended)
        cover.add_expr(&b.and(&c), "out2").unwrap();
        assert_eq!(cover.num_inputs(), 3);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");
        assert_eq!(cover.input_labels()[2].as_ref(), "c");

        assert_eq!(cover.num_outputs(), 2);
    }

    #[test]
    fn test_add_expr_duplicate_output_error() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");

        // Add first expression
        cover.add_expr(&a, "result").unwrap();

        // Try to add another expression with same output name - should fail
        let result = cover.add_expr(&b, "result");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_add_expr_to_different_cover_types() {
        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");

        // F type
        let mut f_cover = Cover::new(CoverType::F);
        f_cover.add_expr(&a.and(&b), "out").unwrap();
        assert_eq!(f_cover.cover_type(), CoverType::F);

        // FD type
        let mut fd_cover = Cover::new(CoverType::FD);
        fd_cover.add_expr(&a.or(&b), "out").unwrap();
        assert_eq!(fd_cover.cover_type(), CoverType::FD);

        // FR type
        let mut fr_cover = Cover::new(CoverType::FR);
        fr_cover.add_expr(&a, "out").unwrap();
        assert_eq!(fr_cover.cover_type(), CoverType::FR);

        // FDR type
        let mut fdr_cover = Cover::new(CoverType::FDR);
        fdr_cover.add_expr(&a.not(), "out").unwrap();
        assert_eq!(fdr_cover.cover_type(), CoverType::FDR);
    }

    #[test]
    fn test_add_expr_multiple_outputs() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");
        let c = crate::BoolExpr::variable("c");

        // Add three different expressions
        cover.add_expr(&a.and(&b), "and_result").unwrap();
        cover.add_expr(&a.or(&c), "or_result").unwrap();
        cover.add_expr(&b.not(), "not_result").unwrap();

        assert_eq!(cover.num_outputs(), 3);
        assert_eq!(cover.output_labels()[0].as_ref(), "and_result");
        assert_eq!(cover.output_labels()[1].as_ref(), "or_result");
        assert_eq!(cover.output_labels()[2].as_ref(), "not_result");

        // All three variables should be present
        assert_eq!(cover.num_inputs(), 3);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");
        assert_eq!(cover.input_labels()[2].as_ref(), "c");
    }

    #[test]
    fn test_add_expr_variable_ordering_preserved() {
        let mut cover = Cover::new(CoverType::F);

        let z = crate::BoolExpr::variable("z");
        let a = crate::BoolExpr::variable("a");
        let m = crate::BoolExpr::variable("m");

        // Add expression with variables in non-alphabetical order
        // Variables in BoolExpr are sorted alphabetically internally
        cover.add_expr(&z.and(&a).and(&m), "out").unwrap();

        // Variables should be in alphabetical order (a, m, z)
        assert_eq!(cover.num_inputs(), 3);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "m");
        assert_eq!(cover.input_labels()[2].as_ref(), "z");
    }

    #[test]
    fn test_add_expr_with_existing_cubes() {
        let mut cover = Cover::new(CoverType::F);

        // Add a manual cube first - no labels are generated
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.num_outputs(), 1);
        assert_eq!(cover.input_labels().len(), 0); // No labels yet
        assert_eq!(cover.output_labels().len(), 0); // No labels yet
        let initial_cubes = cover.num_cubes();

        // Add an expression with variables x0, x1 - this backfills labels
        let x0 = crate::BoolExpr::variable("x0");
        let x1 = crate::BoolExpr::variable("x1");

        // Try to add to output y0 - should FAIL because y0 was backfilled
        let result = cover.add_expr(&x0.or(&x1), "y0");
        assert!(result.is_err()); // y0 already exists after backfilling

        // Add to a different output name - should succeed
        cover.add_expr(&x0.and(&x1), "y1").unwrap();
        assert_eq!(cover.num_outputs(), 2);
        assert_eq!(cover.output_labels().len(), 2);
        assert_eq!(cover.output_labels()[0].as_ref(), "y0"); // Backfilled
        assert_eq!(cover.output_labels()[1].as_ref(), "y1"); // New
        assert!(cover.num_cubes() > initial_cubes);
    }

    // ===== Expression Conversion Tests =====

    #[test]
    fn test_to_expr_basic() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");

        cover.add_expr(&a.and(&b), "result").unwrap();

        let retrieved = cover.to_expr("result").unwrap();

        // Should be able to collect variables
        let vars = retrieved.collect_variables();
        assert_eq!(vars.len(), 2);
        assert!(vars.contains(&Arc::from("a")));
        assert!(vars.contains(&Arc::from("b")));
    }

    #[test]
    fn test_to_expr_by_index() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");

        cover.add_expr(&a, "out0").unwrap();
        cover.add_expr(&a.not(), "out1").unwrap();

        let expr0 = cover.to_expr_by_index(0).unwrap();
        let expr1 = cover.to_expr_by_index(1).unwrap();

        assert_eq!(expr0.collect_variables().len(), 1);
        assert_eq!(expr1.collect_variables().len(), 1);
    }

    #[test]
    fn test_to_expr_nonexistent() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        cover.add_expr(&a, "exists").unwrap();

        // Try to get non-existent output
        let result = cover.to_expr("doesnt_exist");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_to_expr_index_out_of_bounds() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        cover.add_expr(&a, "out").unwrap();

        // Try to get out of bounds index
        let result = cover.to_expr_by_index(1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("out of bounds"));
    }

    #[test]
    fn test_to_exprs_iterator() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");
        let c = crate::BoolExpr::variable("c");

        cover.add_expr(&a, "out1").unwrap();
        cover.add_expr(&b, "out2").unwrap();
        cover.add_expr(&c, "out3").unwrap();

        let exprs: Vec<_> = cover.to_exprs().collect();
        assert_eq!(exprs.len(), 3);

        assert_eq!(exprs[0].0.as_ref(), "out1");
        assert_eq!(exprs[1].0.as_ref(), "out2");
        assert_eq!(exprs[2].0.as_ref(), "out3");

        // Each expression should have one variable
        assert_eq!(exprs[0].1.collect_variables().len(), 1);
        assert_eq!(exprs[1].1.collect_variables().len(), 1);
        assert_eq!(exprs[2].1.collect_variables().len(), 1);
    }

    #[test]
    fn test_to_exprs_after_minimization() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");
        let c = crate::BoolExpr::variable("c");

        // Add redundant expression: a*b + a*b*c
        let redundant = a.and(&b).or(&a.and(&b).and(&c));
        cover.add_expr(&redundant, "out").unwrap();

        let cubes_before = cover.num_cubes();
        cover = cover.minimize().unwrap();
        let cubes_after = cover.num_cubes();

        // Should minimize
        assert!(cubes_after <= cubes_before);

        // Should still be able to convert to expression
        let minimized = cover.to_expr("out").unwrap();
        let vars = minimized.collect_variables();
        assert!(vars.len() >= 2); // At least a and b
    }

    // ===== Cover Type Tests =====

    #[test]
    fn test_f_type_cover() {
        let mut cover = Cover::new(CoverType::F);

        // F type only accepts Some(true) for outputs
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        assert_eq!(cover.num_cubes(), 1);

        // Some(false) and None are ignored for F type
        cover.add_cube(&[Some(true), Some(true)], &[Some(false)]);
        cover.add_cube(&[Some(false), Some(false)], &[None]);

        // Should still have only 1 cube (F type)
        assert_eq!(cover.num_cubes(), 1);
    }

    #[test]
    fn test_fd_type_cover() {
        let mut cover = Cover::new(CoverType::FD);

        // FD type accepts Some(true) and None
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // F cube
        cover.add_cube(&[Some(false), Some(true)], &[None]); // D cube

        // For FD type, num_cubes() only counts F cubes
        assert_eq!(cover.num_cubes(), 1);

        // But internal cubes should have both
        assert_eq!(cover.cubes.len(), 2);
    }

    #[test]
    fn test_fr_type_cover() {
        let mut cover = Cover::new(CoverType::FR);

        // FR type accepts Some(true) and Some(false)
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // F cube
        cover.add_cube(&[Some(false), Some(true)], &[Some(false)]); // R cube

        // For FR type, num_cubes() counts all cubes
        assert_eq!(cover.num_cubes(), 2);
    }

    #[test]
    fn test_fdr_type_cover() {
        let mut cover = Cover::new(CoverType::FDR);

        // FDR type accepts all: Some(true), Some(false), None
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // F cube
        cover.add_cube(&[Some(false), Some(true)], &[Some(false)]); // R cube
        cover.add_cube(&[Some(true), Some(true)], &[None]); // D cube

        // For FDR type, num_cubes() counts all cubes
        assert_eq!(cover.num_cubes(), 3);
    }

    // ===== Mixed Operations Tests =====

    #[test]
    fn test_add_cubes_then_expressions() {
        let mut cover = Cover::new(CoverType::F);

        // Add manual cubes first - no labels generated
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.input_labels().len(), 0); // No labels yet
        assert_eq!(cover.output_labels().len(), 0); // No labels yet

        // Now add expression with named variables - this backfills labels for existing dimensions
        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");

        cover.add_expr(&a.and(&b), "y1").unwrap();

        // Should have 4 inputs now: 2 from cube (x0, x1) + 2 from expression (a, b)
        assert_eq!(cover.num_inputs(), 4);
        // All 4 should have labels: x0, x1 (backfilled), a, b (from expression)
        assert_eq!(cover.input_labels().len(), 4);
        assert_eq!(cover.input_labels()[0].as_ref(), "x0");
        assert_eq!(cover.input_labels()[1].as_ref(), "x1");
        assert_eq!(cover.input_labels()[2].as_ref(), "a");
        assert_eq!(cover.input_labels()[3].as_ref(), "b");

        // Should have 2 outputs with labels
        assert_eq!(cover.num_outputs(), 2);
        assert_eq!(cover.output_labels().len(), 2);
        assert_eq!(cover.output_labels()[0].as_ref(), "y0"); // Backfilled
        assert_eq!(cover.output_labels()[1].as_ref(), "y1"); // From expression
    }

    #[test]
    fn test_add_expressions_then_cubes() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");

        // Add expression first - no backfilling needed since cover is empty
        cover.add_expr(&a.and(&b), "result").unwrap();
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");

        // Add manual cube with more inputs - should auto-extend labels since cover is in labeled mode
        cover.add_cube(
            &[Some(true), Some(false), Some(true)],
            &[Some(true), Some(false)],
        );

        // Should grow to 3 inputs, 2 outputs
        assert_eq!(cover.num_inputs(), 3);
        assert_eq!(cover.num_outputs(), 2);

        // Original labels preserved, and new labels auto-generated
        assert_eq!(cover.input_labels().len(), 3);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");
        assert_eq!(cover.input_labels()[2].as_ref(), "x2"); // Auto-generated

        // Output labels should also be extended
        assert_eq!(cover.output_labels().len(), 2);
        assert_eq!(cover.output_labels()[0].as_ref(), "result");
        assert_eq!(cover.output_labels()[1].as_ref(), "y1"); // Auto-generated
    }

    #[test]
    fn test_complex_expression_with_minimization() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");
        let c = crate::BoolExpr::variable("c");

        // Consensus theorem: a*b + ~a*c + b*c (b*c is redundant)
        let expr = a.and(&b).or(&a.not().and(&c)).or(&b.and(&c));
        cover.add_expr(&expr, "consensus").unwrap();

        // BDD automatically optimizes during conversion, so we get 2 cubes directly
        // (b*c is recognized as redundant by the canonical BDD representation)
        assert_eq!(cover.num_cubes(), 2);

        cover = cover.minimize().unwrap();

        // Should still have 2 cubes after minimization
        assert_eq!(cover.num_cubes(), 2);

        // Should still be able to convert back
        let minimized = cover.to_expr("consensus").unwrap();
        assert_eq!(minimized.collect_variables().len(), 3);
    }

    #[test]
    fn test_empty_cover_to_expr() {
        let cover = Cover::new(CoverType::F);

        // Try to get expression from empty cover - should fail
        let result = cover.to_expr_by_index(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_expression_with_constants() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let t = crate::BoolExpr::constant(true);

        // Expression with constant: a * true = a
        let expr = a.and(&t);
        cover.add_expr(&expr, "out").unwrap();

        // Should have one variable
        assert_eq!(cover.num_inputs(), 1);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
    }

    #[test]
    fn test_dynamic_naming_no_collision() {
        let mut cover = Cover::new(CoverType::F);

        // Add cubes - no labels are auto-generated
        cover.add_cube(&[Some(true), Some(false), None], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 3);
        assert_eq!(cover.input_labels().len(), 0); // No labels yet

        // Now add expression with variables "x1" and "other"
        // This backfills x0, x1, x2 for existing dimensions, then x1 matches existing x1
        let x1 = crate::BoolExpr::variable("x1");
        let other = crate::BoolExpr::variable("other");

        cover.add_expr(&x1.and(&other), "y1").unwrap();

        // Should have 4 inputs: 3 from cube (x0, x1, x2) + 1 new (other)
        // x1 from expression matches the backfilled x1
        assert_eq!(cover.num_inputs(), 4);

        // All 4 should have labels: x0, x1, x2 (backfilled), other (from expression)
        assert_eq!(cover.input_labels().len(), 4);
        assert_eq!(cover.input_labels()[0].as_ref(), "x0");
        assert_eq!(cover.input_labels()[1].as_ref(), "x1");
        assert_eq!(cover.input_labels()[2].as_ref(), "x2");
        assert_eq!(cover.input_labels()[3].as_ref(), "other");
    }

    #[test]
    fn test_pla_roundtrip_with_expressions() {
        use crate::{PLAReader, PLAWriter};

        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");

        cover.add_expr(&a.and(&b), "output").unwrap();

        // Convert to PLA string
        let pla_string = cover.to_pla_string(CoverType::F).unwrap();

        // Parse it back
        let cover2 = Cover::from_pla_string(&pla_string).unwrap();

        // Should have same dimensions
        assert_eq!(cover2.num_inputs(), cover.num_inputs());
        assert_eq!(cover2.num_outputs(), cover.num_outputs());
        assert_eq!(cover2.num_cubes(), cover.num_cubes());

        // Labels should be preserved
        assert_eq!(cover2.input_labels()[0].as_ref(), "a");
        assert_eq!(cover2.input_labels()[1].as_ref(), "b");
        assert_eq!(cover2.output_labels()[0].as_ref(), "output");
    }

    #[test]
    fn test_minimize_preserves_structure() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");

        cover.add_expr(&a.and(&b), "out1").unwrap();
        cover.add_expr(&a.or(&b), "out2").unwrap();

        let inputs_before = cover.num_inputs();
        let outputs_before = cover.num_outputs();

        cover = cover.minimize().unwrap();

        // Dimensions should be preserved
        assert_eq!(cover.num_inputs(), inputs_before);
        assert_eq!(cover.num_outputs(), outputs_before);

        // Should still be able to extract both expressions
        let expr1 = cover.to_expr("out1").unwrap();
        let expr2 = cover.to_expr("out2").unwrap();

        assert!(expr1.collect_variables().len() <= 2);
        assert!(expr2.collect_variables().len() <= 2);
    }

    #[test]
    fn test_unlabeled_cover_to_expr_uses_auto_names() {
        let mut cover = Cover::new(CoverType::F);

        // Add cube without any labels
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

        // Convert to expression
        let expr = cover.to_expr_by_index(0).unwrap();

        // Expression should use auto-generated names x0, x1
        let vars = expr.collect_variables();
        assert_eq!(vars.len(), 2);
        assert!(vars.contains(&Arc::from("x0")));
        assert!(vars.contains(&Arc::from("x1")));
    }
}
