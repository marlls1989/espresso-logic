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
//! C library's cube layout, so set operations reduce to word-wise bit ops and the Espresso
//! boundary is close to a bit-repack.

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
/// The *empty* literal (Espresso's `?`): the variable matches neither value, so the cube covers no
/// minterm. `Option<bool>` cannot express it; the public value API folds it back to `None`.
const FIELD_EMPTY: u8 = 0b00;

/// A parsed input-field value. Carries the three logical states *plus* Espresso's empty literal, which
/// `Option<bool>` cannot represent — used only on the PLA read path so a `?` survives into the
/// minterm's packed bits (and on to the C backend) verbatim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InputField {
    Zero,
    One,
    DontCare,
    Empty,
}

#[inline]
fn encode_field(field: InputField) -> u8 {
    match field {
        InputField::Zero => FIELD_FALSE,
        InputField::One => FIELD_TRUE,
        InputField::DontCare => FIELD_DC,
        InputField::Empty => FIELD_EMPTY,
    }
}

#[inline]
fn decode_field(field: u8) -> InputField {
    match field {
        FIELD_FALSE => InputField::Zero,
        FIELD_TRUE => InputField::One,
        FIELD_DC => InputField::DontCare,
        // The only remaining 2-bit value is `00`, the empty literal.
        _ => InputField::Empty,
    }
}

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

/// Like [`pack`], but from raw [`InputField`]s so the empty literal (`00`) can be stored.
fn pack_fields<I>(fields: I, num_vars: usize) -> Arc<[u64]>
where
    I: IntoIterator<Item = InputField>,
{
    let mut words = vec![0u64; words_for(num_vars)];
    for (i, field) in fields.into_iter().enumerate() {
        words[i / VARS_PER_WORD] |= (encode_field(field) as u64) << ((i % VARS_PER_WORD) * 2);
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

    /// Build a minterm from raw [`InputField`]s, preserving the empty literal (`?`/`00`) that
    /// `Option<bool>` cannot express. Crate-internal; used by the PLA reader.
    pub(crate) fn from_symbols_input_fields<I>(symbols: Arc<Symbols<L>>, fields: I) -> Self
    where
        I: IntoIterator<Item = InputField>,
    {
        let num_vars = symbols.arity();
        Minterm {
            values: pack_fields(fields, num_vars),
            symbols,
        }
    }

    /// The packed 2-bit value-set words (32 variables per `u64`), in the same per-variable encoding as
    /// an Espresso input cube. Crate-internal; used to copy a cover to the C backend without re-coding.
    #[must_use]
    pub(crate) fn raw_words(&self) -> &[u64] {
        &self.values
    }

    /// The shared handle to the packed words, for cheap re-homing onto another [`Symbols`] table of the
    /// same arity (the packing is independent of the label type, so the words can be reused verbatim).
    pub(crate) fn packed(&self) -> &Arc<[u64]> {
        &self.values
    }

    /// Build a minterm from an already-packed word buffer, taking it verbatim with no re-packing — the
    /// inverse of [`raw_words`](Self::raw_words). The buffer must hold exactly `words_for(arity)` words
    /// with zero padding past the final variable's fields (so `Eq`/`Hash`, which compare whole words,
    /// stay canonical). Crate-internal; used to decode an Espresso cube by direct word-copy and to
    /// re-home a minterm onto another `Symbols` table.
    pub(crate) fn from_packed_words(symbols: Arc<Symbols<L>>, values: Arc<[u64]>) -> Self {
        debug_assert_eq!(
            values.len(),
            words_for(symbols.arity()),
            "packed word count must match the symbols' arity"
        );
        Minterm { values, symbols }
    }

    /// Whether any variable holds the empty literal (`00`) — i.e. the cube covers no minterm. Such a
    /// cube is vacuous and is dropped before minimisation (see the cover minimisation pipeline).
    #[must_use]
    pub(crate) fn has_empty_field(&self) -> bool {
        (0..self.num_vars()).any(|i| field_at(&self.values, i) == FIELD_EMPTY)
    }

    /// Disjointness of two cubes defined over the **same header** (shared [`Symbols`]), computed purely
    /// on the packed words so it needs no [`Label`] bound. Two cubes are disjoint when some variable's
    /// fields don't intersect (the intersection is the empty field `00`). Used by the orthogonality
    /// check in the cover minimisation pipeline, where every cube shares the cover's header.
    #[must_use]
    pub(crate) fn is_disjoint_same_header(&self, other: &Self) -> bool {
        debug_assert!(
            Arc::ptr_eq(&self.symbols, &other.symbols),
            "is_disjoint_same_header requires a shared header"
        );
        for (k, (&x, &y)) in self.values.iter().zip(other.values.iter()).enumerate() {
            let inter = x & y;
            let nonempty = (inter & ALLOWS0_MASK) | ((inter >> 1) & ALLOWS0_MASK);
            let valid = valid_even_mask(k, self.num_vars());
            if nonempty & valid != valid {
                return true;
            }
        }
        false
    }

    /// Iterate the raw input fields in index order, preserving the empty literal (`00`) that the
    /// public `Option<bool>` view folds to `None`. Crate-internal; used by the PLA writer so a `?`
    /// read into a cube is echoed back faithfully (matching C's `print_cube`).
    pub(crate) fn input_fields(&self) -> impl Iterator<Item = InputField> + '_ {
        (0..self.num_vars()).map(|i| decode_field(field_at(&self.values, i)))
    }

    /// The shared symbol table this minterm is defined over.
    #[must_use]
    pub fn symbols(&self) -> &Arc<Symbols<L>> {
        &self.symbols
    }

    /// The variables this minterm is defined over (its shared header), in index order.
    #[must_use]
    pub fn vars(&self) -> &[L] {
        self.symbols.labels()
    }

    /// The number of variables defined in this minterm.
    #[must_use]
    pub fn num_vars(&self) -> usize {
        self.symbols.arity()
    }

    /// The value at positional index `i` in this minterm's own variable order.
    ///
    /// Returns `None` (don't-care) for indices beyond the minterm's width.
    #[must_use]
    pub fn value_at(&self, i: usize) -> Option<bool> {
        if i < self.num_vars() {
            decode(field_at(&self.values, i))
        } else {
            None
        }
    }

    /// Iterate over the values in this minterm's own variable order.
    ///
    /// Returns the same [`MintermIter`] as `(&minterm).into_iter()`.
    #[must_use]
    pub fn iter(&self) -> MintermIter<'_, L> {
        self.into_iter()
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

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.minterm.num_vars() - self.pos;
        (remaining, Some(remaining))
    }
}

