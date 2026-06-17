//! The [`Cube`] product term: input pattern + output-membership, both [`Minterm`]s.
//!
//! A cube is one product term of a cover. It belongs to exactly one of the cover's three sets
//! (ON/`F`, don't-care/`D`, OFF/`R`) — recorded by its [`CubeType`] — and stores:
//!
//! - `inputs`: the input pattern minterm (`Some(true)`/`Some(false)`/`None` per input variable);
//! - `outputs`: a membership minterm where `Some(true)` means "this output is asserted by this
//!   cube (in its set)" and `Some(false)` means "not asserted".
//!
//! Keeping the per-cube [`CubeType`] (rather than merging all three into one tri-state output) is
//! what makes the representation **lossless**: the PLA `~` ("not asserted") state stays distinct
//! from the `-` (don't-care) state, so multi-output FD/FDR covers round-trip and minimise
//! byte-identically to the C library.

use super::minterm::Minterm;
use super::symbols::Symbols;
use std::fmt;
use std::sync::Arc;

/// Which of a cover's three sets a cube belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubeType {
    /// ON-set (the function is 1).
    F,
    /// Don't-care set (the function is `-`).
    D,
    /// OFF-set (the function is 0).
    R,
}

/// A cube (product term) in a cover: an input pattern, an output-membership mask, and a set tag.
///
/// Generic over the input label type `I` and the output label type `O` (both default `Arc<str>`),
/// so a cover can have, e.g., labelled inputs and an anonymous output.
#[derive(Clone)]
pub struct Cube<I = Arc<str>, O = Arc<str>> {
    pub(crate) inputs: Minterm<I>,
    /// Membership mask: `Some(true)` where this cube asserts the output, `Some(false)` otherwise.
    pub(crate) outputs: Minterm<O>,
    pub(crate) set: CubeType,
}

impl<I: fmt::Debug, O: fmt::Debug> fmt::Debug for Cube<I, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cube")
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .field("set", &self.set)
            .finish()
    }
}

impl<I, O> Cube<I, O> {
    /// Build a cube from its input pattern, output-membership mask, and set tag.
    pub(crate) fn new(inputs: Minterm<I>, outputs: Minterm<O>, set: CubeType) -> Self {
        Cube {
            inputs,
            outputs,
            set,
        }
    }

    /// The input pattern of this cube.
    pub fn inputs(&self) -> &Minterm<I> {
        &self.inputs
    }

    /// The output-membership mask of this cube (`Some(true)` where the output is asserted).
    ///
    /// This is a per-cube, per-set membership mask, **not** a merged tri-state output: `Some(true)`
    /// means "this cube asserts the output in its own set ([`cube_type`](Self::cube_type))" and
    /// `Some(false)` means "it does not". For an FR/FDR cover, a given input pattern can therefore
    /// appear in more than one cube (e.g. an F cube and an R cube), each with its own mask.
    pub fn outputs(&self) -> &Minterm<O> {
        &self.outputs
    }

    /// Which set (F/D/R) this cube belongs to.
    pub fn cube_type(&self) -> CubeType {
        self.set
    }

    /// Whether output `i` is asserted by this cube.
    pub(crate) fn asserts(&self, i: usize) -> bool {
        self.outputs.value_at(i) == Some(true)
    }
}

impl Cube<(), ()> {
    /// Build an **anonymous** (positional) cube from an input pattern and an output-membership mask.
    ///
    /// - `inputs[i]` is the value of input `i`: `Some(true)`/`Some(false)`, or `None` for don't-care.
    /// - `membership[j]` is whether this cube asserts output `j` **in its own set** (`set`): an `F`
    ///   cube asserts the ON-set outputs, a `D` cube the don't-care outputs, an `R` cube the OFF-set
    ///   outputs.
    ///
    /// The cube carries its own anonymous symbol tables; [`Cover::from_cubes`](crate::Cover::from_cubes)
    /// and [`Cover::push`](crate::Cover::push) re-point it onto the cover's shared tables.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cube, CubeType};
    ///
    /// // 01 -> output 0 asserted, in the ON-set.
    /// let cube = Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F);
    /// assert_eq!(cube.cube_type(), CubeType::F);
    /// ```
    pub fn anonymous(inputs: &[Option<bool>], membership: &[bool], set: CubeType) -> Cube<(), ()> {
        let im = Minterm::from_symbols(Symbols::anonymous(inputs.len()), inputs.iter().copied());
        let om = Minterm::from_symbols(
            Symbols::anonymous(membership.len()),
            membership.iter().map(|&b| Some(b)),
        );
        Cube::new(im, om, set)
    }
}
