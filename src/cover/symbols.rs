//! Shared symbol table backing a cover's variable labels.
//!
//! A [`Symbols`] table maps a dense index range (`0..arity`) to variable labels and provides a
//! lazily-built reverse (label → index) lookup. Every cube of a cover shares one `Arc<Symbols>`, so
//! aligning two cubes of the same cover is a pointer-equality check ([`Arc::ptr_eq`]) and looking a
//! variable up by name is O(1) rather than a linear scan over a header.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// A cover's variable labels, indexed `0..arity`, with a lazy label→index map.
///
/// Construct via [`Symbols::new`]; the table is immutable once built and shared behind an `Arc`.
pub struct Symbols {
    /// index → label. Its length is the arity.
    labels: Arc<[Arc<str>]>,
    /// label → index, built on first [`index_of`](Symbols::index_of).
    index: OnceLock<HashMap<Arc<str>, u32>>,
}

impl Symbols {
    /// Build a symbol table from an ordered list of labels.
    pub fn new(labels: Arc<[Arc<str>]>) -> Arc<Symbols> {
        Arc::new(Symbols {
            labels,
            index: OnceLock::new(),
        })
    }

    /// An empty symbol table (arity 0).
    pub fn empty() -> Arc<Symbols> {
        Symbols::new(Vec::new().into())
    }

    /// The number of variables (positions) this table describes.
    pub fn arity(&self) -> usize {
        self.labels.len()
    }

    /// The labels, in index order.
    pub fn labels(&self) -> &[Arc<str>] {
        &self.labels
    }

    /// The index of a label by name, or `None` if absent. O(1) after a one-time build.
    pub fn index_of(&self, name: &str) -> Option<u32> {
        self.index
            .get_or_init(|| {
                self.labels
                    .iter()
                    .enumerate()
                    .map(|(i, l)| (Arc::clone(l), i as u32))
                    .collect()
            })
            .get(name)
            .copied()
    }
}

/// The sorted union of two symbol tables (labels deduplicated by name), as a fresh shared table.
pub(crate) fn union(a: &Symbols, b: &Symbols) -> Arc<Symbols> {
    let mut set: std::collections::BTreeSet<Arc<str>> = std::collections::BTreeSet::new();
    for name in a.labels.iter().chain(b.labels.iter()) {
        set.insert(Arc::clone(name));
    }
    Symbols::new(set.into_iter().collect())
}