// The length is known exactly (and O(1)): it is the number of variables still to yield.
impl<L> ExactSizeIterator for MintermIter<'_, L> {}

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
    #[must_use]
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

impl<L: Label> Minterm<L> {
    /// The number of variables on which `self` and `other` disagree, aligned by variable identity.
    ///
    /// Two minterms disagree on a variable when their value-sets do not intersect — i.e. one fixes it
    /// `true` and the other `false`. A don't-care agrees with any value, and a variable present in only
    /// one minterm reads as don't-care, so neither counts as a disagreement. Intended for fully-assigned
    /// minterms (e.g. the output of [`expand_over`](Self::expand_over)), where this is exactly the
    /// Hamming distance. `hamming_distance == disagreement().len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let a = Minterm::anonymous(&[Some(true), Some(false), Some(true)]);
    /// let b = Minterm::anonymous(&[Some(true), Some(true),  Some(false)]);
    /// assert_eq!(a.hamming_distance(&b), 2);
    /// assert_eq!(a.hamming_distance(&a), 0);
    /// ```
    #[must_use]
    pub fn hamming_distance(&self, other: &Self) -> usize {
        if self.symbols == other.symbols {
            // Word-wise popcount: a variable's field-intersection is empty exactly where they disagree.
            let n = self.num_vars();
            let mut count = 0usize;
            for (k, (&x, &y)) in self.values.iter().zip(other.values.iter()).enumerate() {
                let inter = x & y;
                let nonempty = (inter & ALLOWS0_MASK) | ((inter >> 1) & ALLOWS0_MASK);
                let valid = valid_even_mask(k, n);
                // One even bit per variable whose field-intersection is empty.
                count += (valid & !nonempty).count_ones() as usize;
            }
            count
        } else {
            self.merged_fields(other).filter(|(sf, of)| sf & of == 0).count()
        }
    }

