//! Trait implementations for Cover
//!
//! This module provides conversions and trait implementations for [`Cover`],
//! including PLA I/O, Debug formatting, and conversions from expressions.

use super::cubes::{Cube, CubeType};
use super::label::Anonymous;
use super::minterm::{InputField, Minterm};
use super::output_set::OutputSet;
use super::symbols::Symbols;
use super::Cover;
use super::CoverType;
use std::fmt;
use std::sync::Arc;

/// Raw parsed cube data from the PLA reader: `(input fields, output-membership mask, set)`. The input
/// side uses [`InputField`] (not `Option<bool>`) so the empty literal (`?`) survives into the minterm.
pub(crate) type RawCube = (Vec<InputField>, Vec<bool>, CubeType);

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
            inputs.resize(num_inputs, InputField::DontCare);
            let im = Minterm::from_symbols_input_fields(Arc::clone(&input_symbols), inputs);
            let om = OutputSet::from_symbols(Arc::clone(&output_symbols), mask.iter().copied());
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
