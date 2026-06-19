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

use std::collections::HashSet;
use std::hash::Hash;

/// How a cover's variable labels align across differently-ordered headers.
///
/// Implemented for every `Ord + Eq + Hash + Clone` type via a blanket impl (so `Symbol`,
/// [`Symbol`](crate::Symbol), `String`, `u32`, … all work as labels, aligning **by value**), and for
/// [`Anonymous`] (aligning **by position**).
pub trait Label: Clone {
    /// What makes two variables "the same" for alignment. `Ord` drives the merge-join that aligns two
    /// headers, `Hash` drives the O(1) reverse lookup, `Clone` lets the caches own a copy.
    type Identity: Ord + Hash + Clone;

    /// Whether this label type carries real **names** (`true`) or is purely positional (`false`, like
    /// [`Anonymous`]). A `Symbols<L>` always stores one label per position; this distinguishes a real
    /// name table from a placeholder `[Anonymous; n]` one, so the latter renders/exposes no names.
    const NAMED: bool = true;

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
/// `PartialEq`, `PartialOrd`, `Display`, `AsRef<str>`, or `From<&str>`. Those omissions are what keep
/// it out of the blanket impls of [`Label`] (above), [`ReconcilableLabel`], and [`PlaLabel`] — if it
/// satisfied any of those bounds the impls would overlap (E0119). They also encode the type-level
/// facts that an anonymous variable has no *name* (`!Display`) and no string form (`!AsRef<str>`). This
/// is deliberate, not an oversight.
///
/// [`PlaLabel`]: crate::PlaLabel
#[derive(Clone, Copy, Debug)]
pub struct Anonymous;

impl Label for Anonymous {
    type Identity = usize;
    const NAMED: bool = false;

    #[inline]
    fn identity(&self, position: usize) -> usize {
        position
    }
}

/// How a label type produces conflict-free labels for the columns [`Cover::extend`](crate::Cover::extend)
/// appends.
///
/// When `extend` stacks `b`'s columns after `a`'s, an appended label may clash with one already in the
/// header. This trait resolves that clash, per label type:
/// - **string-like** labels keep their name, suffixing a number on collision (`x` → `x0` → `x1`, …);
/// - **integer** labels keep their value, taking the first unused number on collision;
/// - [`Anonymous`] appends a fresh position (its identity is its index, so it never clashes).
///
/// `merge`, which overlays by identity rather than appending, needs only [`Label`]; `extend` requires
/// this. Label types with no sensible collision policy (an arbitrary struct key) get neither blanket
/// impl below and so cannot be `extend`ed — only `merge`d.
pub trait ReconcilableLabel: Label {
    /// Given the labels already in `header`, return one label per entry of `additions`, each distinct
    /// from the header and from the others returned (renaming/renumbering on collision).
    fn reconcile(header: &[Self], additions: &[Self]) -> Vec<Self>;
}

/// String-like labels reconcile by suffixing a number to a clashing name (`x` → `x0` → `x1`, …). The
/// construction bound (`From<&str>`) lives only on this impl; `String`, [`Symbol`](crate::Symbol),
/// `Box<str>`, `Cow<str>` all qualify — `Anonymous` implements neither `AsRef<str>` nor `From<&str>`,
/// so it is provably excluded (no overlap with the impls below).
impl<T: Label + AsRef<str> + for<'a> From<&'a str>> ReconcilableLabel for T {
    fn reconcile(header: &[Self], additions: &[Self]) -> Vec<Self> {
        let mut taken: HashSet<String> = header.iter().map(|l| l.as_ref().to_owned()).collect();
        let mut out = Vec::with_capacity(additions.len());
        for add in additions {
            let base = add.as_ref();
            let name = if taken.contains(base) {
                (0..)
                    .map(|n| format!("{base}{n}"))
                    .find(|cand| !taken.contains(cand))
                    .expect("an unbounded candidate range always yields a free name")
            } else {
                base.to_owned()
            };
            taken.insert(name.clone());
            out.push(T::from(name.as_str()));
        }
        out
    }
}

/// [`Anonymous`] reconciles by appending fresh positions — each appended `Anonymous` lands at a new
/// index whose identity is that index, so it is automatically distinct. Just clone the additions.
impl ReconcilableLabel for Anonymous {
    #[inline]
    fn reconcile(_header: &[Self], additions: &[Self]) -> Vec<Self> {
        additions.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol;

    fn syms(names: &[&str]) -> Vec<Symbol> {
        names.iter().map(|s| Symbol::from(*s)).collect()
    }

    #[test]
    fn string_reconcile_suffixes_on_collision() {
        assert_eq!(
            Symbol::reconcile(&syms(&["x"]), &syms(&["x"])),
            syms(&["x0"])
        );
        // `x0` already taken → next free suffix.
        assert_eq!(
            Symbol::reconcile(&syms(&["x", "x0"]), &syms(&["x"])),
            syms(&["x1"])
        );
        // Two identical additions collide with each other, not just the header.
        assert_eq!(
            Symbol::reconcile(&syms(&["x"]), &syms(&["x", "x"])),
            syms(&["x0", "x1"])
        );
    }

    #[test]
    fn string_reconcile_passes_non_colliding_through() {
        assert_eq!(
            Symbol::reconcile(&syms(&["x"]), &syms(&["y"])),
            syms(&["y"])
        );
    }

    #[test]
    fn anonymous_reconcile_appends() {
        // `Anonymous` has no `PartialEq`; only the count is observable.
        let out = Anonymous::reconcile(&[Anonymous, Anonymous], &[Anonymous]);
        assert_eq!(out.len(), 1);
    }
}
