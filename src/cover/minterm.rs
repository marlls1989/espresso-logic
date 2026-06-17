//! The unified [`Minterm`] type: a label-carrying row of tri-state values.
//!
//! A `Minterm` models one row of a Boolean cover — a value per variable, where each value is
//! `Some(true)` (1), `Some(false)` (0), or `None` (don't-care). Unlike a bare positional slice, a
//! `Minterm` **carries its variable names** (via a shared [`Symbols`] table), so comparisons align
//! by variable identity rather than by raw position. This makes it safe to compare minterms that
//! came from different orderings, and lets the same type serve as both the input pattern and the
//! (membership) output pattern of a cube.
//!
//! # Representation
//!
//! Names live in a shared [`Symbols`] table (every cube of a cover shares one `Arc<Symbols>`, so
//! same-cover comparisons take a pointer-equality fast path and label lookup is O(1)). Values are
//! packed two bits per variable using Espresso's value-set encoding — for each variable, one bit
//! means "0 is allowed" and one means "1 is allowed":
//!
//! | value         | allows-0 | allows-1 | 2-bit field |
//! |---------------|----------|----------|-------------|
//! | `Some(false)` | yes      | no       | `01`        |
//! | `Some(true)`  | no       | yes      | `10`        |
//! | `None` (`-`)  | yes      | yes      | `11`        |
//! | *empty*       | no       | no       | `00`        |
//!
//! The packing is private; the public API is expressed in `Option<bool>`. The encoding matches the
//! C library's cube layout, which makes set operations cheap (word-wise bit ops) and the Espresso
//! boundary close to a bit-repack.

use super::symbols::{self, Symbols};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

/// Mask selecting the "allows-0" bit of every variable field in a word.
const ALLOWS0_MASK: u64 = 0x5555_5555_5555_5555;
/// Number of variables packed into one 64-bit word (2 bits each).
const VARS_PER_WORD: usize = 32;

/// 2-bit field values.
const FIELD_FALSE: u8 = 0b01;
const FIELD_TRUE: u8 = 0b10;
const FIELD_DC: u8 = 0b11;

#[inline]
fn words_for(num_vars: usize) -> usize {
    num_vars.div_ceil(VARS_PER_WORD)
}

#[inline]
fn encode(value: Option<bool>) -> u8 {
    match value {
        Some(false) => FIELD_FALSE,
        Some(true) => FIELD_TRUE,
        None => FIELD_DC,
    }
}

#[inline]
fn decode(field: u8) -> Option<bool> {
    match field {
        FIELD_FALSE => Some(false),
        FIELD_TRUE => Some(true),
        // Don't-care, and (defensively) the empty field, surface as don't-care to the public API.
        _ => None,
    }
}

#[inline]
fn field_at(words: &[u64], i: usize) -> u8 {
    ((words[i / VARS_PER_WORD] >> ((i % VARS_PER_WORD) * 2)) & 0b11) as u8
}

fn pack<I>(values: I, num_vars: usize) -> Arc<[u64]>
where
    I: IntoIterator<Item = Option<bool>>,
{
    // Bit-scatter needs a fixed buffer (each value sets 2 bits at an arbitrary offset); freeze
    // it into the minterm's write-once `Arc<[u64]>` storage.
    let mut words = vec![0u64; words_for(num_vars)];
    for (i, value) in values.into_iter().enumerate() {
        words[i / VARS_PER_WORD] |= (encode(value) as u64) << ((i % VARS_PER_WORD) * 2);
    }
    words.into()
}

/// Even-bit (`allows-0`) mask covering exactly the valid fields of word `word_idx`.
#[inline]
fn valid_even_mask(word_idx: usize, num_vars: usize) -> u64 {
    let count = (num_vars - word_idx * VARS_PER_WORD).min(VARS_PER_WORD);
    if count == VARS_PER_WORD {
        ALLOWS0_MASK
    } else {
        ALLOWS0_MASK & ((1u64 << (2 * count)) - 1)
    }
}

/// A label-carrying row of tri-state values. See the [module docs](self) for the representation.
#[derive(Clone)]
pub struct Minterm {
    symbols: Arc<Symbols>,
    /// Packed 2-bit value-set fields, 32 variables per word.
    values: Arc<[u64]>,
}

