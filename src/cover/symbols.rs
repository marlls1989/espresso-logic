//! Shared symbol table backing a cover's variable labels.
//!
//! A [`Symbols`] table describes a dense index range (`0..arity`) of variables, storing **one label
//! of type `L` per position** — there is no separate "anonymous" shape. A positional cover
//! uses the zero-sized [`Anonymous`] label (`Symbols<Anonymous>` holds `[Anonymous; arity]`, which
//! costs nothing); a named cover uses a real label type such as `Symbol`. The difference is carried
//! by the type, via [`Label::NAMED`], not by a runtime variant — so partial labelling is
//! unrepresentable (every position always has a label) and all alignment is one uniform path keyed on
//! [`Label::identity`].
//!
//! Every cube of a cover shares one `Arc<Symbols<L>>`, so aligning two cubes of the same cover is a
//! pointer-equality check ([`Arc::ptr_eq`]) and looking a variable up by name is O(log n) (binary
//! search over the identity-sorted order).

use super::error::DuplicateSymbol;
use super::label::{Anonymous, Label};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

/// The position of the first label whose [`identity`](Label::identity) repeats an earlier one, or
/// `None` if all are distinct. A symbol table's identities must be unique — a repeat would collapse
/// two columns onto one — so [`Symbols::new`] rejects it. [`Anonymous`]'s identity is its position,
/// so an anonymous header never reports a duplicate.
fn first_duplicate<L: Label>(labels: &[L]) -> Option<usize> {
    let mut seen = HashSet::new();
    labels
        .iter()
        .enumerate()
        .find(|&(i, l)| !seen.insert(l.identity(i)))
        .map(|(i, _)| i)
}

/// A cover's variable table: one label of type `L` per position (`0..arity`), plus the positions in
/// identity order for reverse lookups. Construct via [`Symbols::new`] (from a label list) or
/// [`Symbols::<Anonymous>::anonymous`] (positional); the table is **fully immutable** once built and
/// shared behind an `Arc`. There is no partial state and no interior mutability — `labels.len()` *is*
/// the arity, and the sorted order is computed eagerly at construction.
pub struct Symbols<L> {
    /// index → label, one per position. The single, authoritative copy of the labels.
    labels: Arc<[L]>,
    /// Positions `0..arity` sorted by [`identity`](Label::identity), computed once at construction.
    /// Drives the merge-join that aligns differing headers and the binary-search reverse lookups
    /// ([`index_of`](Symbols::index_of) / [`position_of_identity`](Symbols::position_of_identity)).
    ///
    /// Stored as a *permutation of positions* (`u32`), not a second label array: the identity ordering
    /// is read by indirection (`labels[sorted[k]]`), so the labels are never duplicated — which also
    /// keeps this cheap for heap labels like `String`/`Arc<str>`.
    sorted: Box<[u32]>,
}

/// Two symbol tables are equal when they describe the same variables in the same order — i.e. same
/// width and, position by position, the same variable [`identity`](Label::identity). For real labels
/// that is the label itself; two anonymous tables of equal width are equal (every position's identity
/// is its index). The `sorted` order is derived from the labels, so it follows automatically and is
/// not compared.
///
/// Because tables are shared behind an `Arc`, comparing `Arc<Symbols>` short-circuits on pointer
/// equality (the std `Arc: PartialEq` fast path for `T: Eq`) before falling back to this O(n) identity
/// comparison — which is still far cheaper than re-projecting a minterm onto a union.
impl<L: Label> PartialEq for Symbols<L> {
    fn eq(&self, other: &Self) -> bool {
        self.labels.len() == other.labels.len()
            && self
                .labels
                .iter()
                .enumerate()
                .all(|(i, la)| la.identity(i) == other.labels[i].identity(i))
    }
}

impl<L: Label> Eq for Symbols<L> {}

