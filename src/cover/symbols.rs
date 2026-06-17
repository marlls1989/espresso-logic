//! Shared symbol table backing a cover's variable labels.
//!
//! A [`Symbols`] table maps a dense index range (`0..arity`) to variable labels of type `L` and
//! provides a lazily-built reverse (label → index) lookup. Every cube of a cover shares one
//! `Arc<Symbols<L>>`, so aligning two cubes of the same cover is a pointer-equality check
//! ([`Arc::ptr_eq`]) and looking a variable up by label is O(1) rather than a linear scan.
//!
//! The label type is generic and defaults to `Arc<str>`. The table itself imposes no bounds on `L`;
//! only the label→index lookup needs `L: Eq + Hash + Clone`.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, OnceLock};

/// A cover's variable labels, indexed `0..arity`, with a lazy label→index map.
///
/// Construct via [`Symbols::new`]; the table is immutable once built and shared behind an `Arc`.
pub struct Symbols<L = Arc<str>> {
    /// index → label. Its length is the arity.
    labels: Arc<[L]>,
    /// label → index, built on first [`index_of`](Symbols::index_of).
    index: OnceLock<HashMap<L, u32>>,
}

impl<L> Symbols<L> {
    /// Build a symbol table from an ordered list of labels.
    pub fn new(labels: Arc<[L]>) -> Arc<Symbols<L>> {
        Arc::new(Symbols {
            labels,
            index: OnceLock::new(),
        })
    }

    /// An empty symbol table (arity 0).
    pub fn empty() -> Arc<Symbols<L>> {
        Symbols::new(Vec::new().into())
    }

    /// The number of variables (positions) this table describes.
    pub fn arity(&self) -> usize {
        self.labels.len()
    }

    /// The labels, in index order.
    pub fn labels(&self) -> &[L] {
        &self.labels
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

/// The sorted union of two symbol tables (labels deduplicated), as a fresh shared table.
pub(crate) fn union<L: Ord + Clone>(a: &Symbols<L>, b: &Symbols<L>) -> Arc<Symbols<L>> {
    let mut set: std::collections::BTreeSet<L> = std::collections::BTreeSet::new();
    for name in a.labels.iter().chain(b.labels.iter()) {
        set.insert(name.clone());
    }
    Symbols::new(set.into_iter().collect())
}
