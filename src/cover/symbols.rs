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
    /// positions sorted by label, built on first [`sorted_order`](Symbols::sorted_order); used by
    /// the merge-join that aligns minterms of different headers.
    sorted: OnceLock<Box<[u32]>>,
}

/// Two symbol tables are equal when they describe the same labels in the same order — i.e. the same
/// index space. The lazily-built reverse index is a derived cache and is ignored.
///
/// Because tables are shared behind an `Arc`, comparing `Arc<Symbols>` short-circuits on pointer
/// equality (the std `Arc: PartialEq` fast path for `T: Eq`) before falling back to this O(n) label
/// comparison — which is still far cheaper than re-projecting a minterm onto a union.
impl<L: PartialEq> PartialEq for Symbols<L> {
    fn eq(&self, other: &Self) -> bool {
        self.labels == other.labels
    }
}

impl<L: Eq> Eq for Symbols<L> {}

impl<L> Symbols<L> {
    /// Build a symbol table from an ordered list of labels.
    pub fn new(labels: Arc<[L]>) -> Arc<Symbols<L>> {
        Arc::new(Symbols {
            labels,
            index: OnceLock::new(),
            sorted: OnceLock::new(),
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

impl<L: Ord> Symbols<L> {
    /// Positions `0..arity` sorted by label, built once and cached.
    ///
    /// Lets minterms of different headers be aligned by a linear merge of their two sorted label
    /// sequences (O(n+m)) rather than by building a union set and re-projecting.
    pub(crate) fn sorted_order(&self) -> &[u32] {
        self.sorted.get_or_init(|| {
            let mut order: Vec<u32> = (0..self.labels.len() as u32).collect();
            order.sort_by(|&x, &y| self.labels[x as usize].cmp(&self.labels[y as usize]));
            order.into_boxed_slice()
        })
    }
}
