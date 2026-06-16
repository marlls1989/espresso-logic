//! The [`Cube`] product term: input pattern + output-membership, both [`Minterm`]s.
//!
//! A cube is one product term of a cover. It belongs to exactly one of the cover's three sets
//! (ON/`F`, don't-care/`D`, OFF/`R`) — recorded by an internal [`OutputSet`] tag — and stores:
//!
//! - `inputs`: the input pattern minterm (`Some(true)`/`Some(false)`/`None` per input variable);
//! - `outputs`: a membership minterm where `Some(true)` means "this output is asserted by this
//!   cube (in its set)" and `Some(false)` means "not asserted".
//!
//! Keeping the per-cube `set` tag (rather than merging all three into one tri-state output) is what
//! makes the representation **lossless**: the PLA `~` ("not asserted") state stays distinct from the
//! `-` (don't-care) state, so multi-output FD/FDR covers round-trip and minimise byte-identically to
//! the C library. The merged tri-state view is offered separately via
//! [`Cover::cubes_iter`](crate::Cover::cubes_iter).

use super::minterm::Minterm;

/// Owned `(inputs, outputs)` data in the merged tri-state form yielded by
/// [`Cover::cubes_iter`](crate::Cover::cubes_iter).
///
/// Outputs are merged: `Some(true)` = 1 (ON), `Some(false)` = 0 (OFF), `None` = don't-care.
pub type CubeData = (Vec<Option<bool>>, Vec<Option<bool>>);

/// Which of a cover's three sets a cube belongs to.
///
/// Internal: derived from / paired with cube storage when talking to the C library or writing PLA;
/// not part of the public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputSet {
    /// ON-set (the function is 1).
    F,
    /// Don't-care set (the function is `-`).
    D,
    /// OFF-set (the function is 0).
    R,
}

/// A cube (product term) in a cover.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cube {
    pub(crate) inputs: Minterm,
    /// Membership mask: `Some(true)` where this cube asserts the output, `Some(false)` otherwise.
    pub(crate) outputs: Minterm,
    pub(crate) set: OutputSet,
}

impl Cube {
    /// Build a cube from its input pattern, output-membership mask, and set tag.
    pub(crate) fn new(inputs: Minterm, outputs: Minterm, set: OutputSet) -> Self {
        Cube {
            inputs,
            outputs,
            set,
        }
    }

    /// The input pattern of this cube.
    pub fn inputs(&self) -> &Minterm {
        &self.inputs
    }

    /// The output-membership mask of this cube (`Some(true)` where the output is asserted).
    pub fn outputs(&self) -> &Minterm {
        &self.outputs
    }

    /// Which set (F/D/R) this cube belongs to.
    pub(crate) fn set(&self) -> OutputSet {
        self.set
    }

    /// Whether output `i` is asserted by this cube.
    pub(crate) fn asserts(&self, i: usize) -> bool {
        self.outputs.value_at(i) == Some(true)
    }
}
