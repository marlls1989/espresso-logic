//! Expression integration for Cover
//!
//! This module provides methods for converting between covers and boolean expressions,
//! allowing seamless integration with the expression API.

use super::cubes::{Cube, CubeType};
use super::error::{AddExprError, CoverError, ToExprError};
use super::iterators::ToExprs;
use super::minterm::Minterm;
use super::Cover;
use crate::expression::BoolExpr;
use std::sync::Arc;

impl Cover {
    /// Add a boolean function to a named output
    ///
    /// The expression's product terms (extracted from its internal BDD) become F cubes
    /// asserting `output_name`. Input variables are matched by name with existing variables,
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
    /// // Add expression to cover
    /// cover.add_expr(&expr, "output1").unwrap();
    /// assert_eq!(cover.num_inputs(), 2);
    /// assert_eq!(cover.num_outputs(), 1);
    ///
    /// // Add another expression as a second output
    /// let expr2 = b.or(&a);
    /// cover.add_expr(&expr2, "output2").unwrap();
    /// ```
    pub fn add_expr(&mut self, expr: &BoolExpr, output_name: &str) -> Result<(), AddExprError> {
        // Check if output already exists (fail fast before doing any work). Even an unlabeled cover
        // with outputs has implicit `y0, y1, …` names (its header), which a named output collides
        // with — matching the pre-existing backfill-then-check behaviour.
        if self.output_vars.iter().any(|v| v.as_ref() == output_name) {
            return Err(CoverError::OutputAlreadyExists {
                name: Arc::from(output_name),
            }
            .into());
        }

        // Extract the expression's product terms as input minterms (goes through the BDD for
        // canonical form). Every minterm shares one header: the expression's variables, sorted.
        let cubes = expr.to_cubes();
        let expr_vars: &[Arc<str>] = cubes.first().map(|m| m.vars()).unwrap_or(&[]);

        // Determine which of the expression's variables are new to the cover's input header.
        let new_inputs: Vec<Arc<str>> = expr_vars
            .iter()
            .filter(|v| !self.input_vars.iter().any(|x| x.as_ref() == v.as_ref()))
            .cloned()
            .collect();

        // Extend the input header with new variables and re-point existing cubes (new = don't-care).
        if !new_inputs.is_empty() {
            let mut header: Vec<Arc<str>> = self.input_vars.to_vec();
            header.extend(new_inputs.iter().cloned());
            let header: Arc<[Arc<str>]> = header.into();
            for cube in &mut self.cubes {
                cube.inputs = cube.inputs.project_onto(&header);
            }
            self.input_vars = header;
        }
        if !expr_vars.is_empty() {
            self.input_labeled = true;
        }

        // Add the new output to the output header; existing cubes don't assert it.
        let output_index = self.num_outputs();
        let mut out_header: Vec<Arc<str>> = self.output_vars.to_vec();
        out_header.push(Arc::from(output_name));
        let out_header: Arc<[Arc<str>]> = out_header.into();
        for cube in &mut self.cubes {
            let mut mask: Vec<Option<bool>> = cube.outputs.iter().collect();
            mask.resize(out_header.len(), Some(false));
            cube.outputs = Minterm::from_values(Arc::clone(&out_header), mask);
        }
        self.output_vars = out_header;
        self.output_labeled = true;

        // Add an F cube per product term, asserting only the new output. Each product-term minterm
        // carries its own variable names, so map them onto the cover's input header by name.
        for product_term in &cubes {
            let mut inputs = vec![None; self.num_inputs()];
            for (var, polarity) in product_term.vars().iter().zip(product_term.iter()) {
                if let Some(value) = polarity {
                    if let Some(idx) = self
                        .input_vars
                        .iter()
                        .position(|x| x.as_ref() == var.as_ref())
                    {
                        inputs[idx] = Some(value);
                    }
                }
            }

            let mut mask = vec![false; self.num_outputs()];
            mask[output_index] = true;

            let im = self.input_minterm(&inputs);
            let om =
                Minterm::from_values(Arc::clone(&self.output_vars), mask.iter().map(|&b| Some(b)));
            self.cubes.push(Cube::new(im, om, CubeType::F));
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
        let output_idx = self
            .output_labels()
            .iter()
            .position(|v| v.as_ref() == output_name)
            .ok_or_else(|| CoverError::OutputNotFound {
                name: Arc::from(output_name),
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
        if output_idx >= self.num_outputs() {
            return Err(CoverError::OutputIndexOutOfBounds {
                index: output_idx,
                max: self.num_outputs().saturating_sub(1),
            }
            .into());
        }

        // Only F cubes that assert this output contribute to the expression.
        let relevant_cubes: Vec<&Cube> = self
            .cubes
            .iter()
            .filter(|cube| cube.cube_type() == CubeType::F && cube.asserts(output_idx))
            .collect();

        Ok(cubes_to_expr(
            &relevant_cubes,
            self.input_vars(),
            self.num_inputs(),
        ))
    }
}

/// Convert cube references back to a boolean expression
///
/// If `variables` is empty or shorter than `num_inputs`, generates default variable names (x0, x1, ...).
pub(super) fn cubes_to_expr(
    cubes: &[&Cube],
    variables: &[Arc<str>],
    num_inputs: usize,
) -> BoolExpr {
    use std::collections::BTreeMap;

    if cubes.is_empty() {
        return BoolExpr::constant(false);
    }

    // Build product terms directly for factorization
    let mut product_terms = Vec::new();

    for cube in cubes.iter() {
        // Build product term as a map of literals
        let mut literals = BTreeMap::new();

        for i in 0..num_inputs {
            // Get variable name - use provided label or generate default
            let var_name: Arc<str> = if i < variables.len() {
                Arc::clone(&variables[i])
            } else {
                Arc::from(format!("x{}", i).as_str())
            };

            match cube.inputs().value_at(i) {
                Some(true) => {
                    // Positive literal
                    literals.insert(var_name, true);
                }
                Some(false) => {
                    // Negative literal
                    literals.insert(var_name, false);
                }
                None => {
                    // Don't care - skip this variable
                }
            }
        }

        // Add this product term (include=true for all cubes we process)
        product_terms.push((literals, true));
    }

    // Apply algebraic factorization to produce more compact multi-level logic
    crate::expression::factorization::factorise_cubes(product_terms)
}
