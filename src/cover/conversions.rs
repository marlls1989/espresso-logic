//! Trait implementations for Cover
//!
//! This module provides conversions and trait implementations for [`Cover`],
//! including PLA I/O, Debug formatting, and conversions from expressions.

use super::cubes::{Cube, CubeType};
use super::minterm::Minterm;
use super::symbols::Symbols;
use super::CoverType;
use super::{extend_header, Cover};
use std::fmt;
use std::sync::Arc;

/// Raw parsed cube data handed to [`PLASerialisable::create_from_pla_parts`]:
/// `(input pattern, output-membership mask, set)`.
pub(crate) type RawCube = (Vec<Option<bool>>, Vec<bool>, CubeType);

// Implement PLASerialisable for Cover (used for PLA I/O)
impl super::pla::PLASerialisable for Cover {
    type CubesIter<'a> = std::slice::Iter<'a, Cube>;

    fn num_inputs(&self) -> usize {
        self.input_symbols().arity()
    }

    fn num_outputs(&self) -> usize {
        self.output_symbols().arity()
    }

    fn internal_cubes_iter(&self) -> Self::CubesIter<'_> {
        self.cubes.iter()
    }

    fn get_input_labels(&self) -> Option<&[Arc<str>]> {
        let labels = self.input_labels();
        if labels.is_empty() {
            None
        } else {
            Some(labels)
        }
    }

    fn get_output_labels(&self) -> Option<&[Arc<str>]> {
        let labels = self.output_labels();
        if labels.is_empty() {
            None
        } else {
            Some(labels)
        }
    }

    fn create_from_pla_parts(
        num_inputs: usize,
        num_outputs: usize,
        input_labels: Vec<Arc<str>>,
        output_labels: Vec<Arc<str>>,
        cubes: Vec<RawCube>,
        cover_type: CoverType,
    ) -> Self {
        let input_labeled = !input_labels.is_empty();
        let output_labeled = !output_labels.is_empty();
        let input_vars: Arc<[Arc<str>]> = if input_labeled {
            input_labels.into()
        } else {
            extend_header(&[], num_inputs, 'x')
        };
        let output_vars: Arc<[Arc<str>]> = if output_labeled {
            output_labels.into()
        } else {
            extend_header(&[], num_outputs, 'y')
        };
        let input_symbols = Symbols::new(input_vars);
        let output_symbols = Symbols::new(output_vars);

        let cubes = cubes
            .into_iter()
            .map(|(mut inputs, mask, set)| {
                inputs.resize(num_inputs, None);
                let im = Minterm::from_symbols(Arc::clone(&input_symbols), inputs);
                let om = Minterm::from_symbols(
                    Arc::clone(&output_symbols),
                    mask.iter().map(|&b| Some(b)),
                );
                Cube::new(im, om, set)
            })
            .collect();

        Cover {
            input_symbols,
            output_symbols,
            input_labeled,
            output_labeled,
            cubes,
            cover_type,
        }
    }
}

/// Build a single-output [`Cover<Arc<str>, ()>`](Cover) from a Boolean expression.
///
/// Goes through the expression's internal **BDD** for efficiency: [`to_cubes`](crate::BoolExpr::to_cubes)
/// yields the product terms as input minterms over one shared, canonical header. Each becomes an F cube
/// asserting the cover's single output. The output is **anonymous** (`O = ()`) — an expression has no
/// output name; label it explicitly with [`relabel_outputs`](Cover::relabel_outputs) if needed.
fn cover_from_expr(expr: &crate::expression::BoolExpr) -> Cover<Arc<str>, ()> {
    let minterms = expr.to_cubes();
    let input_symbols = minterms
        .first()
        .map(|m| Arc::clone(m.symbols()))
        .unwrap_or_else(Symbols::empty);
    let output_symbols = Symbols::<()>::anonymous(1);
    let asserted = Minterm::from_symbols(Arc::clone(&output_symbols), [Some(true)]);
    let cubes = minterms
        .iter()
        .map(|m| Cube::new(m.clone(), asserted.clone(), CubeType::F))
        .collect();
    Cover {
        input_labeled: input_symbols.is_labeled(),
        output_labeled: false,
        input_symbols,
        output_symbols,
        cubes,
        cover_type: CoverType::F,
    }
}

/// Convert a `BoolExpr` into a single-output [`Cover<Arc<str>, ()>`](Cover) (anonymous output).
///
/// Uses the BDD representation for efficient product-term extraction.
///
/// # Examples
///
/// ```
/// use espresso_logic::{BoolExpr, Cover};
/// use std::sync::Arc;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = a.and(&b);
///
/// let cover: Cover<Arc<str>, ()> = expr.into();
/// assert_eq!(cover.num_outputs(), 1);
/// ```
impl From<crate::expression::BoolExpr> for Cover<Arc<str>, ()> {
    fn from(expr: crate::expression::BoolExpr) -> Self {
        cover_from_expr(&expr)
    }
}

/// Convert a `&BoolExpr` into a single-output [`Cover<Arc<str>, ()>`](Cover) (anonymous output).
///
/// Extracts the cubes from the internal BDD without requiring ownership of the expression.
///
/// # Examples
///
/// ```
/// use espresso_logic::{BoolExpr, Cover};
/// use std::sync::Arc;
///
/// let a = BoolExpr::variable("a");
///
/// let cover = Cover::<Arc<str>, ()>::from(&a);
/// assert_eq!(cover.num_outputs(), 1);
/// ```
impl From<&crate::expression::BoolExpr> for Cover<Arc<str>, ()> {
    fn from(expr: &crate::expression::BoolExpr) -> Self {
        cover_from_expr(expr)
    }
}

impl fmt::Debug for Cover {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cover")
            .field("num_inputs", &self.num_inputs())
            .field("num_outputs", &self.num_outputs())
            .field("cover_type", &self.cover_type)
            .field("num_cubes", &self.num_cubes())
            .field("input_labels", &self.input_labels())
            .field("output_labels", &self.output_labels())
            .finish()
    }
}
