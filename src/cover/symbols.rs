//! Shared symbol table backing a cover's variable labels.
//!
//! A [`Symbols`] table describes a dense index range (`0..arity`) of variables, storing **one label
//! of type `L` per position** — there is no separate "anonymous" shape. A positional cover simply
//! uses the zero-sized [`Anonymous`] label (`Symbols<Anonymous>` holds `[Anonymous; arity]`, which
//! costs nothing); a named cover uses a real label type such as `Arc<str>`. The difference is carried
//! by the type, via [`Label::NAMED`], not by a runtime variant — so partial labelling is
//! unrepresentable (every position always has a label) and all alignment is one uniform path keyed on
//! [`Label::identity`].
//!
//! Every cube of a cover shares one `Arc<Symbols<L>>`, so aligning two cubes of the same cover is a
//! pointer-equality check ([`Arc::ptr_eq`]) and looking a variable up by name is O(1).

use super::label::{Anonymous, Label};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, OnceLock};

/// A cover's variable table: one label of type `L` per position (`0..arity`), with lazily-built
/// reverse lookups. Construct via [`Symbols::new`] (from a label list) or
/// [`Symbols::<Anonymous>::anonymous`] (positional); the table is immutable once built and shared
/// behind an `Arc`. There is no partial state — `labels.len()` *is* the arity.
pub struct Symbols<L = Arc<str>> {
    /// index → label, one per position.
    labels: Arc<[L]>,
    /// label → index, built on first [`index_of`](Symbols::index_of) (real label types only).
    index: OnceLock<HashMap<L, u32>>,
    /// positions sorted by [`identity`](Label::identity), built on first
    /// [`sorted_order`](Symbols::sorted_order); used by the merge-join that aligns differing headers.
    sorted: OnceLock<Box<[u32]>>,
}

/// Two symbol tables are equal when they describe the same variables in the same order — i.e. same
/// width and, position by position, the same variable [`identity`](Label::identity). For real labels
/// that is the label itself; two anonymous tables of equal width are equal (every position's identity
/// is its index). The lazily-built reverse lookups are derived caches and are ignored.
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

impl<L> Symbols<L> {
    /// Build a symbol table from an ordered list of labels (arity = `labels.len()`).
    pub fn new(labels: Arc<[L]>) -> Arc<Symbols<L>> {
        Arc::new(Symbols {
            labels,
            index: OnceLock::new(),
            sorted: OnceLock::new(),
        })
    }

    /// An empty symbol table (arity 0).
    pub fn empty() -> Arc<Symbols<L>> {
        Symbols::new(Arc::from([]))
    }

    /// The number of variables (positions) this table describes.
    pub fn arity(&self) -> usize {
        self.labels.len()
    }

    /// The variable labels in index order. For a positional ([`Anonymous`]) table these are the
    /// placeholder `Anonymous` values, one per position; callers that want *names* should gate on
    /// [`is_labeled`](Self::is_labeled) (or `L::NAMED`).
    pub fn labels(&self) -> &[L] {
        &self.labels
    }
}

impl Symbols<Anonymous> {
    /// An anonymous (positional) symbol table of the given width: one [`Anonymous`] placeholder per
    /// position (a named table is built from real labels via [`new`](Self::new)).
    pub fn anonymous(arity: usize) -> Arc<Symbols<Anonymous>> {
        Symbols::new((0..arity).map(|_| Anonymous).collect())
    }
}

impl<L: Label> Symbols<L> {
    /// Whether this table carries real **names** (`L::NAMED`) as opposed to positional placeholders,
    /// and is non-empty. An anonymous table, or an empty one, reports `false`.
    pub fn is_labeled(&self) -> bool {
        L::NAMED && !self.labels.is_empty()
    }
}

impl<L: Eq + Hash + Clone> Symbols<L> {
    /// The index of a label, or `None` if absent. O(1) after a one-time build.
    ///
    /// Accepts any borrowed form of the label (so a `Symbols<Arc<str>>` can be queried with `&str`).
    pub fn index_of<Q>(&self, label: &Q) -> Option<u32>
    where
        L: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.index
            .get_or_init(|| {
                self.labels
                    .iter()
                    .enumerate()
                    .map(|(i, l)| (l.clone(), i as u32))
                    .collect()
            })
            .get(label)
            .copied()
    }
}

impl<L: Label> Symbols<L> {
    /// Positions `0..arity` sorted by variable [`identity`](Label::identity), built once and cached.
    ///
    /// Lets minterms of different headers be aligned by a linear merge of their two sorted identity
    /// sequences (O(n+m)) rather than by building a union set and re-projecting.
    pub(crate) fn sorted_order(&self) -> &[u32] {
        self.sorted.get_or_init(|| {
            let mut order: Vec<u32> = (0..self.labels.len() as u32).collect();
            order.sort_by(|&x, &y| {
                self.labels[x as usize]
                    .identity(x as usize)
                    .cmp(&self.labels[y as usize].identity(y as usize))
            });
            order.into_boxed_slice()
        })
    }

    /// The index of the variable whose [`identity`](Label::identity) equals `id`, or `None` if absent.
    /// O(log n) via a binary search over the cached [`sorted_order`](Self::sorted_order).
    ///
    /// This is the identity-keyed counterpart of [`index_of`](Self::index_of): `index_of` looks a
    /// variable up by a borrowed label (`&str`) for any real label type, whereas this aligns by the
    /// abstract `Identity` and so works uniformly for every `L: Label` (including [`Anonymous`], whose
    /// identity is the position).
    pub(crate) fn position_of_identity(&self, id: &L::Identity) -> Option<u32> {
        let order = self.sorted_order();
        order
            .binary_search_by(|&pos| self.labels[pos as usize].identity(pos as usize).cmp(id))
            .ok()
            .map(|k| order[k])
    }
}
