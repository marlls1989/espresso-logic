//! The alignment contract for a cover's variable labels.
//!
//! A label type tells the symbol machinery how two variables — possibly sitting at different positions
//! in differently-ordered headers — are recognised as "the same variable": its
//! [`Identity`](Label::Identity). For real labels the identity *is* the label (position-independent:
//! variable `"a"` is `"a"` wherever it sits). For the dedicated anonymous label [`Anonymous`], the
//! identity is the variable's *position*, so anonymous covers align purely positionally.
//!
//! All alignment in the cover layer (projection, the merge-join comparison, `extend`/`merge`) is one
//! generic algorithm keyed on [`Identity`](Label::Identity); there is no per-concrete-type dispatch.

use std::hash::Hash;

/// How a cover's variable labels align across differently-ordered headers.
///
/// Implemented for every `Ord + Eq + Hash + Clone` type via a blanket impl (so `Arc<str>`,
/// [`Symbol`](crate::Symbol), `String`, `u32`, … all work as labels, aligning **by value**), and for
/// [`Anonymous`] (aligning **by position**).
pub trait Label: Clone {
    /// What makes two variables "the same" for alignment. `Ord` drives the merge-join that aligns two
    /// headers, `Hash` drives the O(1) reverse lookup, `Clone` lets the caches own a copy.
    type Identity: Ord + Hash + Clone;

    /// Form the alignment identity of *this* label sitting at `position`.
    ///
    /// The position is supplied by the symbol table so the label type never has to store it: real
    /// labels ignore it and return the label by value; [`Anonymous`] returns the position.
    #[doc(hidden)]
    fn identity(&self, position: usize) -> Self::Identity;
}

/// Any totally-ordered, hashable, cloneable label aligns **by value** — the same label is the same
/// variable in any header order, so the position is ignored.
impl<T: Ord + Eq + Hash + Clone> Label for T {
    type Identity = T;

    #[inline]
    fn identity(&self, _position: usize) -> T {
        self.clone()
    }
}

/// The anonymous (positional) variable label: a zero-sized type whose alignment identity is its
/// *position*. Used as the label type of a positional cover, e.g. `Cover<Anonymous, Anonymous>`.
///
/// **Invariant (load-bearing):** `Anonymous` must **never** implement `Ord`, `Eq`, `Hash`,
/// `PartialEq`, or `PartialOrd`. Those omissions are what keep it out of the blanket [`Label`] impl
/// above — if it satisfied that bound the two impls would overlap (E0119), and its position-based
/// identity would collapse all positions to "equal". This is deliberate, not an oversight.
#[derive(Clone, Copy, Debug)]
pub struct Anonymous;

impl Label for Anonymous {
    type Identity = usize;

    #[inline]
    fn identity(&self, position: usize) -> usize {
        position
    }
}
