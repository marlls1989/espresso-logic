//! Expression integration for Cover
//!
//! This module provides methods for converting between covers and boolean expressions,
//! allowing seamless integration with the expression API.

use super::cubes::{Cube, CubeType};
use super::error::{AddExprError, CoverError, ToExprError};
use super::iterators::ToExprs;
use super::minterm::Minterm;
use super::symbols::Symbols;
use super::Cover;
use crate::expression::BoolExpr;
use crate::Symbol;
use std::sync::Arc;

impl Cover<Symbol, Symbol> {
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
        // `add_expr` is a *labelled* operation, intended for empty or fully-labelled covers. Build
        // positional covers as `Cover<Anonymous, Anonymous>` and convert explicitly (`relabel`) rather than naming
        // anonymous positions here.

        // Check if output already exists (fail fast before doing any work).
        if self
            .output_symbols
            .labels()
            .iter()
            .any(|v| v.as_ref() == output_name)
        {
            return Err(CoverError::OutputAlreadyExists {
                name: Symbol::from(output_name),
            }
            .into());
        }

        // Extract the expression's product terms as input minterms (goes through the BDD for
        // canonical form). Every minterm shares one header: the expression's variables, sorted.
        let cubes = expr.to_cubes();
        let expr_vars: &[Symbol] = cubes.first().map(|m| m.vars()).unwrap_or(&[]);

        // Extend the input table with any variables new to the cover, re-pointing existing cubes
        // (new inputs = don't-care). Build the grown header as one chained iterator.
        let is_new = |v: &Symbol| {
            !self
                .input_symbols
                .labels()
                .iter()
                .any(|x| x.as_ref() == v.as_ref())
        };
        if expr_vars.iter().any(is_new) {
            let header: Arc<[Symbol]> = self
                .input_symbols
                .labels()
                .iter()
                .cloned()
                .chain(expr_vars.iter().filter(|v| is_new(v)).cloned())
                .collect();
            let new_syms = Symbols::new(header);
            for cube in &mut self.cubes {
                cube.inputs = cube.inputs.project_onto(&new_syms);
            }
            self.input_symbols = new_syms;
        }

        // Append the new output to the output table; existing cubes gain an unasserted column.
        let output_index = self.num_outputs();
        let out_header: Arc<[Symbol]> = self
            .output_symbols
            .labels()
            .iter()
            .cloned()
            .chain(std::iter::once(Symbol::from(output_name)))
            .collect();
        let out_syms = Symbols::new(out_header);
        for cube in &mut self.cubes {
            cube.outputs = Minterm::from_symbols(
                Arc::clone(&out_syms),
                cube.outputs.iter().chain(std::iter::once(Some(false))),
            );
        }
        self.output_symbols = out_syms;

        // Add an F cube per product term, asserting only the new output. Each product-term minterm
        // carries its own names, so read the input pattern positionally off the cover table by name.
        let input_symbols = Arc::clone(&self.input_symbols);
        let output_symbols = Arc::clone(&self.output_symbols);
        let no = self.num_outputs();
        self.cubes.extend(cubes.iter().map(|product_term| {
            let im = Minterm::from_symbols(
                Arc::clone(&input_symbols),
                input_symbols
                    .labels()
                    .iter()
                    .map(|name| product_term.value_of(name)),
            );
            let om = Minterm::from_symbols(
                Arc::clone(&output_symbols),
                (0..no).map(|i| Some(i == output_index)),
            );
            Cube::new(im, om, CubeType::F)
        }));

        Ok(())
    }
}

/// Rebuilding an expression depends only on the **input** variable names, so these conversions work
/// for any string-like input label `I` whatever the output label type `O` is — including an
/// anonymous-output cover from a `BoolExpr` (`Cover<Symbol, Anonymous>`).
impl<I: AsRef<str>, O> Cover<I, O> {
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
        let relevant_cubes = self
            .cubes
            .iter()
            .filter(|cube| cube.cube_type() == CubeType::F && cube.asserts(output_idx));

        Ok(cubes_to_expr(
            relevant_cubes,
            self.input_symbols().labels(),
            self.num_inputs(),
        ))
    }

    /// Convert every output to a boolean expression.
    ///
    /// Yields `(output_label, expression)` for each output — the output label borrowed from the cover
    /// (`&O`), paired with the expression rebuilt from the input names. For an anonymous-output cover
    /// the label is uninformative; use [`to_expr_by_index`](Self::to_expr_by_index) there instead.
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
    pub fn to_exprs(&self) -> ToExprs<'_, I, O> {
        ToExprs {
            cover: self,
            current_idx: 0,
        }
    }
}

/// Looking an output up by name additionally needs a string-like **output** label.
impl<I: AsRef<str>, O: AsRef<str>> Cover<I, O> {
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
            .output_symbols()
            .labels()
            .iter()
            .position(|v| v.as_ref() == output_name)
            .ok_or_else(|| CoverError::OutputNotFound {
                name: Symbol::from(output_name),
            })?;

        self.to_expr_by_index(output_idx)
    }
}

/// Convert cubes back to a boolean expression.
///
/// Reads the cubes' input pattern against the input variable names (any `I: AsRef<str>`); if
/// `variables` is empty or shorter than `num_inputs`, generates default names (x0, x1, ...).
pub(super) fn cubes_to_expr<'a, I: AsRef<str> + 'a, O: 'a>(
    cubes: impl IntoIterator<Item = &'a Cube<I, O>>,
    variables: &[I],
    num_inputs: usize,
) -> BoolExpr {
    use std::collections::BTreeMap;

    // Each cube becomes a product term (a `name -> polarity` literal map) for the factoriser, which
    // requires an owned collection it can scan repeatedly. Input labels are interned into `Symbol`s
    // (the expression layer's name type) at this boundary.
    let var_name = |i: usize| -> Symbol {
        variables
            .get(i)
            .map(|v| Symbol::from(v.as_ref()))
            .unwrap_or_else(|| Symbol::from(format!("x{i}").as_str()))
    };
    let product_terms: Vec<(BTreeMap<Symbol, bool>, bool)> = cubes
        .into_iter()
        .map(|cube| {
            let literals = (0..num_inputs)
                .filter_map(|i| {
                    cube.inputs()
                        .value_at(i)
                        .map(|polarity| (var_name(i), polarity))
                })
                .collect();
            (literals, true)
        })
        .collect();

    if product_terms.is_empty() {
        return BoolExpr::constant(false);
    }

    // Apply algebraic factorization to produce more compact multi-level logic
    crate::expression::factorization::factorise_cubes(product_terms)
}
