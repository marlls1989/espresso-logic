//! Expression integration for Cover
//!
//! This module provides methods for converting between covers and boolean expressions,
//! allowing integration with the expression API.

use super::cubes::{Cube, CubeType};
use super::error::{AddExprError, CoverError, ToExprError};
use super::iterators::ToExprs;
use super::minterm::Minterm;
use super::output_set::OutputSet;
use super::symbols::Symbols;
use super::Cover;
use crate::expression::BoolExpr;
use crate::Symbol;
use std::sync::Arc;

impl Cover<Symbol, Symbol> {
    /// Add a [`Bdd`](crate::bdd::Bdd)'s ON-set to a named output.
    ///
    /// This is the primitive Boolean-function → cover bridge: the handle's product terms
    /// ([`Bdd::to_cubes`](crate::bdd::Bdd::to_cubes)) become F cubes asserting `output_name`. Input
    /// variables are matched by name with existing variables, and new variables are appended.
    ///
    /// Returns an error if the output name already exists (to prevent accidental overwrite).
    pub fn add_bdd<S: AsRef<str>, B: crate::bdd::Brand>(
        &mut self,
        bdd: &crate::bdd::Bdd<'_, B>,
        output_name: S,
    ) -> Result<(), AddExprError> {
        let output_name = output_name.as_ref();
        // `add_bdd` is a *labelled* operation, intended for empty or fully-labelled covers. Build
        // positional covers as `Cover<Anonymous, Anonymous>` and convert explicitly (`relabel`) rather
        // than naming anonymous positions here.

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

        // Extract the function's product terms as input minterms (canonical ON-set from the BDD).
        // Every minterm shares one header: the function's variables, sorted.
        let on_set = bdd.to_cubes();
        let cubes: Vec<Minterm<Symbol>> = on_set.cubes().map(|c| c.inputs().clone()).collect();

        // Union the cover's input header with the expression's variables, re-pointing existing cubes
        // onto the grown header (new inputs = don't-care). Reuse the shared identity-union helper
        // rather than re-implementing union-by-name (for `Symbol`, identity *is* the name).
        if let Some(expr_syms) = cubes.first().map(|m| m.symbols()) {
            let (new_syms, _, _) = super::identity_union(&self.input_symbols, expr_syms);
            if new_syms.arity() > self.num_inputs() {
                for cube in &mut self.cubes {
                    cube.inputs = cube.inputs.project_onto(&new_syms);
                }
                self.input_symbols = new_syms;
            }
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
            cube.outputs = OutputSet::from_symbols(
                Arc::clone(&out_syms),
                cube.outputs.iter().chain(std::iter::once(false)),
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
            let om = OutputSet::from_symbols(
                Arc::clone(&output_symbols),
                (0..no).map(|i| i == output_index),
            );
            Cube::new(im, om, CubeType::F)
        }));

        Ok(())
    }

    /// Add a boolean function to a named output.
    ///
    /// A convenience wrapper over [`add_bdd`](Self::add_bdd): the owned, syntactic [`BoolExpr`] has no
    /// cubes of its own, so it is first built into a [`Bdd`](crate::bdd::Bdd) in a private, temporary
    /// single-threaded context (which canonicalises it), then its ON-set is added as a new output. The
    /// temporary context lives only for this call; the handle is consumed before it returns.
    ///
    /// This is the bridge *from* the `Symbol`-based expression layer, so it produces the natural
    /// `Cover<Symbol, Symbol>`. To carry the result under a different string label type, build it here
    /// and [`relabel`](Cover::relabel) (or `relabel_inputs`/`relabel_outputs`).
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
    pub fn add_expr<S: AsRef<str>>(
        &mut self,
        expr: &BoolExpr,
        output_name: S,
    ) -> Result<(), AddExprError> {
        // Mediate the syntactic → cube transformation through a throwaway BDD context (canonicalises
        // the expression). The context is local; the handle borrows it and is consumed by `add_bdd`
        // before this function returns, so the lifetimes work out.
        let ctx = crate::bdd_builder!();
        let bdd = ctx.build(expr);
        self.add_bdd(&bdd, output_name)
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

        Ok(cubes_to_expr(relevant_cubes, self.input_symbols().labels()))
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
    pub fn to_expr<S: AsRef<str>>(&self, output_name: S) -> Result<BoolExpr, ToExprError> {
        let output_name = output_name.as_ref();
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
/// Reads each cube's input pattern against the input variable names (`variables`, one label per input
/// — the caller always passes the cover's full input header).
pub(super) fn cubes_to_expr<'a, I: AsRef<str> + 'a, O: 'a>(
    cubes: impl IntoIterator<Item = &'a Cube<I, O>>,
    variables: &[I],
) -> BoolExpr {
    use std::collections::BTreeMap;

    // Each cube becomes a product term (a `name -> polarity` literal map) for the factoriser, which
    // requires an owned collection it can scan repeatedly. Input labels are interned into `Symbol`s
    // (the expression layer's name type) at this boundary.
    let product_terms: Vec<(BTreeMap<Symbol, bool>, bool)> = cubes
        .into_iter()
        .map(|cube| {
            let literals = (0..variables.len())
                .filter_map(|i| {
                    cube.inputs()
                        .value_at(i)
                        .map(|polarity| (Symbol::from(variables[i].as_ref()), polarity))
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