impl fmt::Debug for Minterm {
    /// Renders values by name — e.g. `Minterm { a: 1, b: -, c: 0 }` where `1`/`0`/`-` are
    /// true/false/don't-care — rather than exposing the internal packed words.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Minterm {{")?;
        for (i, (name, value)) in self.symbols.labels().iter().zip(self.iter()).enumerate() {
            let sym = match value {
                Some(true) => '1',
                Some(false) => '0',
                None => '-',
            };
            write!(f, "{} {name}: {sym}", if i == 0 { "" } else { "," })?;
        }
        write!(f, " }}")
    }
}

impl Minterm {
    /// Build a minterm from values against a shared [`Symbols`] table.
    ///
    /// `values` is read positionally against `symbols`; both describe the same number of variables.
    /// Minterms built from the *same* `Arc<Symbols>` compare via the pointer-equality fast path.
    pub fn from_symbols<I>(symbols: Arc<Symbols>, values: I) -> Self
    where
        I: IntoIterator<Item = Option<bool>>,
    {
        let num_vars = symbols.arity();
        Minterm {
            values: pack(values, num_vars),
            symbols,
        }
    }

    /// Build a standalone minterm from a slice of values, generating anonymous `x0, x1, …` names.
    ///
    /// Convenient for tests and ad-hoc use. Minterms built this way share no symbol table, so they
    /// compare via the (correct, slightly slower) name-aligned path.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let m = Minterm::new(&[Some(true), None, Some(false)]);
    /// assert_eq!(m.num_vars(), 3);
    /// ```
    pub fn new(values: &[Option<bool>]) -> Self {
        let labels: Arc<[Arc<str>]> = (0..values.len())
            .map(|i| Arc::from(format!("x{i}").as_str()))
            .collect();
        Self::from_symbols(Symbols::new(labels), values.iter().copied())
    }

    /// The shared symbol table this minterm is defined over.
    pub fn symbols(&self) -> &Arc<Symbols> {
        &self.symbols
    }

    /// The variable names this minterm is defined over (its shared header).
    pub fn vars(&self) -> &[Arc<str>] {
        self.symbols.labels()
    }

    /// The number of variables defined in this minterm.
    pub fn num_vars(&self) -> usize {
        self.symbols.arity()
    }

    /// The value at positional index `i` in this minterm's own variable order.
    ///
    /// Returns `None` (don't-care) for indices beyond the minterm's width.
    pub fn value_at(&self, i: usize) -> Option<bool> {
        if i < self.num_vars() {
            decode(field_at(&self.values, i))
        } else {
            None
        }
    }

    /// The value of a named variable (`None` if the variable is absent → implicitly don't-care).
    pub fn value_of(&self, name: &str) -> Option<bool> {
        match self.symbols.index_of(name) {
            Some(i) => decode(field_at(&self.values, i as usize)),
            None => None,
        }
    }

