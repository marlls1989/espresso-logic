//! Trait implementations for Cover
//!
//! This module provides conversions and trait implementations for [`Cover`],
//! including PLA I/O, Debug formatting, and conversions from expressions.

use super::cubes::{Cube, CubeType};
use super::minterm::Minterm;
use super::CoverType;
use super::{extend_header, Cover};
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

/// Raw parsed cube data handed to [`PLASerialisable::create_from_pla_parts`]:
/// `(input pattern, output-membership mask, set)`.
pub(crate) type RawCube = (Vec<Option<bool>>, Vec<bool>, CubeType);

// Implement PLASerialisable for Cover (used for PLA I/O)
impl super::pla::PLASerialisable for Cover {
    type CubesIter<'a> = std::slice::Iter<'a, Cube>;

    fn num_inputs(&self) -> usize {
        self.input_vars().len()
    }

    fn num_outputs(&self) -> usize {
        self.output_vars().len()
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

        let cubes = cubes
            .into_iter()
            .map(|(mut inputs, mask, set)| {
                inputs.resize(num_inputs, None);
                let im = Minterm::from_values(Arc::clone(&input_vars), inputs);
                let om =
                    Minterm::from_values(Arc::clone(&output_vars), mask.iter().map(|&b| Some(b)));
                Cube::new(im, om, set)
            })
            .collect();

        Cover {
            input_vars,
            output_vars,
            input_labeled,
            output_labeled,
            cubes,
            cover_type,
        }
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

/// Convert a `&BoolExpr` into a `Cover` with a single output named "out"
///
/// This conversion extracts the cubes from the internal BDD representation without
/// requiring ownership of the expression.
///
/// # Examples
///
/// ```
/// use espresso_logic::{BoolExpr, Cover};
///
/// let a = BoolExpr::variable("a");
/// // Note: BoolExpr uses BDD internally (v3.1.1+)
///
/// let cover = Cover::from(&a);
/// assert_eq!(cover.num_outputs(), 1);
/// ```
impl From<&crate::expression::BoolExpr> for Cover {
    fn from(expr: &crate::expression::BoolExpr) -> Self {
        // Convert to DNF
        let dnf = crate::cover::dnf::Dnf::from(expr);
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
                if let Some(idx) = var_vec.iter().position(|v| v == var) {
                    inputs[idx] = Some(polarity);
                }
            }

            cover.add_cube(&inputs, &[Some(true)]);
        }

        cover
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