    /// The variables on which `self` and `other` disagree (see [`hamming_distance`](Self::hamming_distance)
    /// for the disagreement rule). The labels are returned cloned; ordering is unspecified.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let a = Minterm::from_symbols(
    ///     espresso_logic::Symbols::new(["a", "b", "c"].iter().map(|s| s.to_string()).collect()),
    ///     [Some(true), Some(false), Some(true)],
    /// );
    /// let b = Minterm::from_symbols(
    ///     espresso_logic::Symbols::new(["a", "b", "c"].iter().map(|s| s.to_string()).collect()),
    ///     [Some(true), Some(true), Some(false)],
    /// );
    /// let mut d = a.disagreement(&b);
    /// d.sort();
    /// assert_eq!(d, vec!["b".to_string(), "c".to_string()]);
    /// ```
    #[must_use]
    pub fn disagreement(&self, other: &Self) -> Vec<L> {
        if self.symbols == other.symbols {
            let labels = self.symbols.labels();
            (0..self.num_vars())
                .filter(|&i| field_at(&self.values, i) & field_at(&other.values, i) == 0)
                .map(|i| labels[i].clone())
                .collect()
        } else {
            // Merge-join over the two sorted-identity label sequences; only variables present on both
            // sides with disjoint fields can disagree (a single-sided variable reads as don't-care).
            let (la, lb) = (self.symbols.labels(), other.symbols.labels());
            let (sa, sb) = (self.symbols.sorted_order(), other.symbols.sorted_order());
            let (mut i, mut j) = (0usize, 0usize);
            let mut out = Vec::new();
            while i < sa.len() && j < sb.len() {
                let (ia, ib) = (sa[i] as usize, sb[j] as usize);
                match la[ia].identity(ia).cmp(&lb[ib].identity(ib)) {
                    Ordering::Less => i += 1,
                    Ordering::Greater => j += 1,
                    Ordering::Equal => {
                        if field_at(&self.values, ia) & field_at(&other.values, ib) == 0 {
                            out.push(la[ia].clone());
                        }
                        i += 1;
                        j += 1;
                    }
                }
            }
            out
        }
    }

