//! The [`Cube`] product term: a tri-state input [`Minterm`] + a binary output [`OutputSet`].
//!
//! A cube is one product term of a cover. It belongs to exactly one of the cover's three sets
//! (ON/`F`, don't-care/`D`, OFF/`R`) — recorded by its [`CubeType`] — and stores:
//!
//! - `inputs`: the input pattern minterm (`Some(true)`/`Some(false)`/`None` per input variable);
//! - `outputs`: a binary membership bitmap ([`OutputSet`]) — one bit per output: set means "this cube
//!   asserts the output (in its set)", clear means "not asserted".
//!
//! Keeping the per-cube [`CubeType`] (rather than merging all three into one tri-state output) is
//! what makes the representation **lossless**: the PLA `~` ("not asserted") state stays distinct
//! from the `-` (don't-care) state, so multi-output FD/FDR covers round-trip and minimise
//! byte-identically to the C library.

use super::label::{Anonymous, Label};
use super::minterm::Minterm;
use super::output_set::OutputSet;
use super::symbols::Symbols;
use std::fmt;

/// Which of a cover's three sets a cube belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CubeType {
    /// ON-set (the function is 1).
    #[default]
    F,
    /// Don't-care set (the function is `-`).
    D,
    /// OFF-set (the function is 0).
    R,
}

/// A cube (product term) in a cover: an input pattern, an output-membership mask, and a set tag.
///
/// Generic over the input label type `I` and the output label type `O`, so a cover can have, e.g.,
/// labelled inputs and an anonymous output.
#[derive(Clone)]
pub struct Cube<I, O> {
    pub(crate) inputs: Minterm<I>,
    /// Output-membership bitmap: bit set where this cube asserts the output (in its set).
    pub(crate) outputs: OutputSet<O>,
    pub(crate) set: CubeType,
}

impl<I: Label + fmt::Debug, O> fmt::Debug for Cube<I, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cube")
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .field("set", &self.set)
            .finish()
    }
}

/// Renders the cube as a PLA-style row — `<inputs> <outputs>` (the inputs a bare `1`/`0`/`-` string
/// from [`Minterm`]'s `Display`, the outputs a bare `1`/`0` string from [`OutputSet`]'s) — annotating
/// the set tag for don't-care/off-set cubes (`F` is the default and left unmarked). Needs no bound on
/// the label types.
impl<I, O> fmt::Display for Cube<I, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.inputs, self.outputs)?;
        match self.set {
            CubeType::F => Ok(()),
            CubeType::D => write!(f, " (D)"),
            CubeType::R => write!(f, " (R)"),
        }
    }
}

/// Two cubes are equal when they belong to the same set, their input patterns are equal, and their
/// output bitmaps are equal. Inputs compare by [`Minterm`]'s identity-based equality (aligning by
/// variable name, absent variables as don't-cares); outputs compare positionally by [`OutputSet`].
impl<I: Label, O> PartialEq for Cube<I, O> {
    fn eq(&self, other: &Self) -> bool {
        self.set == other.set && self.inputs == other.inputs && self.outputs == other.outputs
    }
}

impl<I: Label, O> Eq for Cube<I, O> {}

/// Hashes the same fields the [`PartialEq`] impl compares (set tag + input minterm + output bitmap),
/// keeping the `Hash`/`Eq` contract so a `Cube` can key a `HashMap`/`HashSet`.
impl<I: Label, O> std::hash::Hash for Cube<I, O> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.set.hash(state);
        self.inputs.hash(state);
        self.outputs.hash(state);
    }
}

impl<I, O> Cube<I, O> {
    /// Build a cube from its input pattern, output-membership bitmap, and set tag.
    pub(crate) fn new(inputs: Minterm<I>, outputs: OutputSet<O>, set: CubeType) -> Self {
        Cube {
            inputs,
            outputs,
            set,
        }
    }

    /// The input pattern of this cube.
    #[must_use]
    pub fn inputs(&self) -> &Minterm<I> {
        &self.inputs
    }

    /// The output-membership bitmap of this cube (a set bit where the output is asserted).
    ///
    /// This is a per-cube, per-set membership bitmap, **not** a merged tri-state output: an asserted
    /// bit means "this cube asserts the output in its own set ([`cube_type`](Self::cube_type))". For an
    /// FR/FDR cover, a given input pattern can therefore appear in more than one cube (e.g. an F cube
    /// and an R cube), each with its own bitmap.
    #[must_use]
    pub fn outputs(&self) -> &OutputSet<O> {
        &self.outputs
    }

    /// Which set (F/D/R) this cube belongs to.
    #[must_use]
    pub fn cube_type(&self) -> CubeType {
        self.set
    }

    /// Whether output `i` is asserted by this cube.
    pub(crate) fn asserts(&self, i: usize) -> bool {
        self.outputs.value_at(i)
    }
}

impl Cube<Anonymous, Anonymous> {
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
    #[must_use]
    pub fn anonymous(
        inputs: &[Option<bool>],
        membership: &[bool],
        set: CubeType,
    ) -> Cube<Anonymous, Anonymous> {
        let im = Minterm::from_symbols(
            Symbols::<Anonymous>::anonymous(inputs.len()),
            inputs.iter().copied(),
        );
        let om = OutputSet::anonymous(membership);
        Cube::new(im, om, set)
    }
}