/// Hashes the per-position [`identity`](Label::identity) sequence — exactly what [`Eq`] compares — so
/// the `Hash`/`Eq` contract holds. The derived `sorted` cache is not hashed (it follows from `labels`).
impl<L: Label> std::hash::Hash for Symbols<L> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for (i, label) in self.labels.iter().enumerate() {
            label.identity(i).hash(state);
        }
        self.labels.len().hash(state);
    }
}

/// Renders the variable labels in index order. The `sorted` field is derived from the labels (the
/// [`Eq`] impl above ignores it too), so it is omitted to keep the debug output stable.
///
/// Gated on `L: Debug` (rather than `#[derive(Debug)]`) so the bound is required only when the table
/// is actually formatted — the type stays usable for label types that are not `Debug`.
impl<L: Label + fmt::Debug> fmt::Debug for Symbols<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Symbols")
            .field("labels", &self.labels)
            .finish_non_exhaustive()
    }
}

/// Cloning shares the label storage (`Arc<[L]>` is reference-counted) and copies the small `sorted`
/// permutation, so no `L: Clone` bound is needed. Tables are normally shared behind an `Arc`, but a
/// standalone `Symbols<L>` is `Clone` for the cases that hold one by value.
impl<L> Clone for Symbols<L> {
    fn clone(&self) -> Self {
        Symbols {
            labels: Arc::clone(&self.labels),
            sorted: self.sorted.clone(),
        }
    }
}

impl<L> Symbols<L> {
    /// Build a table whose labels are **already in [`identity`](Label::identity) order**, so the
    /// sorted order is just `0..arity` — no comparison sort, and no `L: Label` bound needed.
    ///
    /// The caller guarantees the ordering; used for the cases where it is known for free: an empty
    /// table and an [`anonymous`](Symbols::anonymous) one (whose identities are the positions). For an
    /// arbitrary label list use [`new`](Symbols::new), which sorts.
    pub(crate) fn from_identity_sorted(labels: Arc<[L]>) -> Arc<Symbols<L>> {
        let sorted = (0..labels.len() as u32).collect();
        Arc::new(Symbols { labels, sorted })
    }

    /// An empty symbol table (arity 0).
    #[must_use]
    pub fn empty() -> Arc<Symbols<L>> {
        Symbols::from_identity_sorted(Arc::from([]))
    }

    /// The number of variables (positions) this table describes.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.labels.len()
    }

    /// The variable labels in index order. Whether these are *names* is the label type's business —
    /// for a positional ([`Anonymous`]) table they are the zero-sized `Anonymous` placeholders, and a
    /// named table (e.g. `Symbols<Symbol>`) holds real names. There is no separate "is it labelled" flag.
    #[must_use]
    pub fn labels(&self) -> &[L] {
        &self.labels
    }
}

impl Symbols<Anonymous> {
    /// An anonymous (positional) symbol table of the given width: one [`Anonymous`] placeholder per
    /// position (a named table is built from real labels via [`new`](Self::new)).
    ///
    /// An anonymous label's identity is its position, so the labels are already in identity order and
    /// the sorted order is `0..arity` for free — no comparison sort.
    #[must_use]
    pub fn anonymous(arity: usize) -> Arc<Symbols<Anonymous>> {
        Symbols::from_identity_sorted((0..arity).map(|_| Anonymous).collect())
    }
}

