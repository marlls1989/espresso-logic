//! The unified [`Minterm`] type: a label-carrying row of tri-state values.
//!
//! A `Minterm` models one row of a Boolean cover — a value per variable, where each value is
//! `Some(true)` (1), `Some(false)` (0), or `None` (don't-care). Unlike a bare positional slice, a
//! `Minterm` **carries its variable labels** (via a shared [`Symbols`] table), so comparisons align
//! by variable identity rather than by raw position. This makes it safe to compare minterms that
//! came from different orderings, and lets the same type serve as both the input pattern and the
//! (membership) output pattern of a cube.
//!
//! The label type `L` is fully generic, with no default — `Symbol` is just one choice among
//! `String`, `Arc<str>`, `u32`, [`Anonymous`](crate::Anonymous), … The core type imposes no bound on
//! `L`; richer operations add bounds on their own `impl` blocks (most need `L: Label` — the alignment
//! contract — and `Debug` additionally needs `L: Debug`).
//!
//! # Representation
//!
//! Labels live in a shared [`Symbols`] table (every cube of a cover shares one `Arc<Symbols<L>>`, so
//! same-cover comparisons take a pointer-equality fast path and label lookup is O(log n)). Values are
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

use super::label::{Anonymous, Label};
use super::symbols::Symbols;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
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

/// A label-carrying row of tri-state values. See the module-level documentation for the representation.
#[derive(Clone)]
pub struct Minterm<L> {
    symbols: Arc<Symbols<L>>,
    /// Packed 2-bit value-set fields, 32 variables per word.
    values: Arc<[u64]>,
}

impl<L: Label + fmt::Debug> fmt::Debug for Minterm<L> {
    /// Renders values by variable — e.g. `Minterm { "a": 1, "b": - }` for named minterms, or
    /// `Minterm { 0: 1, 1: - }` for anonymous (positional) ones — where `1`/`0`/`-` are
    /// true/false/don't-care, rather than exposing the internal packed words.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Minterm {{")?;
        let labels = self.symbols.labels();
        for (i, value) in self.iter().enumerate() {
            let sym = match value {
                Some(true) => '1',
                Some(false) => '0',
                None => '-',
            };
            let sep = if i == 0 { "" } else { "," };
            if L::NAMED {
                write!(f, "{sep} {:?}: {sym}", labels[i])?;
            } else {
                write!(f, "{sep} {i}: {sym}")?;
            }
        }
        write!(f, " }}")
    }
}

/// Renders the tri-state values as a bare `1`/`0`/`-` row (true/false/don't-care), in index order —
/// the cube body of a PLA line. Unlike the `Debug` form, no variable labels are shown, so this
/// needs no bound on `L`.
impl<L> fmt::Display for Minterm<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for value in self.iter() {
            f.write_str(match value {
                Some(true) => "1",
                Some(false) => "0",
                None => "-",
            })?;
        }
        Ok(())
    }
}

impl<L> Minterm<L> {
    /// Build a minterm from values against a shared [`Symbols`] table.
    ///
    /// `values` is read positionally against `symbols`; both describe the same number of variables.
    /// Minterms built from the *same* `Arc<Symbols>` compare via the pointer-equality fast path.
    #[must_use]
    pub fn from_symbols<I>(symbols: Arc<Symbols<L>>, values: I) -> Self
    where
        I: IntoIterator<Item = Option<bool>>,
    {
        let num_vars = symbols.arity();
        Minterm {
            values: pack(values, num_vars),
            symbols,
        }
    }

    /// The shared symbol table this minterm is defined over.
    pub fn symbols(&self) -> &Arc<Symbols<L>> {
        &self.symbols
    }

    /// The variables this minterm is defined over (its shared header), in index order.
    pub fn vars(&self) -> &[L] {
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

    /// Iterate over the values in this minterm's own variable order.
    pub fn iter(&self) -> impl Iterator<Item = Option<bool>> + '_ {
        (0..self.num_vars()).map(move |i| decode(field_at(&self.values, i)))
    }
}

/// Iterator over a minterm's tri-state values in index order, created by `(&minterm).into_iter()`
/// (i.e. `for value in &minterm`). Mirrors [`Minterm::iter`].
pub struct MintermIter<'a, L> {
    minterm: &'a Minterm<L>,
    pos: usize,
}

impl<L> Iterator for MintermIter<'_, L> {
    type Item = Option<bool>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.minterm.num_vars() {
            let value = self.minterm.value_at(self.pos);
            self.pos += 1;
            Some(value)
        } else {
            None
        }
    }
}

impl<'a, L> IntoIterator for &'a Minterm<L> {
    type Item = Option<bool>;
    type IntoIter = MintermIter<'a, L>;

    fn into_iter(self) -> MintermIter<'a, L> {
        MintermIter {
            minterm: self,
            pos: 0,
        }
    }
}