    /// Enumerate every fully-assigned minterm covered by `self`, expressed over the `target` header.
    ///
    /// This is the inverse of minimisation ("maximise"): `self` is first re-expressed over `target`
    /// (variables of `target` absent from `self` become don't-care, variables of `self` absent from
    /// `target` are dropped — see [`project_onto`](Self::project_onto)), then every remaining don't-care
    /// is split into both polarities. The result is the `2^k` concrete minterms (where `k` is the number
    /// of don't-cares after projection), each assigning **every** variable in `target`, all sharing
    /// `target` as their canonical header (so they stay on the fast-comparison path and are usable as
    /// `BTreeSet`/`HashSet` keys). Expanding an already-maximal minterm over its own header is a no-op.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Minterm, Symbols};
    /// use std::collections::BTreeSet;
    ///
    /// let vars = Symbols::new(["a", "b"].iter().map(|s| s.to_string()).collect());
    /// // a fixed true, b don't-care → both polarities of b.
    /// let m = Minterm::from_symbols(vars.clone(), [Some(true), None]);
    /// let got: BTreeSet<_> = m.expand_over(&vars).into_iter().collect();
    /// let want: BTreeSet<_> = [
    ///     Minterm::from_symbols(vars.clone(), [Some(true), Some(false)]),
    ///     Minterm::from_symbols(vars.clone(), [Some(true), Some(true)]),
    /// ]
    /// .into_iter()
    /// .collect();
    /// assert_eq!(got, want);
    /// ```
    #[must_use]
    pub fn expand_over(&self, target: &Arc<Symbols<L>>) -> Vec<Minterm<L>> {
        let base = self.project_onto(target);
        let positions: Vec<usize> = (0..base.num_vars())
            .filter(|&i| field_at(&base.values, i) == FIELD_DC)
            .collect();
        let k = positions.len();
        let mut out = Vec::with_capacity(1usize << k);
        for mask in 0u64..(1u64 << k) {
            let mut words: Vec<u64> = base.values.to_vec();
            for (b, &pos) in positions.iter().enumerate() {
                let field = if (mask >> b) & 1 == 1 {
                    FIELD_TRUE
                } else {
                    FIELD_FALSE
                };
                let shift = (pos % VARS_PER_WORD) * 2;
                let wi = pos / VARS_PER_WORD;
                words[wi] = (words[wi] & !(0b11u64 << shift)) | ((field as u64) << shift);
            }
            out.push(Minterm::from_packed_words(Arc::clone(target), words.into()));
        }
        out
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

    // --- Requirement 3: Hamming distance / disagreement set ------------------------------------

    /// {a:1,b:0,c:1} vs {a:1,b:1,c:0} disagree on exactly {b, c}: distance 2.
    #[test]
    fn hamming_distance_and_disagreement_set() {
        let s = syms(&["a", "b", "c"]);
        let a = Minterm::from_symbols(Arc::clone(&s), [Some(true), Some(false), Some(true)]);
        let b = Minterm::from_symbols(Arc::clone(&s), [Some(true), Some(true), Some(false)]);

        assert_eq!(a.hamming_distance(&b), 2);

        let mut got = a.disagreement(&b);
        got.sort();
        assert_eq!(got, vec![Symbol::from("b"), Symbol::from("c")]);
        // The disagreement relation is symmetric.
        let mut got_rev = b.disagreement(&a);
        got_rev.sort();
        assert_eq!(got_rev, vec![Symbol::from("b"), Symbol::from("c")]);
    }

    /// Equal minterms are at distance 0 with an empty disagreement set.
    #[test]
    fn hamming_distance_zero_for_equal_minterms() {
        let s = syms(&["a", "b"]);
        let a = Minterm::from_symbols(Arc::clone(&s), [Some(true), Some(false)]);
        let b = Minterm::from_symbols(Arc::clone(&s), [Some(true), Some(false)]);
        assert_eq!(a.hamming_distance(&b), 0);
        assert!(a.disagreement(&b).is_empty());
    }

    /// `hamming_distance` always equals the cardinality of `disagreement`, exhaustively over a
    /// shared header.
    #[test]
    fn hamming_distance_equals_disagreement_len() {
        let s = syms(&["a", "b", "c"]);
        let opts = [Some(false), Some(true), None];
        for &x0 in &opts {
            for &x1 in &opts {
                for &x2 in &opts {
                    for &y0 in &opts {
                        for &y1 in &opts {
                            for &y2 in &opts {
                                let a = Minterm::from_symbols(Arc::clone(&s), [x0, x1, x2]);
                                let b = Minterm::from_symbols(Arc::clone(&s), [y0, y1, y2]);
                                assert_eq!(a.hamming_distance(&b), a.disagreement(&b).len());
                            }
                        }
                    }
                }
            }
        }
    }

    /// The shared-header (popcount fast) path and the differently-permuted-header (merge-join slow)
    /// path must give identical distance and disagreement sets. Mirrors `merge_path_matches_shared_path`.
    #[test]
    fn hamming_path_matches_shared_path() {
        // a = {a:1, b:0, c:1}; b = {a:1, b:1, c:0}.
        let shared = syms(&["a", "b", "c"]);
        let a_shared =
            Minterm::from_symbols(Arc::clone(&shared), [Some(true), Some(false), Some(true)]);
        let b_shared =
            Minterm::from_symbols(Arc::clone(&shared), [Some(true), Some(true), Some(false)]);
        // The same two functions over independent, differently-permuted headers (slow path).
        let a_perm =
            Minterm::from_symbols(syms(&["c", "a", "b"]), [Some(true), Some(true), Some(false)]);
        let b_perm =
            Minterm::from_symbols(syms(&["b", "c", "a"]), [Some(true), Some(false), Some(true)]);

        assert_eq!(a_shared, a_perm);
        assert_eq!(b_shared, b_perm);

        assert_eq!(
            a_shared.hamming_distance(&b_shared),
            a_perm.hamming_distance(&b_perm)
        );

        let mut d_shared = a_shared.disagreement(&b_shared);
        d_shared.sort();
        let mut d_perm = a_perm.disagreement(&b_perm);
        d_perm.sort();
        assert_eq!(d_shared, d_perm);
        assert_eq!(d_shared, vec![Symbol::from("b"), Symbol::from("c")]);
    }

    // --- Requirement 2: minterm expansion over an explicit variable set ------------------------

    /// `a` fixed, expanded over [a, b], splits b into both polarities: {a:1,b:0}, {a:1,b:1}.
    #[test]
    fn expand_over_splits_absent_dont_care() {
        let vars = syms(&["a", "b"]);
        // b is don't-care in the source.
        let m = Minterm::from_symbols(Arc::clone(&vars), [Some(true), None]);
        let got: std::collections::BTreeSet<_> = m.expand_over(&vars).into_iter().collect();
        let want: std::collections::BTreeSet<_> = [
            Minterm::from_symbols(Arc::clone(&vars), [Some(true), Some(false)]),
            Minterm::from_symbols(Arc::clone(&vars), [Some(true), Some(true)]),
        ]
        .into_iter()
        .collect();
        assert_eq!(got, want);
    }

    /// Widening over an absent variable c (present in `vars`, absent from the source header) splits c
    /// into both polarities, yielding 4 minterms.
    #[test]
    fn expand_over_widens_with_absent_variable() {
        let src = syms(&["a", "b"]);
        let target = syms(&["a", "b", "c"]);
        // a fixed true, b fixed false; c is absent from the source so it widens.
        let m = Minterm::from_symbols(src, [Some(true), Some(false)]);
        let got: std::collections::BTreeSet<_> = m.expand_over(&target).into_iter().collect();
        let want: std::collections::BTreeSet<_> = [
            Minterm::from_symbols(Arc::clone(&target), [Some(true), Some(false), Some(false)]),
            Minterm::from_symbols(Arc::clone(&target), [Some(true), Some(false), Some(true)]),
        ]
        .into_iter()
        .collect();
        assert_eq!(got, want);
        assert_eq!(got.len(), 2);
    }

    /// A fully-don't-care minterm over a 2-variable header yields the full 4-minterm cube.
    #[test]
    fn expand_over_dont_care_yields_full_cube() {
        let vars = syms(&["a", "b"]);
        let m = Minterm::from_symbols(Arc::clone(&vars), [None, None]);
        let got: std::collections::BTreeSet<_> = m.expand_over(&vars).into_iter().collect();
        let want: std::collections::BTreeSet<_> = [
            Minterm::from_symbols(Arc::clone(&vars), [Some(false), Some(false)]),
            Minterm::from_symbols(Arc::clone(&vars), [Some(false), Some(true)]),
            Minterm::from_symbols(Arc::clone(&vars), [Some(true), Some(false)]),
            Minterm::from_symbols(Arc::clone(&vars), [Some(true), Some(true)]),
        ]
        .into_iter()
        .collect();
        assert_eq!(got, want);
        assert_eq!(got.len(), 4);
    }

    /// Expanding an already-fully-assigned minterm over its own header is a no-op (one minterm, equal
    /// to itself).
    #[test]
    fn expand_over_is_idempotent_when_already_maximal() {
        let vars = syms(&["a", "b"]);
        let m = Minterm::from_symbols(Arc::clone(&vars), [Some(true), Some(false)]);
        let got = m.expand_over(&vars);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], m);
    }
}
