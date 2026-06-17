//! Shared symbol table backing a cover's variable labels.
//!
//! A [`Symbols`] table describes a dense index range (`0..arity`) of variables that is *either*
//! **anonymous** (positional only — no labels) *or* **labelled** (one label of type `L` per
//! position). There is no in-between: a table can never have some positions named and others not,
//! so "labelled" always means *fully* labelled. Every cube of a cover shares one
//! `Arc<Symbols<L>>`, so aligning two cubes of the same cover is a pointer-equality check
//! ([`Arc::ptr_eq`]) and looking a variable up by label is O(1) rather than a linear scan.
//!
//! The label type is generic and defaults to `Arc<str>`. The table itself imposes no bounds on `L`;
//! only the label→index lookup needs `L: Eq + Hash + Clone`.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, OnceLock};

/// A cover's variable table: either anonymous (a width only) or fully labelled (`0..arity`
/// labels with a lazy label→index map). Construct via [`Symbols::new`] (labelled) or
/// [`Symbols::anonymous`]; the table is immutable once built and shared behind an `Arc`.
///
/// The representation is opaque: there is no way to construct a partially-labelled table, so
/// [`is_labeled`](Self::is_labeled) being `true` guarantees one label per position.
pub struct Symbols<L = Arc<str>> {
    repr: Repr<L>,
}

/// The two — and only two — shapes a symbol table can take. Private, so a partially-labelled
/// state (`Labeled` with `labels.len() != arity`) is unrepresentable outside this module, and
/// `Labeled` is only ever built with `labels.len() == arity`.
enum Repr<L> {
    /// Positional, unlabelled: just a width. Used by `Cover<(), ()>` and by `Arc<str>` covers that
    /// have no `.ilb`/`.ob` names (names, if ever needed, are synthesised at serialisation time).
    Anonymous { arity: usize },
    /// Fully labelled: one label per position (`arity == labels.len()`), with lazily-built
    /// reverse lookups.
    Labeled {
        /// index → label, one per position.
        labels: Arc<[L]>,
        /// label → index, built on first [`index_of`](Symbols::index_of).
        index: OnceLock<HashMap<L, u32>>,
        /// positions sorted by label, built on first [`sorted_order`](Symbols::sorted_order);
        /// used by the merge-join that aligns minterms of different headers.
        sorted: OnceLock<Box<[u32]>>,
    },
}

/// Two symbol tables are equal when they describe the same variables in the same order — same
/// width and same labels (an anonymous table only equals another table with no labels of the same
/// width). The lazily-built reverse lookups are derived caches and are ignored.
///
/// Because tables are shared behind an `Arc`, comparing `Arc<Symbols>` short-circuits on pointer
/// equality (the std `Arc: PartialEq` fast path for `T: Eq`) before falling back to this O(n) label
/// comparison — which is still far cheaper than re-projecting a minterm onto a union.
impl<L: PartialEq> PartialEq for Symbols<L> {
    fn eq(&self, other: &Self) -> bool {
        self.arity() == other.arity() && self.labels() == other.labels()
    }
}

impl<L: Eq> Eq for Symbols<L> {}

impl<L> Symbols<L> {
    /// Build a fully-labelled symbol table from an ordered list of labels (arity = `labels.len()`).
    ///
    /// An empty label list yields the canonical [`anonymous`](Self::anonymous) table of arity 0.
    pub fn new(labels: Arc<[L]>) -> Arc<Symbols<L>> {
        if labels.is_empty() {
            return Symbols::anonymous(0);
        }
        Arc::new(Symbols {
            repr: Repr::Labeled {
                labels,
                index: OnceLock::new(),
                sorted: OnceLock::new(),
            },
        })
    }

    /// An anonymous (positional, unlabelled) symbol table of the given width.
    pub fn anonymous(arity: usize) -> Arc<Symbols<L>> {
        Arc::new(Symbols {
            repr: Repr::Anonymous { arity },
        })
    }

    /// An empty symbol table (arity 0).
    pub fn empty() -> Arc<Symbols<L>> {
        Symbols::anonymous(0)
    }

    /// The number of variables (positions) this table describes.
    pub fn arity(&self) -> usize {
        match &self.repr {
            Repr::Anonymous { arity } => *arity,
            Repr::Labeled { labels, .. } => labels.len(),
        }
    }

    /// The variable labels in index order, or an empty slice for an anonymous table. When non-empty,
    /// the slice has exactly one entry per position (`len() == arity()`).
    pub fn labels(&self) -> &[L] {
        match &self.repr {
            Repr::Anonymous { .. } => &[],
            Repr::Labeled { labels, .. } => labels,
        }
    }

    /// Whether this table carries labels (fully — there is no partial state).
    pub fn is_labeled(&self) -> bool {
        matches!(self.repr, Repr::Labeled { .. })
    }
}

impl<L: Eq + Hash + Clone> Symbols<L> {
    /// The index of a label, or `None` if absent (always `None` for an anonymous table).
    /// O(1) after a one-time build.
    ///
    /// Accepts any borrowed form of the label (so a `Symbols<Arc<str>>` can be queried with `&str`).
    pub fn index_of<Q>(&self, label: &Q) -> Option<u32>
    where
        L: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let Repr::Labeled { labels, index, .. } = &self.repr else {
            return None;
        };
        index
            .get_or_init(|| {
                labels
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
    /// Positions `0..arity` sorted by label, built once and cached (empty for an anonymous table).
    ///
    /// Lets minterms of different headers be aligned by a linear merge of their two sorted label
    /// sequences (O(n+m)) rather than by building a union set and re-projecting.
    pub(crate) fn sorted_order(&self) -> &[u32] {
        let Repr::Labeled { labels, sorted, .. } = &self.repr else {
            return &[];
        };
        sorted.get_or_init(|| {
            let mut order: Vec<u32> = (0..labels.len() as u32).collect();
            order.sort_by(|&x, &y| labels[x as usize].cmp(&labels[y as usize]));
            order.into_boxed_slice()
        })
    }
}
