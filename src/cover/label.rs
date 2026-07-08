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

/// Seals the label trait family ([`Label`], [`ReconcilableLabel`], [`PlaLabel`], [`StringLabel`]) so
/// they cannot be implemented for new types outside this crate. `Sealed` is implemented exactly where
/// `Label` is — for every `Ord + Eq + Hash + Clone` type and for [`Anonymous`] — so a type that
/// already qualifies as a label via the blanket impls is unaffected, but a foreign type that does not
/// can no longer hand-roll a `Label` (and thus a custom [`identity`](Label::identity)) impl.
pub(crate) mod sealed {
    /// Private supertrait that gates the label traits; see [`super::sealed`].
    pub trait Sealed {}
}

impl<T: Ord + Eq + Hash + Clone> sealed::Sealed for T {}
impl sealed::Sealed for Anonymous {}

/// How a cover's variable labels align across differently-ordered headers.
///
/// Implemented for every `Ord + Eq + Hash + Clone` type via a blanket impl (so
/// [`Symbol`](crate::Symbol), `String`, `u32`, … all work as labels, aligning **by value**), and for
/// [`Anonymous`] (aligning **by position**).
///
/// This trait is sealed (private `Sealed` supertrait): the blanket impls below are the only
/// implementations, so external code cannot supply its own [`identity`](Label::identity).
pub trait Label: Clone + sealed::Sealed {
    /// What makes two variables "the same" for alignment. `Ord` drives both the merge-join that aligns
    /// two headers and the binary-search reverse lookups; `Hash` drives the header-union map; `Clone`
    /// lets the tables own a copy.
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

/// A [`Label`] that is also a string: it has a borrowed `&str` view ([`AsRef<str>`]) and can be built
/// from one ([`From<&str>`]). This is the bound the string-oriented cover APIs share — building a
/// labelled cover from names ([`Cover::with_labels`](crate::Cover::with_labels)), reading a named
/// [`PlaCover`](crate::PlaCover), and string-collision reconciliation ([`ReconcilableLabel`]) — so it
/// is named once here instead of repeating the `Label + AsRef<str> + for<'a> From<&'a str>` cluster at
/// every site. `String`, [`Symbol`](crate::Symbol), `Arc<str>`, `Box<str>`, `Cow<str>` all qualify;
/// [`Anonymous`] does not (it is neither `AsRef<str>` nor `From<&str>`).
///
/// Sealed via its [`Label`] supertrait and provided by a single blanket impl, so it is an alias
/// callers name but never implement.
///
/// # Round-trip contract
///
/// An implementor's `From<&str>` must be **content-preserving** with respect to its `AsRef<str>`:
/// converting a `&str` and reading the label back must yield the same string (`S::from(name).as_ref()
/// == name`), so in particular two distinct names never collapse into one label. The BDD manager stores
/// variable names genuinely as its label type `S` and enforces this contract at variable **insertion**:
/// `BddManager::get_or_create_var` mints `S::from(name)` and panics if the round-trip alters the name,
/// so a lossy `From` (e.g. one that case-folds) fails deterministically at the first insertion rather
/// than silently keying two distinct variable columns to the same label.
pub trait StringLabel: Label + AsRef<str> + for<'a> From<&'a str> {}

impl<T: Label + AsRef<str> + for<'a> From<&'a str>> StringLabel for T {}

/// A [`Label`] whose alignment identity is the label value itself — a real name/value rather than a
/// position. Every `Ord + Eq + Hash + Clone` label qualifies via the single blanket impl below
/// ([`Symbol`](crate::Symbol), `String`, `Arc<str>`, `u32`, …); [`Anonymous`] does not, since its
/// identity is its position rather than a value.
///
/// This is the bound taken by variable-set arguments that name variables by value, e.g.
/// `Minterm::project_to_labels` and `Cover::over_labels`.
///
/// Where [`Label::NAMED`] is a const consulted at runtime (to decide whether a label type renders
/// names), `NamedLabel` is a trait bound: it lets a signature require "a label that has a value
/// identity" at compile time, rather than checking `NAMED` after the fact.
///
/// Sealed via its [`Label`] supertrait and provided by a single blanket impl, so it is an alias
/// callers name but never implement.
pub trait NamedLabel: Label {}

impl<T: Ord + Eq + Hash + Clone> NamedLabel for T {}

/// How a label type produces conflict-free labels for the columns [`Cover::extend`](crate::Cover::extend)
/// appends.
///
/// When `extend` stacks `b`'s columns after `a`'s, an appended label may clash with one already in the
/// header. This trait resolves that clash, per label type:
/// - **string-like** labels keep their name, suffixing a number on collision (`x` → `x0` → `x1`, …);
/// - [`Anonymous`] appends a fresh position (its identity is its index, so it never clashes).
///
/// `merge`, which overlays by identity rather than appending, needs only [`Label`]; `extend` requires
/// this. Only the two impls below exist — integer (and other non-string) label types have no
/// `ReconcilableLabel` impl (an integer impl would overlap the string blanket), so a `Cover<u32, …>`
/// can be `merge`d but not `extend`ed.
pub trait ReconcilableLabel: Label {
    /// Given the labels already in `header`, return one label per entry of `additions`, each distinct
    /// from the header and from the others returned (renaming/renumbering on collision).
    fn reconcile(header: &[Self], additions: &[Self]) -> Vec<Self>;
}

/// String-like labels reconcile by suffixing a number to a clashing name (`x` → `x0` → `x1`, …). The
/// construction bound (`From<&str>`) lives only on this impl; `String`, [`Symbol`](crate::Symbol),
/// `Box<str>`, `Cow<str>` all qualify — `Anonymous` implements neither `AsRef<str>` nor `From<&str>`,
/// so it is provably excluded (no overlap with the impls below).
impl<T: StringLabel> ReconcilableLabel for T {
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

    fn is_named<L: NamedLabel>() {}

    #[test]
    fn named_label_covers_value_identity_labels() {
        is_named::<Symbol>();
        is_named::<String>();
        is_named::<u32>();
    }
}
