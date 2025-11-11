//! Expression integration for Cover
//!
//! This module provides methods for converting between covers and boolean expressions,
//! allowing seamless integration with the expression API.

use super::cubes::{Cube, CubeType};
use super::dnf::Dnf;
use super::error::{AddExprError, CoverError, ToExprError};
use super::iterators::ToExprs;
use super::Cover;
use crate::expression::BoolExpr;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

impl Cover {
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
                name: Arc::from(output_name),
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
                cube.cube_type() == CubeType::F
                    && output_idx < cube.outputs().len()
                    && cube.outputs()[output_idx]
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

            match cube.inputs().get(i) {
                Some(Some(true)) => {
                    // Positive literal
                    literals.insert(var_name, true);
                }
                Some(Some(false)) => {
                    // Negative literal
                    literals.insert(var_name, false);
                }
                Some(None) | None => {
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