impl Minterm<Anonymous> {
    /// Build a standalone **anonymous** (positional) minterm from a slice of values.
    ///
    /// Convenient for tests and ad-hoc use; the variables are positional ([`Anonymous`]), so two such
    /// minterms align by position. For a named minterm build a [`Symbols`] table and use
    /// [`from_symbols`](Minterm::from_symbols).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let m = Minterm::anonymous(&[Some(true), None, Some(false)]);
    /// assert_eq!(m.num_vars(), 3);
    /// ```
    #[must_use]
    pub fn anonymous(values: &[Option<bool>]) -> Self {
        Self::from_symbols(
            Symbols::<Anonymous>::anonymous(values.len()),
            values.iter().copied(),
        )
    }
}

impl<L: Label> Minterm<L> {
    /// The value of a named variable (`None` if the variable is absent → implicitly don't-care).
    ///
    /// Accepts any borrowed form of the label (so a `Minterm<Symbol>` can be queried with `&str`).
    pub fn value_of<Q>(&self, label: &Q) -> Option<bool>
    where
        L: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        match self.symbols.index_of(label) {
            Some(i) => decode(field_at(&self.values, i as usize)),
            None => None,
        }
    }
}

impl<L: Label> Minterm<L> {
    /// Re-express this minterm over a `target` symbol table (the union-extend primitive).
    ///
    /// Variables of `target` absent from `self` become don't-care; variables of `self` absent from
    /// `target` are dropped. The result shares `target` as its symbol table. Alignment is by variable
    /// [`identity`](Label::identity): by name for labelled headers, by position for anonymous ones —
    /// one path for every `L: Label`.
    #[must_use]
    pub fn project_onto(&self, target: &Arc<Symbols<L>>) -> Minterm<L> {
        // Same index space (pointer-equal, or structurally-equal identities) → reuse values as-is.
        if &self.symbols == target {
            return Minterm {
                values: Arc::clone(&self.values),
                symbols: Arc::clone(target),
            };
        }
        Minterm {
            values: self.project_words(target).into(),
            symbols: Arc::clone(target),
        }
    }

    /// Pack `self`'s values, reordered onto `target` (absent → don't-care).
    ///
    /// Each `target` position pulls `self`'s field for the variable of the same
    /// [`identity`](Label::identity) (absent → don't-care) — one uniform path: by name for a named
    /// header, by position for an anonymous one (`Anonymous`'s identity is its index, so positions
    /// beyond `self`'s width pad don't-care).
    fn project_words(&self, target: &Symbols<L>) -> Vec<u64> {
        let mut words = vec![0u64; words_for(target.arity())];
        for i in 0..target.arity() {
            let id = target.labels()[i].identity(i);
            let field = self
                .symbols
                .position_of_identity(&id)
                .map(|j| field_at(&self.values, j as usize))
                .unwrap_or(FIELD_DC);
            words[i / VARS_PER_WORD] |= (field as u64) << ((i % VARS_PER_WORD) * 2);
        }
        words
    }
}

