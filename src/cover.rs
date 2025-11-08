//! Cover types and traits for Boolean function minimization
//!
//! This module provides the unified Cover type for working with covers (sum-of-products representations
//! of Boolean functions). The Cover type supports dynamic dimensions that grow as cubes are added,
//! and can work with both manually constructed cubes and boolean expressions.

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::sync::Arc;

use crate::error::EspressoError;
use crate::expression::BoolExpr;
use crate::EspressoConfig;

/// Type alias for complex cube iterator return type
pub type CubeIterator<'a> = Box<dyn Iterator<Item = (Vec<Option<bool>>, Vec<Option<bool>>)> + 'a>;

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

/// Internal trait for types that can be minimized
/// Contains implementation details needed by the minimization algorithm
pub(crate) trait Minimizable: Send + Sync {
    /// Iterate over typed cubes (required for minimization)
    fn internal_cubes_iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cube> + 'a>;

    /// Set cubes after minimization (required for minimization)
    fn set_cubes(&mut self, cubes: Vec<Cube>);
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
/// use espresso_logic::{Cover, CoverType};
///
/// // Create an empty cover
/// let mut cover = Cover::new(CoverType::F);
///
/// // Add cubes (dimensions grow automatically)
/// cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
/// cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
///
/// // Minimize it
/// cover.minimize().unwrap();
///
/// println!("Minimized to {} cubes", cover.num_cubes());
/// ```
#[derive(Clone)]
pub struct Cover {
    /// Number of input variables
    num_inputs: usize,
    /// Number of output variables
    num_outputs: usize,
    /// Input variable labels (Arc for efficient cloning)
    input_labels: Vec<Arc<str>>,
    /// Output variable labels (Arc for efficient cloning)
    output_labels: Vec<Arc<str>>,
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
            input_labels: Vec::new(),
            output_labels: Vec::new(),
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
        let input_labels: Vec<Arc<str>> =
            input_labels.iter().map(|s| Arc::from(s.as_ref())).collect();
        let output_labels: Vec<Arc<str>> = output_labels
            .iter()
            .map(|s| Arc::from(s.as_ref()))
            .collect();

        Cover {
            num_inputs: input_labels.len(),
            num_outputs: output_labels.len(),
            input_labels,
            output_labels,
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
    /// Returns a slice of Arc<str> for efficient access to variable names.
    pub fn input_labels(&self) -> &[Arc<str>] {
        &self.input_labels
    }

    /// Get output variable labels
    ///
    /// Returns a slice of Arc<str> for efficient access to variable names.
    pub fn output_labels(&self) -> &[Arc<str>] {
        &self.output_labels
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
    pub fn cubes<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cube> + 'a> {
        // For F-type covers, only return F cubes; for FD/FR/FDR, return all
        let cover_type = self.cover_type;
        Box::new(
            self.cubes
                .iter()
                .filter(move |cube| cover_type != CoverType::F || cube.cube_type == CubeType::F),
        )
    }

    /// Iterate over cubes (inputs, outputs)
    ///
    /// Returns cubes in a format compatible with add_cube (owned vecs for easy use)
    pub fn cubes_iter<'a>(&'a self) -> CubeIterator<'a> {
        let cover_type = self.cover_type;
        Box::new(
            self.cubes
                .iter()
                .filter(move |cube| cover_type != CoverType::F || cube.cube_type == CubeType::F)
                .map(|cube| {
                    let inputs = cube.inputs.to_vec();
                    let outputs: Vec<Option<bool>> =
                        cube.outputs.iter().map(|&b| Some(b)).collect();
                    (inputs, outputs)
                }),
        )
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
    /// This extends all existing cubes and adds generated labels as needed.
    fn grow_to_fit(&mut self, min_inputs: usize, min_outputs: usize) {
        // Grow inputs if needed
        if min_inputs > self.num_inputs {
            let old_size = self.num_inputs;
            self.num_inputs = min_inputs;

            // Extend all existing cubes
            for cube in &mut self.cubes {
                let mut new_inputs = cube.inputs.to_vec();
                new_inputs.resize(self.num_inputs, None);
                cube.inputs = new_inputs.into();
            }

            // Add generated labels: x0, x1, x2, etc.
            for i in old_size..self.num_inputs {
                self.input_labels
                    .push(Arc::from(format!("x{}", i).as_str()));
            }
        }

        // Grow outputs if needed
        if min_outputs > self.num_outputs {
            let old_size = self.num_outputs;
            self.num_outputs = min_outputs;

            // Extend all existing cubes
            for cube in &mut self.cubes {
                let mut new_outputs = cube.outputs.to_vec();
                new_outputs.resize(self.num_outputs, false);
                cube.outputs = new_outputs.into();
            }

            // Add generated labels: y0, y1, y2, etc.
            for i in old_size..self.num_outputs {
                self.output_labels
                    .push(Arc::from(format!("y{}", i).as_str()));
            }
        }
    }

