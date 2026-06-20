//! Trait implementations for Cover
//!
//! This module provides conversions and trait implementations for [`Cover`],
//! including PLA I/O, Debug formatting, and conversions from expressions.

use super::cubes::{Cube, CubeType};
use super::label::Anonymous;
use super::minterm::Minterm;
use super::symbols::Symbols;
use super::Cover;
use super::CoverType;
use crate::Symbol;
use std::fmt;
use std::sync::Arc;

/// Raw parsed cube data from the PLA reader: `(input pattern, output-membership mask, set)`.
pub(crate) type RawCube = (Vec<Option<bool>>, Vec<bool>, CubeType);

/// Build a positional [`Cover<Anonymous, Anonymous>`](Cover) from raw parsed PLA cubes. The PLA reader
/// then relabels the present sides (`.ilb`/`.ob`) to select a [`PlaCover`](super::pla::PlaCover) variant
/// — there are no synthesised placeholder names, so an unlabelled side stays `Anonymous`.
pub(crate) fn anonymous_cover_from_raw(
    num_inputs: usize,
    num_outputs: usize,
    cubes: Vec<RawCube>,
    cover_type: CoverType,
) -> Cover<Anonymous, Anonymous> {
    let input_symbols = Symbols::<Anonymous>::anonymous(num_inputs);
    let output_symbols = Symbols::<Anonymous>::anonymous(num_outputs);

    let cubes = cubes
        .into_iter()
        .map(|(mut inputs, mask, set)| {
            inputs.resize(num_inputs, None);
            let im = Minterm::from_symbols(Arc::clone(&input_symbols), inputs);
            let om =
                Minterm::from_symbols(Arc::clone(&output_symbols), mask.iter().map(|&b| Some(b)));
            Cube::new(im, om, set)
        })
        .collect();

    Cover {
        input_symbols,
        output_symbols,
        cubes,
        cover_type,
    }
}

/// Build a single-output [`Cover<Symbol, Anonymous>`](Cover) from a Boolean expression.
///
/// Goes through the expression's internal **BDD** for efficiency: [`to_cubes`](crate::BoolExpr::to_cubes)
/// yields the product terms as input minterms over one shared, canonical header. Each becomes an F cube
/// asserting the cover's single output. The output is **anonymous** (`O = Anonymous`) — an expression has no
/// output name; label it explicitly with [`relabel_outputs`](Cover::relabel_outputs) if needed.
fn cover_from_expr(expr: &crate::expression::BoolExpr) -> Cover<Symbol, Anonymous> {
    let minterms = expr.to_cubes();
    let input_symbols = minterms
        .first()
        .map(|m| Arc::clone(m.symbols()))
        .unwrap_or_else(Symbols::empty);
    let output_symbols = Symbols::<Anonymous>::anonymous(1);
    let asserted = Minterm::from_symbols(Arc::clone(&output_symbols), [Some(true)]);
    let cubes = minterms
        .iter()
        .map(|m| Cube::new(m.clone(), asserted.clone(), CubeType::F))
        .collect();
    Cover {
        input_symbols,
        output_symbols,
        cubes,
        cover_type: CoverType::F,
    }
}

/// Convert a `BoolExpr` into a single-output [`Cover<Symbol, Anonymous>`](Cover) (anonymous output).
///
/// Uses the BDD representation for efficient product-term extraction.
///
/// # Examples
///
/// ```
/// use espresso_logic::{Symbol, Anonymous, BoolExpr, Cover};
/// use std::sync::Arc;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = a.and(&b);
///
/// let cover: Cover<Symbol, Anonymous> = expr.into();
/// assert_eq!(cover.num_outputs(), 1);
/// ```
impl From<crate::expression::BoolExpr> for Cover<Symbol, Anonymous> {
    fn from(expr: crate::expression::BoolExpr) -> Self {
        cover_from_expr(&expr)
    }
}

/// Convert a `&BoolExpr` into a single-output [`Cover<Symbol, Anonymous>`](Cover) (anonymous output).
///
/// Extracts the cubes from the internal BDD without requiring ownership of the expression.
///
/// # Examples
///
/// ```
/// use espresso_logic::{Symbol, Anonymous, BoolExpr, Cover};
/// use std::sync::Arc;
///
/// let a = BoolExpr::variable("a");
///
/// let cover = Cover::<Symbol, Anonymous>::from(&a);
/// assert_eq!(cover.num_outputs(), 1);
/// ```
impl From<&crate::expression::BoolExpr> for Cover<Symbol, Anonymous> {
    fn from(expr: &crate::expression::BoolExpr) -> Self {
        cover_from_expr(expr)
    }
}

impl<I: fmt::Debug, O: fmt::Debug> fmt::Debug for Cover<I, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cover")
            .field("num_inputs", &self.num_inputs())
            .field("num_outputs", &self.num_outputs())
            .field("cover_type", &self.cover_type)
            .field("num_cubes", &self.num_cubes())
            .field("input_labels", &self.input_symbols().labels())
            .field("output_labels", &self.output_symbols().labels())
            .finish()
    }
}

/// Renders the cover as its sum-of-products body: one [`Cube`] per line, in order, each a
/// PLA-style `<inputs> <outputs>` row. No `.i`/`.o` header is emitted — use the
/// [`PLAWriter`](crate::PLAWriter) for a complete PLA file. Needs no bound on the label types.
impl<I, O> fmt::Display for Cover<I, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, cube) in self.cubes().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{cube}")?;
        }
        Ok(())
    }
}