impl<L: Label> Minterm<L> {
    /// Merge-join the two minterms' fields, aligned by variable identity in sorted-label order.
    ///
    /// Yields `(self_field, other_field)` per variable of the union; an absent variable reads as
    /// don't-care. O(n+m) over the two sorted label sequences — no union set, no projection. Callers
    /// that share a symbol table take the faster word-wise path directly instead of this.
    fn merged_fields<'a>(&'a self, other: &'a Self) -> MergedFields<'a, L> {
        MergedFields {
            a: self,
            b: other,
            sa: self.symbols.sorted_order(),
            sb: other.symbols.sorted_order(),
            i: 0,
            j: 0,
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
    /// let minterm1 = Minterm::anonymous(&[Some(true), Some(false), None]);
    /// let minterm2 = Minterm::anonymous(&[Some(true), Some(false), Some(true)]);
    ///
    /// assert!(!minterm1.is_subset_of(&minterm2));
    /// assert!(minterm2.is_subset_of(&minterm1));
    /// ```
    #[must_use]
    pub fn is_subset_of(&self, other: &Self) -> bool {
        // self ⊆ other  ⟺  every allowed bit of self is allowed in other  ⟺  self & !other == 0.
        if self.symbols == other.symbols {
            self.values
                .iter()
                .zip(other.values.iter())
                .all(|(&x, &y)| x & !y == 0)
        } else {
            self.merged_fields(other).all(|(sf, of)| sf & !of == 0)
        }
    }

    /// Whether `self` covers every position fixed in `other`. The dual of [`is_subset_of`](Self::is_subset_of).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let minterm1 = Minterm::anonymous(&[Some(true), Some(false), None]);
    /// let minterm2 = Minterm::anonymous(&[Some(true), Some(false), Some(true)]);
    ///
    /// assert!(minterm1.is_superset_of(&minterm2));
    /// assert!(!minterm2.is_superset_of(&minterm1));
    /// ```
    #[must_use]
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
    /// let minterm1 = Minterm::anonymous(&[Some(true),  Some(false), None]);
    /// let minterm2 = Minterm::anonymous(&[Some(false), Some(true),  Some(false)]);
    /// let minterm3 = Minterm::anonymous(&[Some(true),  None,        Some(false)]);
    ///
    /// assert!(minterm1.is_disjoint_with(&minterm2));
    /// assert!(!minterm1.is_disjoint_with(&minterm3));
    /// ```
    #[must_use]
    pub fn is_disjoint_with(&self, other: &Self) -> bool {
        if self.symbols == other.symbols {
            // Word-wise: a field's intersection is empty (00) exactly where the two disagree.
            for (k, (&x, &y)) in self.values.iter().zip(other.values.iter()).enumerate() {
                let inter = x & y;
                let allows0 = inter & ALLOWS0_MASK;
                let allows1 = (inter >> 1) & ALLOWS0_MASK;
                let nonempty = allows0 | allows1;
                let valid = valid_even_mask(k, self.num_vars());
                if nonempty & valid != valid {
                    return true;
                }
            }
            false
        } else {
            // A field whose intersection is empty means the two fix that variable oppositely.
            self.merged_fields(other).any(|(sf, of)| sf & of == 0)
        }
    }
}

/// Merge-join iterator backing [`Minterm::merged_fields`]; a variable absent from one side reads as
/// don't-care (`FIELD_DC`).
struct MergedFields<'a, L> {
    a: &'a Minterm<L>,
    b: &'a Minterm<L>,
    sa: &'a [u32],
    sb: &'a [u32],
    i: usize,
    j: usize,
}

impl<L: Label> Iterator for MergedFields<'_, L> {
    type Item = (u8, u8);