impl<L: Label> Symbols<L> {
    /// Build a symbol table from an ordered list of labels (arity = `labels.len()`).
    ///
    /// Computes the identity-sorted order eagerly (one O(n log n) sort). When the labels are already
    /// in identity order, the crate-internal `from_identity_sorted` skips it.
    ///
    /// This is the single place a table is validated: a table's identities must be distinct (they key
    /// every alignment), so a repeated label is rejected here rather than silently collapsing two
    /// columns onto one. For an [`Anonymous`] header identity is position, so it never fails.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateSymbol`] if two labels share an identity; `index` is the second occurrence.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Symbols, Symbol};
    ///
    /// let ok = Symbols::new([Symbol::new("a"), Symbol::new("b")].into());
    /// assert!(ok.is_ok());
    ///
    /// let dup = Symbols::new([Symbol::new("a"), Symbol::new("a")].into());
    /// assert_eq!(dup.unwrap_err().index, 1);
    /// ```
    pub fn new(labels: Arc<[L]>) -> Result<Arc<Symbols<L>>, DuplicateSymbol> {
        if let Some(index) = first_duplicate(&labels) {
            return Err(DuplicateSymbol { index });
        }
        let mut order: Vec<u32> = (0..labels.len() as u32).collect();
        // Unstable sort: a header's identities are unique, so there are no equal keys for a stable
        // sort to order — and unstable (pdqsort) avoids the merge sort's temporary allocation
        // (measured ~15-20% faster for named tables).
        order.sort_unstable_by(|&x, &y| {
            labels[x as usize]
                .identity(x as usize)
                .cmp(&labels[y as usize].identity(y as usize))
        });
        Ok(Arc::new(Symbols {
            labels,
            sorted: order.into_boxed_slice(),
        }))
    }

    /// Build a table from labels that may repeat, keeping the first occurrence of each identity.
    ///
    /// This is the shared deduplication path behind the variable-*set* arguments of
    /// [`Cover::over_vars`](crate::Cover::over_vars) and [`Cube::expand_to`](crate::Cube::expand_to),
    /// where a repeated variable means the same set (`{a, a}` ≡ `{a}`) rather than an error. Validation
    /// still lives solely in [`new`](Self::new): the retained labels are distinct by construction, so
    /// the inner call cannot fail. Deduplicating by `identity(kept.len())` — the identity a label would
    /// take at the position it would occupy — makes an [`Anonymous`] header (identity = position) a
    /// no-op, so nothing is ever dropped from a positional set.
    pub(crate) fn deduped(labels: impl IntoIterator<Item = L>) -> Arc<Symbols<L>> {
        let mut seen = HashSet::new();
        let mut kept: Vec<L> = Vec::new();
        for label in labels {
            if seen.insert(label.identity(kept.len())) {
                kept.push(label);
            }
        }
        Symbols::new(kept.into()).expect("deduplicated labels are distinct by construction")
    }

    /// The index of a label, or `None` if absent. O(log n) via binary search over the identity-sorted
    /// order (which, for a real label type, *is* sorted by the label).
    ///
    /// Accepts any borrowed form of the label (so a `Symbols<Symbol>` can be queried with `&str`).
    #[must_use]
    pub fn index_of<Q>(&self, label: &Q) -> Option<u32>
    where
        L: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.sorted
            .binary_search_by(|&pos| self.labels[pos as usize].borrow().cmp(label))
            .ok()
            .map(|k| self.sorted[k])
    }

    /// Positions `0..arity` sorted by variable [`identity`](Label::identity), computed at construction.
    ///
    /// Lets minterms of different headers be aligned by a linear merge of their two sorted identity
    /// sequences (O(n+m)) rather than by building a union set and re-projecting.
    pub(crate) fn sorted_order(&self) -> &[u32] {
        &self.sorted
    }

    /// The index of the variable whose [`identity`](Label::identity) equals `id`, or `None` if absent.
    /// O(log n) via a binary search over the [`sorted_order`](Self::sorted_order).
    ///
    /// This is the identity-keyed counterpart of [`index_of`](Self::index_of): `index_of` looks a
    /// variable up by a borrowed label (`&str`) for any real label type, whereas this aligns by the
    /// abstract `Identity` and so works uniformly for every `L: Label` (including [`Anonymous`], whose
    /// identity is the position).
    pub(crate) fn position_of_identity(&self, id: &L::Identity) -> Option<u32> {
        self.sorted
            .binary_search_by(|&pos| self.labels[pos as usize].identity(pos as usize).cmp(id))
            .ok()
            .map(|k| self.sorted[k])
    }
}