    /// Minimize this cover in-place using default configuration
    pub fn minimize(&mut self) -> io::Result<()> {
        let config = EspressoConfig::default();
        self.minimize_with_config(&config)
    }

    /// Minimize this cover in-place with custom configuration
    pub fn minimize_with_config(&mut self, config: &EspressoConfig) -> io::Result<()> {
        use crate::espresso::{Espresso, EspressoCover};

        // Split cubes into F, D, R sets based on cube type
        let mut f_cubes = Vec::new();
        let mut d_cubes = Vec::new();
        let mut r_cubes = Vec::new();

        for cube in self.internal_cubes_iter() {
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
        let (f_result, d_result, r_result) = esp.minimize(f_cover, d_cover, r_cover);

        // Extract cubes and combine
        let mut all_cubes = Vec::new();
        all_cubes.extend(f_result.to_cubes(self.num_inputs(), self.num_outputs(), CubeType::F));
        all_cubes.extend(d_result.to_cubes(self.num_inputs(), self.num_outputs(), CubeType::D));
        all_cubes.extend(r_result.to_cubes(self.num_inputs(), self.num_outputs(), CubeType::R));

        // Update cubes with type information preserved
        self.set_cubes(all_cubes);
        Ok(())
    }

    /// Add a boolean expression to a named output
    ///
    /// This method converts the expression to DNF cubes and adds them to the cover.
    /// Input variables from the expression are matched by name with existing variables,
    /// and new variables are appended in order.
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
    /// cover.add_expr(expr, "output1").unwrap();
    /// assert_eq!(cover.num_inputs(), 2);
    /// assert_eq!(cover.num_outputs(), 1);
    /// ```
    pub fn add_expr(&mut self, expr: BoolExpr, output_name: &str) -> Result<(), EspressoError> {
        // Check if output already exists
        if self
            .output_labels
            .iter()
            .any(|label| label.as_ref() == output_name)
        {
            return Err(EspressoError::InvalidInput {
                message: format!("Output '{}' already exists in cover", output_name),
            });
        }

        // Collect variables from expression (in sorted order)
        let expr_variables: Vec<Arc<str>> = expr.collect_variables().into_iter().collect();

        // Build variable mapping: expr variable -> cover input index
        let mut var_to_index: BTreeMap<Arc<str>, usize> = BTreeMap::new();

        for expr_var in &expr_variables {
            // Check if variable already exists in cover
            if let Some(pos) = self.input_labels.iter().position(|label| label == expr_var) {
                var_to_index.insert(Arc::clone(expr_var), pos);
            } else {
                // New variable - add to cover
                let new_index = self.num_inputs;
                self.input_labels.push(Arc::clone(expr_var));
                var_to_index.insert(Arc::clone(expr_var), new_index);
                self.num_inputs += 1;

                // Extend all existing cubes with None for new input
                for cube in &mut self.cubes {
                    let mut new_inputs = cube.inputs.to_vec();
                    new_inputs.push(None);
                    cube.inputs = new_inputs.into();
                }
            }
        }

        // Convert expression to DNF
        let dnf = to_dnf(&expr);

        // Add output variable
        let output_index = self.num_outputs;
        self.output_labels.push(Arc::from(output_name));
        self.num_outputs += 1;

        // Extend all existing cubes with false for new output
        for cube in &mut self.cubes {
            let mut new_outputs = cube.outputs.to_vec();
            new_outputs.push(false);
            cube.outputs = new_outputs.into();
        }

        // Convert DNF to cubes and add to cover
        for product_term in dnf {
            // Build input vector based on variable mapping
            let mut inputs = vec![None; self.num_inputs];
            for (var, &polarity) in &product_term {
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
    /// cover.add_expr(a.clone(), "out1").unwrap();
    /// cover.add_expr(b.clone(), "out2").unwrap();
    ///
    /// for (name, expr) in cover.to_exprs() {
    ///     println!("{}: {}", name, expr);
    /// }
    /// ```
    pub fn to_exprs(&self) -> impl Iterator<Item = (Arc<str>, BoolExpr)> + '_ {
        (0..self.num_outputs).map(move |output_idx| {
            let name = Arc::clone(&self.output_labels[output_idx]);
            let expr = self
                .to_expr_by_index(output_idx)
                .unwrap_or_else(|_| BoolExpr::constant(false));
            (name, expr)
        })
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
    /// cover.add_expr(a, "result").unwrap();
    /// let expr = cover.to_expr("result").unwrap();
    /// println!("result: {}", expr);
    /// ```
    pub fn to_expr(&self, output_name: &str) -> Result<BoolExpr, EspressoError> {
        let output_idx = self
            .output_labels
            .iter()
            .position(|label| label.as_ref() == output_name)
            .ok_or_else(|| EspressoError::InvalidInput {
                message: format!("Output '{}' not found in cover", output_name),
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
    /// cover.add_expr(a, "out").unwrap();
    /// let expr = cover.to_expr_by_index(0).unwrap();
    /// println!("Output 0: {}", expr);
    /// ```
    pub fn to_expr_by_index(&self, output_idx: usize) -> Result<BoolExpr, EspressoError> {
        if output_idx >= self.num_outputs {
            return Err(EspressoError::InvalidInput {
                message: format!(
                    "Output index {} out of bounds (have {} outputs)",
                    output_idx, self.num_outputs
                ),
            });
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

        cubes_to_expr(&relevant_cubes, &self.input_labels)
    }
}

/// Convert a boolean expression to Disjunctive Normal Form (DNF)
/// Returns a vector of product terms, where each term is a map from variable to its literal value
/// (true for positive literal, false for negative literal)
fn to_dnf(expr: &BoolExpr) -> Vec<BTreeMap<Arc<str>, bool>> {
    use crate::expression::BoolExprInner;

    match expr.inner() {
        BoolExprInner::Constant(true) => {
            // True constant = one product term with no literals (tautology)
            vec![BTreeMap::new()]
        }
        BoolExprInner::Constant(false) => {
            // False constant = no product terms (empty sum)
            vec![]
        }
        BoolExprInner::Variable(name) => {
            // Single variable = one product with positive literal
            let mut term = BTreeMap::new();
            term.insert(Arc::clone(name), true);
            vec![term]
        }
        BoolExprInner::Not(inner) => {
            // NOT is handled recursively with De Morgan's laws
            to_dnf_not(inner)
        }
        BoolExprInner::And(left, right) => {
            // AND: cross product of terms from each side
            let left_dnf = to_dnf(left);
            let right_dnf = to_dnf(right);

            let mut result = Vec::new();
            for left_term in &left_dnf {
                for right_term in &right_dnf {
                    // Merge terms, checking for contradictions (x AND ~x)
                    if let Some(merged) = merge_product_terms(left_term, right_term) {
                        result.push(merged);
                    }
                }
            }
            result
        }
        BoolExprInner::Or(left, right) => {
            // OR: union of terms from each side
            let mut left_dnf = to_dnf(left);
            let right_dnf = to_dnf(right);
            left_dnf.extend(right_dnf);
            left_dnf
        }
    }
}

/// Convert NOT expression to DNF using De Morgan's laws
fn to_dnf_not(expr: &BoolExpr) -> Vec<BTreeMap<Arc<str>, bool>> {
    use crate::expression::BoolExprInner;

    match expr.inner() {
        BoolExprInner::Constant(val) => {
            // NOT of constant
            to_dnf(&BoolExpr::constant(!val))
        }
        BoolExprInner::Variable(name) => {
            // NOT of variable = one product with negative literal
            let mut term = BTreeMap::new();
            term.insert(Arc::clone(name), false);
            vec![term]
        }
        BoolExprInner::Not(inner) => {
            // Double negation
            to_dnf(inner)
        }
        BoolExprInner::And(left, right) => {
            // De Morgan: ~(A * B) = ~A + ~B
            let not_left = left.not();
            let not_right = right.not();
            to_dnf(&not_left.or(&not_right))
        }
        BoolExprInner::Or(left, right) => {
            // De Morgan: ~(A + B) = ~A * ~B
            let not_left = left.not();
            let not_right = right.not();
            to_dnf(&not_left.and(&not_right))
        }
    }
}

/// Merge two product terms (AND them together)
/// Returns None if they contradict (e.g., x AND ~x)
fn merge_product_terms(
    left: &BTreeMap<Arc<str>, bool>,
    right: &BTreeMap<Arc<str>, bool>,
) -> Option<BTreeMap<Arc<str>, bool>> {
    let mut result = left.clone();

    for (var, &polarity) in right {
        if let Some(&existing) = result.get(var) {
            if existing != polarity {
                // Contradiction: x AND ~x = false
                return None;
            }
        } else {
            result.insert(Arc::clone(var), polarity);
        }
    }

    Some(result)
}

/// Convert cube references back to a boolean expression
fn cubes_to_expr(cubes: &[&Cube], variables: &[Arc<str>]) -> Result<BoolExpr, EspressoError> {
    if cubes.is_empty() {
        return Ok(BoolExpr::constant(false));
    }

    let mut terms = Vec::new();

    for cube in cubes.iter() {
        // Build product term for this cube
        let mut factors = Vec::new();

        for (i, var) in variables.iter().enumerate() {
            match cube.inputs.get(i) {
                Some(Some(true)) => {
                    // Positive literal
                    factors.push(BoolExpr::variable(var));
                }
                Some(Some(false)) => {
                    // Negative literal
                    factors.push(BoolExpr::variable(var).not());
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
        Ok(BoolExpr::constant(false))
    } else {
        Ok(terms.into_iter().reduce(|acc, t| acc.or(&t)).unwrap())
    }
}

// Implement Minimizable for Cover (used by minimization algorithm)
impl Minimizable for Cover {
    fn internal_cubes_iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cube> + 'a> {
        Box::new(self.cubes.iter())
    }

    fn set_cubes(&mut self, cubes: Vec<Cube>) {
        // Filter cubes based on the cover type
        self.cubes = cubes
            .into_iter()
            .filter(|cube| match cube.cube_type {
                CubeType::F => self.cover_type.has_f(),
                CubeType::D => self.cover_type.has_d(),
                CubeType::R => self.cover_type.has_r(),
            })
            .collect();
    }
}

// Implement PLASerialisable for Cover (used for PLA I/O)
impl crate::pla::PLASerialisable for Cover {
    fn num_inputs(&self) -> usize {
        self.num_inputs
    }

    fn num_outputs(&self) -> usize {
        self.num_outputs
    }

    fn internal_cubes_iter(&self) -> Box<dyn Iterator<Item = &Cube> + '_> {
        Box::new(self.cubes.iter())
    }

    fn get_input_labels(&self) -> Option<&[Arc<str>]> {
        if self.input_labels.is_empty() {
            None
        } else {
            Some(&self.input_labels)
        }
    }

    fn get_output_labels(&self) -> Option<&[Arc<str>]> {
        if self.output_labels.is_empty() {
            None
        } else {
            Some(&self.output_labels)
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
            input_labels,
            output_labels,
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

        // Check generated labels
        assert_eq!(cover.input_labels()[0].as_ref(), "x0");
        assert_eq!(cover.input_labels()[1].as_ref(), "x1");
        assert_eq!(cover.input_labels()[2].as_ref(), "x2");
        assert_eq!(cover.output_labels()[0].as_ref(), "y0");
        assert_eq!(cover.output_labels()[1].as_ref(), "y1");
    }

    #[test]
    fn test_minimize() {
        let mut cover = Cover::new(CoverType::F);
        cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        cover.minimize().unwrap();
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

        // Check auto-generated labels
        assert_eq!(cover.input_labels().len(), 5);
        assert_eq!(cover.input_labels()[0].as_ref(), "x0");
        assert_eq!(cover.input_labels()[1].as_ref(), "x1");
        assert_eq!(cover.input_labels()[2].as_ref(), "x2");
        assert_eq!(cover.input_labels()[3].as_ref(), "x3");
        assert_eq!(cover.input_labels()[4].as_ref(), "x4");
    }

    #[test]
    fn test_auto_generated_output_labels() {
        let mut cover = Cover::new(CoverType::F);

        // Add cube with 4 outputs
        cover.add_cube(
            &[Some(true), Some(false)],
            &[Some(true), Some(false), Some(true), Some(false)],
        );

        // Check auto-generated labels
        assert_eq!(cover.output_labels().len(), 4);
        assert_eq!(cover.output_labels()[0].as_ref(), "y0");
        assert_eq!(cover.output_labels()[1].as_ref(), "y1");
        assert_eq!(cover.output_labels()[2].as_ref(), "y2");
        assert_eq!(cover.output_labels()[3].as_ref(), "y3");
    }

    #[test]
    fn test_label_uniqueness_on_growth() {
        let mut cover = Cover::new(CoverType::F);

        // Add cubes causing growth
        cover.add_cube(&[Some(true)], &[Some(true)]);
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        cover.add_cube(&[Some(true), Some(false), None], &[Some(true)]);

        let labels = cover.input_labels();

        // Check all labels are unique
        use std::collections::HashSet;
        let unique_labels: HashSet<_> = labels.iter().collect();
        assert_eq!(unique_labels.len(), 3);

        // Check sequential naming
        assert_eq!(labels[0].as_ref(), "x0");
        assert_eq!(labels[1].as_ref(), "x1");
        assert_eq!(labels[2].as_ref(), "x2");
    }

    #[test]
    fn test_mixed_labels_and_growth() {
        // Start with labeled cover
        let mut cover = Cover::with_labels(CoverType::F, &["a", "b"], &["out1"]);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.num_outputs(), 1);

        // Grow inputs - should add x2, x3, etc
        cover.add_cube(&[Some(true), Some(false), None, Some(true)], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 4);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");
        assert_eq!(cover.input_labels()[2].as_ref(), "x2");
        assert_eq!(cover.input_labels()[3].as_ref(), "x3");

        // Grow outputs - should add y1, y2, etc
        cover.add_cube(
            &[Some(true), Some(false)],
            &[Some(true), Some(false), Some(true)],
        );
        assert_eq!(cover.num_outputs(), 3);
        assert_eq!(cover.output_labels()[0].as_ref(), "out1");
        assert_eq!(cover.output_labels()[1].as_ref(), "y1");
        assert_eq!(cover.output_labels()[2].as_ref(), "y2");
    }

    // ===== Expression Addition Tests =====

    #[test]
    fn test_add_expr_basic() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");
        let expr = a.and(&b);

        cover.add_expr(expr, "output").unwrap();

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
        cover.add_expr(a.and(&b), "out1").unwrap();
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");

        // Add second expression with variables b and c (b should match, c appended)
        cover.add_expr(b.and(&c), "out2").unwrap();
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
        cover.add_expr(a.clone(), "result").unwrap();

        // Try to add another expression with same output name - should fail
        let result = cover.add_expr(b.clone(), "result");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_add_expr_to_different_cover_types() {
        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");

        // F type
        let mut f_cover = Cover::new(CoverType::F);
        f_cover.add_expr(a.and(&b), "out").unwrap();
        assert_eq!(f_cover.cover_type(), CoverType::F);

        // FD type
        let mut fd_cover = Cover::new(CoverType::FD);
        fd_cover.add_expr(a.or(&b), "out").unwrap();
        assert_eq!(fd_cover.cover_type(), CoverType::FD);

        // FR type
        let mut fr_cover = Cover::new(CoverType::FR);
        fr_cover.add_expr(a.clone(), "out").unwrap();
        assert_eq!(fr_cover.cover_type(), CoverType::FR);

        // FDR type
        let mut fdr_cover = Cover::new(CoverType::FDR);
        fdr_cover.add_expr(a.not(), "out").unwrap();
        assert_eq!(fdr_cover.cover_type(), CoverType::FDR);
    }

    #[test]
    fn test_add_expr_multiple_outputs() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");
        let c = crate::BoolExpr::variable("c");

        // Add three different expressions
        cover.add_expr(a.and(&b), "and_result").unwrap();
        cover.add_expr(a.or(&c), "or_result").unwrap();
        cover.add_expr(b.not(), "not_result").unwrap();

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
        cover.add_expr(z.and(&a).and(&m), "out").unwrap();

        // Variables should be in alphabetical order (a, m, z)
        assert_eq!(cover.num_inputs(), 3);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "m");
        assert_eq!(cover.input_labels()[2].as_ref(), "z");
    }

    #[test]
    fn test_add_expr_with_existing_cubes() {
        let mut cover = Cover::new(CoverType::F);

        // Add a manual cube first
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.num_outputs(), 1);
        let initial_cubes = cover.num_cubes();

        // Add an expression (should match x0, x1 and add to y0)
        let x0 = crate::BoolExpr::variable("x0");
        let x1 = crate::BoolExpr::variable("x1");

        // This should fail because y0 already exists
        let result = cover.add_expr(x0.or(&x1), "y0");
        assert!(result.is_err());

        // Add to a different output
        cover.add_expr(x0.and(&x1), "y1").unwrap();
        assert_eq!(cover.num_outputs(), 2);
        assert!(cover.num_cubes() > initial_cubes);
    }

    // ===== Expression Conversion Tests =====

    #[test]
    fn test_to_expr_basic() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");

        cover.add_expr(a.and(&b), "result").unwrap();

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

        cover.add_expr(a.clone(), "out0").unwrap();
        cover.add_expr(a.not(), "out1").unwrap();

        let expr0 = cover.to_expr_by_index(0).unwrap();
        let expr1 = cover.to_expr_by_index(1).unwrap();

        assert_eq!(expr0.collect_variables().len(), 1);
        assert_eq!(expr1.collect_variables().len(), 1);
    }

    #[test]
    fn test_to_expr_nonexistent() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        cover.add_expr(a, "exists").unwrap();

        // Try to get non-existent output
        let result = cover.to_expr("doesnt_exist");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_to_expr_index_out_of_bounds() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        cover.add_expr(a, "out").unwrap();

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

        cover.add_expr(a.clone(), "out1").unwrap();
        cover.add_expr(b.clone(), "out2").unwrap();
        cover.add_expr(c.clone(), "out3").unwrap();

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
        cover.add_expr(redundant, "out").unwrap();

        let cubes_before = cover.num_cubes();
        cover.minimize().unwrap();
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

        // Add manual cubes first
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.input_labels()[0].as_ref(), "x0");
        assert_eq!(cover.input_labels()[1].as_ref(), "x1");

        // Now add expression with named variables
        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");

        cover.add_expr(a.and(&b), "y1").unwrap();

        // Should have 4 inputs now: x0, x1, a, b
        assert_eq!(cover.num_inputs(), 4);
        assert_eq!(cover.input_labels()[0].as_ref(), "x0");
        assert_eq!(cover.input_labels()[1].as_ref(), "x1");
        assert_eq!(cover.input_labels()[2].as_ref(), "a");
        assert_eq!(cover.input_labels()[3].as_ref(), "b");

        // Should have 2 outputs
        assert_eq!(cover.num_outputs(), 2);
    }

    #[test]
    fn test_add_expressions_then_cubes() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");

        // Add expression first
        cover.add_expr(a.and(&b), "result").unwrap();
        assert_eq!(cover.num_inputs(), 2);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");

        // Add manual cube with more inputs
        cover.add_cube(
            &[Some(true), Some(false), Some(true)],
            &[Some(true), Some(false)],
        );

        // Should grow to 3 inputs, 2 outputs
        assert_eq!(cover.num_inputs(), 3);
        assert_eq!(cover.num_outputs(), 2);

        // Original labels preserved, new ones added
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
        assert_eq!(cover.input_labels()[1].as_ref(), "b");
        assert_eq!(cover.input_labels()[2].as_ref(), "x2");
        assert_eq!(cover.output_labels()[0].as_ref(), "result");
        assert_eq!(cover.output_labels()[1].as_ref(), "y1");
    }

    #[test]
    fn test_complex_expression_with_minimization() {
        let mut cover = Cover::new(CoverType::F);

        let a = crate::BoolExpr::variable("a");
        let b = crate::BoolExpr::variable("b");
        let c = crate::BoolExpr::variable("c");

        // Consensus theorem: a*b + ~a*c + b*c (b*c is redundant)
        let expr = a.and(&b).or(&a.not().and(&c)).or(&b.and(&c));
        cover.add_expr(expr, "consensus").unwrap();

        assert_eq!(cover.num_cubes(), 3);

        cover.minimize().unwrap();

        // Should minimize to 2 cubes (b*c is redundant)
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
        cover.add_expr(expr, "out").unwrap();

        // Should have one variable
        assert_eq!(cover.num_inputs(), 1);
        assert_eq!(cover.input_labels()[0].as_ref(), "a");
    }

    #[test]
    fn test_dynamic_naming_no_collision() {
        let mut cover = Cover::new(CoverType::F);

        // Add cubes causing auto-generation of x0, x1, x2
        cover.add_cube(&[Some(true), Some(false), None], &[Some(true)]);

        // Now add expression with variable "x1" - should not collide
        let x1 = crate::BoolExpr::variable("x1");
        let other = crate::BoolExpr::variable("other");

        cover.add_expr(x1.and(&other), "y1").unwrap();

        // Should have 4 inputs: x0, x1 (from cube), x1 (from expr - same), other
        // Actually x1 should match, so only 4 total
        assert_eq!(cover.num_inputs(), 4);

        // x1 should match the existing position 1
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

        cover.add_expr(a.and(&b), "output").unwrap();

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

        cover.add_expr(a.and(&b), "out1").unwrap();
        cover.add_expr(a.or(&b), "out2").unwrap();

        let inputs_before = cover.num_inputs();
        let outputs_before = cover.num_outputs();

        cover.minimize().unwrap();

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