    fn next(&mut self) -> Option<(u8, u8)> {
        let la = self.a.symbols.labels();
        let lb = self.b.symbols.labels();
        match (self.sa.get(self.i), self.sb.get(self.j)) {
            (Some(&ia), Some(&ib)) => match la[ia as usize]
                .identity(ia as usize)
                .cmp(&lb[ib as usize].identity(ib as usize))
            {
                Ordering::Less => {
                    self.i += 1;
                    Some((field_at(&self.a.values, ia as usize), FIELD_DC))
                }
                Ordering::Greater => {
                    self.j += 1;
                    Some((FIELD_DC, field_at(&self.b.values, ib as usize)))
                }
                Ordering::Equal => {
                    self.i += 1;
                    self.j += 1;
                    Some((
                        field_at(&self.a.values, ia as usize),
                        field_at(&self.b.values, ib as usize),
                    ))
                }
            },
            (Some(&ia), None) => {
                self.i += 1;
                Some((field_at(&self.a.values, ia as usize), FIELD_DC))
            }
            (None, Some(&ib)) => {
                self.j += 1;
                Some((FIELD_DC, field_at(&self.b.values, ib as usize)))
            }
            (None, None) => None,
        }
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

/// Compare two minterms by value, aligned by variable identity over the sorted union of their
/// variables (an absent variable counts as don't-care; at each variable don't-care < false < true).
///
/// The order is total and independent of header ordering, so `Minterm` can be used as a
/// `BTreeSet`/`BTreeMap` key for deduplication. It is computed by a single O(n+m) merge of the two
/// minterms' sorted label sequences — no per-comparison set or projection is allocated.
///
/// # Examples
///
/// ```
/// use espresso_logic::Minterm;
///
/// let minterm1 = Minterm::anonymous(&[Some(true), Some(false), None]);
/// let minterm2 = Minterm::anonymous(&[Some(true), Some(false), Some(true)]);
///
/// assert!(minterm1 < minterm2);
/// assert!(minterm2 > minterm1);
/// ```
impl<L: Label> Ord for Minterm<L> {
    fn cmp(&self, other: &Self) -> Ordering {
        for (sf, of) in self.merged_fields(other) {
            match rank(decode(sf)).cmp(&rank(decode(of))) {
                Ordering::Equal => continue,
                non_eq => return non_eq,
            }
        }
        Ordering::Equal
    }
}

impl<L: Label> PartialOrd for Minterm<L> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<L: Label> PartialEq for Minterm<L> {
    fn eq(&self, other: &Self) -> bool {
        if self.symbols == other.symbols {
            // Same layout: equal value-sets pack to identical words.
            self.values == other.values
        } else {
            self.merged_fields(other).all(|(sf, of)| sf == of)
        }
    }
}

impl<L: Label> Eq for Minterm<L> {}

/// Hashes the same identity-aligned canonical sequence that [`Eq`]/[`Ord`] compare over, so
/// `a == b` always implies `hash(a) == hash(b)`.
///
/// Equality aligns variables by [`identity`](Label::identity), ignores don't-care (`None`) entries
/// (an absent variable reads as don't-care, so a fixed variable and a missing one are
/// distinguished only when the present one is *not* don't-care), and is independent of physical
/// word/position order. We therefore hash, in sorted-identity order, the `(identity, value)` pair
/// of every variable whose value is **not** don't-care — skipping the don't-cares that `Eq` treats
/// as absent. Raw words, the `Arc` pointer, the arity and any position-dependent state that `Eq`
/// ignores are deliberately left out, preserving the `Hash`/`Eq` contract.
impl<L: Label> Hash for Minterm<L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let labels = self.symbols.labels();
        let mut len = 0usize;
        for &pos in self.symbols.sorted_order() {
            let pos = pos as usize;
            if let Some(value) = decode(field_at(&self.values, pos)) {
                // Walk sorted-identity order so the sequence is canonical regardless of header
                // ordering; don't-cares are skipped to match `Eq`'s absent-equals-don't-care rule.
                labels[pos].identity(pos).hash(state);
                value.hash(state);
                len += 1;
            }
        }
        // Hash the trimmed length too, so the empty/all-don't-care prefix of a longer sequence
        // cannot collide with the same prefix followed by more fixed variables.
        len.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol;

    fn syms(names: &[&str]) -> Arc<Symbols<Symbol>> {
        Symbols::new(names.iter().map(|s| Symbol::from(*s)).collect())
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
        let m = Minterm::anonymous(&values);
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
        let a2 = Minterm::anonymous(&[Some(true), None, Some(false)]);
        let b2 = Minterm::anonymous(&[Some(true), Some(false), Some(false)]);
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
                        let a = Minterm::anonymous(&[x0, x1]);
                        let b = Minterm::anonymous(&[y0, y1]);
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
        let a = Minterm::anonymous(&va);
        let b = Minterm::anonymous(&vb);
        assert!(a.is_disjoint_with(&b));
        vb[40] = Some(true);
        let b = Minterm::anonymous(&vb);
        assert!(!a.is_disjoint_with(&b));
    }

    #[test]
    fn structurally_equal_tables_are_same_space() {
        // Two independent symbol tables with identical labels (no shared Arc, no interning).
        let s1 = syms(&["a", "b"]);
        let s2 = syms(&["a", "b"]);
        assert!(!Arc::ptr_eq(&s1, &s2));
        assert!(s1 == s2); // structural equality (cheaper than re-projecting)

        let m1 = Minterm::from_symbols(Arc::clone(&s1), [Some(true), None]);
        // Projecting onto the equal-but-distinct table reuses the values and re-homes onto it.
        let m2 = m1.project_onto(&s2);
        assert!(Arc::ptr_eq(m2.symbols(), &s2));
        assert_eq!(m1, m2);
        assert_eq!(m2.value_of("a"), Some(true));
        assert_eq!(m2.value_of("b"), None);
    }

    #[test]
    fn merge_path_matches_shared_path() {
        // Same two functions built (a) on one shared header → word path, and (b) on independent,
        // differently-permuted headers → merge-join path. Set-ops and ordering must agree.
        let shared = syms(&["a", "b", "c"]);
        let a_shared = Minterm::from_symbols(Arc::clone(&shared), [Some(true), None, Some(false)]);
        let b_shared = Minterm::from_symbols(Arc::clone(&shared), [Some(true), Some(false), None]);
        // a = {a:1, b:-, c:0}; b = {a:1, b:0, c:-} expressed over permuted headers.
        let a_perm = Minterm::from_symbols(syms(&["c", "a", "b"]), [Some(false), Some(true), None]);
        let b_perm = Minterm::from_symbols(syms(&["b", "c", "a"]), [Some(false), None, Some(true)]);

        assert_eq!(a_shared, a_perm);
        assert_eq!(b_shared, b_perm);
        assert_eq!(
            a_shared.is_subset_of(&b_shared),
            a_perm.is_subset_of(&b_perm)
        );
        assert_eq!(
            b_shared.is_subset_of(&a_shared),
            b_perm.is_subset_of(&a_perm)
        );
        assert_eq!(
            a_shared.is_disjoint_with(&b_shared),
            a_perm.is_disjoint_with(&b_perm)
        );
        assert_eq!(a_shared.cmp(&b_shared), a_perm.cmp(&b_perm));
    }

    #[test]
    fn ord_is_dont_care_lt_false_lt_true() {
        let dc = Minterm::anonymous(&[None]);
        let f = Minterm::anonymous(&[Some(false)]);
        let t = Minterm::anonymous(&[Some(true)]);
        assert!(dc < f);
        assert!(f < t);
        assert!(dc < t);
    }
}
