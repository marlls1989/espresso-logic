//! Trait implementations for Cover
//!
//! This module provides conversions and trait implementations for [`Cover`],
//! including PLA I/O, Debug formatting, and conversions from expressions.

use super::cubes::{Cube, CubeType};
use super::labels::LabelManager;
use super::Cover;
use super::CoverType;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

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