    /// Iterate over the values in this minterm's own variable order.
    pub fn iter(&self) -> impl Iterator<Item = Option<bool>> + '_ {
        (0..self.num_vars()).map(move |i| decode(field_at(&self.values, i)))
    }

    /// Re-express this minterm over a `target` symbol table (the union-extend primitive).
    ///
    /// Variables of `target` absent from `self` become don't-care; variables of `self` absent from
    /// `target` are dropped. The result shares `target` as its symbol table.
    pub fn project_onto(&self, target: &Arc<Symbols>) -> Minterm {
        if Arc::ptr_eq(&self.symbols, target) {
            return self.clone();
        }
        Minterm {
            values: self.project_words(target).into(),
            symbols: Arc::clone(target),
        }
    }

    /// Pack `self`'s values, reordered onto `target` (absent → don't-care).
    ///
    /// Returns the bare word buffer: callers either freeze it into storage (`project_onto`) or use
    /// it as a throwaway for an aligned set operation (`aligned`).
    fn project_words(&self, target: &Symbols) -> Vec<u64> {
        let mut words = vec![0u64; words_for(target.arity())];
        for (i, name) in target.labels().iter().enumerate() {
            let field = self
                .symbols
                .index_of(name)
                .map(|j| field_at(&self.values, j as usize))
                .unwrap_or(FIELD_DC);
            words[i / VARS_PER_WORD] |= (field as u64) << ((i % VARS_PER_WORD) * 2);
        }
        words
    }

    /// Align two minterms to a common word layout for per-field set operations.
    ///
    /// Fast path: shared symbol table → borrow both word slices directly. Slow path: project both
    /// onto the sorted union of their variables.
    fn aligned<'a>(&'a self, other: &'a Self) -> (Cow<'a, [u64]>, Cow<'a, [u64]>, usize) {
        if Arc::ptr_eq(&self.symbols, &other.symbols) {
            (
                Cow::Borrowed(&self.values[..]),
                Cow::Borrowed(&other.values[..]),
                self.num_vars(),
            )
        } else {
            let union = symbols::union(&self.symbols, &other.symbols);
            let a = self.project_words(&union);
            let b = other.project_words(&union);
            (Cow::Owned(a), Cow::Owned(b), union.arity())
        }
    }

    /// Whether every variable's value-set in `self` is contained in `other`'s.
    ///
    /// `self` is a subset of `other` when `other` covers every position fixed in `self`; a
    /// don't-care in `other` covers any fixed value in `self`, but a fixed value in `other` cannot
    /// cover a don't-care in `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let minterm1 = Minterm::new(&[Some(true), Some(false), None]);
    /// let minterm2 = Minterm::new(&[Some(true), Some(false), Some(true)]);
    ///
    /// assert!(!minterm1.is_subset_of(&minterm2));
    /// assert!(minterm2.is_subset_of(&minterm1));
    /// ```
    pub fn is_subset_of(&self, other: &Self) -> bool {
        let (a, b, _) = self.aligned(other);
        // self ⊆ other  ⟺  every allowed bit of self is allowed in other  ⟺  a & !b == 0.
        a.iter().zip(b.iter()).all(|(&x, &y)| x & !y == 0)
    }

    /// Whether `self` covers every position fixed in `other`. The dual of [`is_subset_of`](Self::is_subset_of).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let minterm1 = Minterm::new(&[Some(true), Some(false), None]);
    /// let minterm2 = Minterm::new(&[Some(true), Some(false), Some(true)]);
    ///
    /// assert!(minterm1.is_superset_of(&minterm2));
    /// assert!(!minterm2.is_superset_of(&minterm1));
    /// ```
    pub fn is_superset_of(&self, other: &Self) -> bool {
        other.is_subset_of(self)
    }

    /// Whether `self` and `other` share no common assignment (their intersection is empty).
    ///
    /// They are disjoint when some variable is fixed to opposite values in the two minterms.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let minterm1 = Minterm::new(&[Some(true),  Some(false), None]);
    /// let minterm2 = Minterm::new(&[Some(false), Some(true),  Some(false)]);
    /// let minterm3 = Minterm::new(&[Some(true),  None,        Some(false)]);
    ///
    /// assert!(minterm1.is_disjoint_with(&minterm2));
    /// assert!(!minterm1.is_disjoint_with(&minterm3));
    /// ```
    pub fn is_disjoint_with(&self, other: &Self) -> bool {
        let (a, b, num_vars) = self.aligned(other);
        // Intersect the value-sets; a field becomes empty (00) exactly where the two disagree.
        for (k, (&x, &y)) in a.iter().zip(b.iter()).enumerate() {
            let inter = x & y;
            let allows0 = inter & ALLOWS0_MASK;
            let allows1 = (inter >> 1) & ALLOWS0_MASK;
            let nonempty = allows0 | allows1;
            let valid = valid_even_mask(k, num_vars);
            if nonempty & valid != valid {
                return true;
            }
        }
        false
    }
}

/// Order rank for a value, smallest first: don't-care < false < true.
#[inline]
fn rank(value: Option<bool>) -> u8 {
    match value {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    }
}

/// Compare two minterms by value, aligned by variable identity over their sorted union.
///
/// The order is total and independent of header ordering: variables are visited in name order, an
/// absent variable counts as don't-care, and at each variable don't-care < false < true. This makes
/// `Minterm` usable as a `BTreeSet`/`BTreeMap` key for deduplication.
///
/// # Examples
///
/// ```
/// use espresso_logic::Minterm;
///
/// let minterm1 = Minterm::new(&[Some(true), Some(false), None]);
/// let minterm2 = Minterm::new(&[Some(true), Some(false), Some(true)]);
///
/// assert!(minterm1 < minterm2);
/// assert!(minterm2 > minterm1);
/// ```
impl Ord for Minterm {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut names: BTreeSet<&str> = BTreeSet::new();
        names.extend(self.vars().iter().map(|n| n.as_ref()));
        names.extend(other.vars().iter().map(|n| n.as_ref()));
        for name in names {
            let a = rank(self.value_of(name));
            let b = rank(other.value_of(name));
            match a.cmp(&b) {
                Ordering::Equal => continue,
                non_eq => return non_eq,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for Minterm {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Minterm {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Minterm {}

#[cfg(test)]
mod tests {
    use super::*;

    fn syms(names: &[&str]) -> Arc<Symbols> {
        Symbols::new(names.iter().map(|s| Arc::from(*s)).collect())
    }

    /// Round-trip every value through the packed encoding.
    #[test]
    fn pack_unpack_roundtrip() {
        let values: Vec<Option<bool>> = (0..70)
            .map(|i| match i % 3 {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            })
            .collect();
        let m = Minterm::new(&values);
        assert_eq!(m.iter().collect::<Vec<_>>(), values);
    }

    #[test]
    fn value_of_by_name() {
        let m = Minterm::from_symbols(syms(&["a", "b", "c"]), [Some(true), None, Some(false)]);
        assert_eq!(m.value_of("a"), Some(true));
        assert_eq!(m.value_of("b"), None);
        assert_eq!(m.value_of("c"), Some(false));
        assert_eq!(m.value_of("missing"), None);
    }

    #[test]
    fn comparison_aligns_by_name_not_position() {
        // Same variables, different header orderings, must compare equal.
        let m1 = Minterm::from_symbols(syms(&["a", "b"]), [Some(true), Some(false)]);
        let m2 = Minterm::from_symbols(syms(&["b", "a"]), [Some(false), Some(true)]);
        assert_eq!(m1, m2);
        assert!(m1.is_subset_of(&m2) && m2.is_subset_of(&m1));
    }

    #[test]
    fn absent_variable_equals_dont_care() {
        let short = Minterm::from_symbols(syms(&["a"]), [Some(true)]);
        let long = Minterm::from_symbols(syms(&["a", "b"]), [Some(true), None]);
        assert_eq!(short, long);
        assert!(short.is_subset_of(&long));
        assert!(long.is_subset_of(&short));
    }

    #[test]
    fn fast_path_matches_slow_path() {
        let s = syms(&["a", "b", "c"]);
        // Shared symbol table (fast path).
        let a = Minterm::from_symbols(Arc::clone(&s), [Some(true), None, Some(false)]);
        let b = Minterm::from_symbols(Arc::clone(&s), [Some(true), Some(false), Some(false)]);
        // Same values, independent symbol tables (slow path).
        let a2 = Minterm::new(&[Some(true), None, Some(false)]);
        let b2 = Minterm::new(&[Some(true), Some(false), Some(false)]);
        assert_eq!(a.is_subset_of(&b), a2.is_subset_of(&b2));
        assert_eq!(b.is_subset_of(&a), b2.is_subset_of(&a2));
        assert_eq!(a.is_disjoint_with(&b), a2.is_disjoint_with(&b2));
    }

    /// Reference subset/disjoint over `Option<bool>` to validate the bit-twiddling.
    #[test]
    fn set_ops_against_reference() {
        fn ref_subset(a: &[Option<bool>], b: &[Option<bool>]) -> bool {
            a.iter().zip(b).all(|(x, y)| match (x, y) {
                (None, Some(_)) => false,
                (Some(p), Some(q)) => p == q,
                _ => true,
            })
        }
        fn ref_disjoint(a: &[Option<bool>], b: &[Option<bool>]) -> bool {
            a.iter()
                .zip(b)
                .any(|(x, y)| matches!((x, y), (Some(p), Some(q)) if p != q))
        }

        let opts = [Some(false), Some(true), None];
        for &x0 in &opts {
            for &x1 in &opts {
                for &y0 in &opts {
                    for &y1 in &opts {
                        let a = Minterm::new(&[x0, x1]);
                        let b = Minterm::new(&[y0, y1]);
                        assert_eq!(a.is_subset_of(&b), ref_subset(&[x0, x1], &[y0, y1]));
                        assert_eq!(a.is_disjoint_with(&b), ref_disjoint(&[x0, x1], &[y0, y1]));
                    }
                }
            }
        }
    }

    #[test]
    fn disjoint_across_word_boundary() {
        // Variable 40 (second word) is the only conflict.
        let mut va = vec![None; 64];
        let mut vb = vec![None; 64];
        va[40] = Some(true);
        vb[40] = Some(false);
        let a = Minterm::new(&va);
        let b = Minterm::new(&vb);
        assert!(a.is_disjoint_with(&b));
        vb[40] = Some(true);
        let b = Minterm::new(&vb);
        assert!(!a.is_disjoint_with(&b));
    }

    #[test]
    fn ord_is_dont_care_lt_false_lt_true() {
        let dc = Minterm::new(&[None]);
        let f = Minterm::new(&[Some(false)]);
        let t = Minterm::new(&[Some(true)]);
        assert!(dc < f);
        assert!(f < t);
        assert!(dc < t);
    }
}
