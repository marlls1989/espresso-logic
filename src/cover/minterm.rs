//! The unified [`Minterm`] type: a label-carrying row of tri-state values.
//!
//! A `Minterm` models one row of a Boolean cover — a value per variable, where each value is
//! `Some(true)` (1), `Some(false)` (0), or `None` (don't-care). Unlike a bare positional slice, a
//! `Minterm` **carries its variable labels** (via a shared `Symbols` table), so comparisons align
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
//! Labels live in a shared `Symbols` table (every cube of a cover shares one `Arc<Symbols<L>>`, so
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

use super::error::{DuplicateLabel, IndexOutOfRange, LabelNotFound};
use super::label::{Anonymous, Label, NamedLabel, StringLabel};
use super::symbols::{identity_union, Symbols};
use crate::impl_binary_operator;
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

/// Whether two value-set fields disagree: both must be non-empty and their value-sets must not
/// intersect (one fixes the variable `true`, the other `false`). A don't-care intersects either
/// polarity, and the empty literal (`00`) is never a disagreement — in particular a field never
/// disagrees with itself, so the disagreement relation stays reflexive even for `?`.
#[inline]
fn fields_disagree(a: u8, b: u8) -> bool {
    a != FIELD_EMPTY && b != FIELD_EMPTY && a & b == 0
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

/// Both-bit mask covering exactly the valid fields of word `word_idx` — the `allows-0` *and*
/// `allows-1` bit of every field below `num_vars`, and nothing above. Used by the element-wise
/// operators to re-zero the padding past the arity after combining, so the word-wise `Eq`/`Hash`
/// fast path stays canonical.
#[inline]
fn valid_field_mask(word_idx: usize, num_vars: usize) -> u64 {
    let count = (num_vars - word_idx * VARS_PER_WORD).min(VARS_PER_WORD);
    if count == VARS_PER_WORD {
        u64::MAX
    } else {
        (1u64 << (2 * count)) - 1
    }
}

/// Normalise every empty field (`00`, Espresso's `?`) in a packed word to don't-care (`11`), leaving
/// `01`/`10`/`11` untouched. This matches the public `Option<bool>` view's fold of `?`→`None`, so the
/// element-wise operators agree with it.
///
/// A field is empty exactly when neither of its bits is set. `is_empty` carries the per-field
/// "empty" flag on the `allows-0` (even) bit; OR-ing both that bit and its `allows-1` neighbour in
/// fills such fields to `11`. Padding past the arity (genuine `00`) is normalised too and must be
/// re-zeroed by the caller with [`valid_field_mask`].
#[inline]
fn normalise_empty(word: u64) -> u64 {
    let allows0 = word & ALLOWS0_MASK;
    let allows1 = (word >> 1) & ALLOWS0_MASK;
    let is_empty = !(allows0 | allows1) & ALLOWS0_MASK;
    word | is_empty | (is_empty << 1)
}

/// Element-wise three-valued AND of two packed words (all 32 fields in parallel), reading each field
/// as the value-set it allows. With `x0`/`y0` the `allows-0` bits and `x1`/`y1` the `allows-1` bits:
/// the result allows `1` only where both do (`x1 & y1`) and allows `0` where either does (`x0 | y0`).
#[inline]
fn word_and(x: u64, y: u64) -> u64 {
    let (x0, x1) = (x & ALLOWS0_MASK, (x >> 1) & ALLOWS0_MASK);
    let (y0, y1) = (y & ALLOWS0_MASK, (y >> 1) & ALLOWS0_MASK);
    let r0 = x0 | y0;
    let r1 = x1 & y1;
    (r1 << 1) | r0
}

/// Element-wise three-valued OR of two packed words (all 32 fields in parallel). The result allows
/// `1` where either operand does (`x1 | y1`) and allows `0` only where both do (`x0 & y0`).
#[inline]
fn word_or(x: u64, y: u64) -> u64 {
    let (x0, x1) = (x & ALLOWS0_MASK, (x >> 1) & ALLOWS0_MASK);
    let (y0, y1) = (y & ALLOWS0_MASK, (y >> 1) & ALLOWS0_MASK);
    let r0 = x0 & y0;
    let r1 = x1 | y1;
    (r1 << 1) | r0
}

/// Element-wise three-valued XOR of two packed words (all 32 fields in parallel). The result allows
/// `1` where the operands can differ (`(x1 & y0) | (x0 & y1)`) and allows `0` where they can agree
/// (`(x1 & y1) | (x0 & y0)`).
#[inline]
fn word_xor(x: u64, y: u64) -> u64 {
    let (x0, x1) = (x & ALLOWS0_MASK, (x >> 1) & ALLOWS0_MASK);
    let (y0, y1) = (y & ALLOWS0_MASK, (y >> 1) & ALLOWS0_MASK);
    let r0 = (x1 & y1) | (x0 & y0);
    let r1 = (x1 & y0) | (x0 & y1);
    (r1 << 1) | r0
}

/// Element-wise three-valued complement of a packed word (all 32 fields in parallel): swap each
/// field's `allows-0` and `allows-1` bits, so `{0}`↔`{1}` and `{0,1}` (don't-care) is fixed.
#[inline]
fn word_not(x: u64) -> u64 {
    ((x & ALLOWS0_MASK) << 1) | ((x >> 1) & ALLOWS0_MASK)
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
    /// Build a minterm from values against a shared `Symbols` table.
    ///
    /// `values` is read positionally against `symbols`; both describe the same number of variables.
    /// Minterms built from the *same* `Arc<Symbols>` compare via the pointer-equality fast path.
    #[must_use]
    pub(crate) fn from_symbols<I>(symbols: Arc<Symbols<L>>, values: I) -> Self
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
    pub(crate) fn symbols(&self) -> &Arc<Symbols<L>> {
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

    /// Set the value at positional index `i` in this minterm's own variable order, in place.
    ///
    /// The counterpart setter to [`value_at`](Self::value_at). Mutates the packed word buffer
    /// copy-on-write (via [`Arc::make_mut`]), so a minterm sharing its buffer with clones is cloned
    /// first and only the receiver is mutated; a uniquely-held buffer is mutated directly.
    ///
    /// # Errors
    ///
    /// Returns [`IndexOutOfRange`] if `i >= self.num_vars()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let mut m = Minterm::anonymous(&[Some(true), None]);
    /// m.set_value_at(1, Some(false)).unwrap();
    /// assert_eq!(m.value_at(1), Some(false));
    /// ```
    pub fn set_value_at(&mut self, i: usize, value: Option<bool>) -> Result<(), IndexOutOfRange> {
        let arity = self.num_vars();
        if i >= arity {
            return Err(IndexOutOfRange { index: i, arity });
        }
        let words = Arc::make_mut(&mut self.values);
        let word = i / VARS_PER_WORD;
        let shift = (i % VARS_PER_WORD) * 2;
        // `encode` always yields one of `01`/`10`/`11` (never the empty `00`) and only ever touches
        // this one field, so the canonical zero-padding past the arity that the word-wise `Eq`/`Hash`
        // fast path relies on is left untouched.
        words[word] &= !(0b11u64 << shift);
        words[word] |= (encode(value) as u64) << shift;
        Ok(())
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
    /// minterms align by position. For a named minterm use [`labeled`](Self::labeled) or
    /// [`with_labels`](Self::with_labels).
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

    /// Project this anonymous minterm onto a target of the given `arity`.
    ///
    /// Anonymous variables carry no labels, so alignment is by position ([`Anonymous`]'s identity is
    /// its index): `arity > num_vars` widens with trailing don't-cares, `arity < num_vars` drops the
    /// trailing positions, and `arity == num_vars` is a no-op. The named siblings
    /// [`project_to`](Self::project_to) and [`project_to_labels`](Self::project_to_labels) re-home by
    /// variable identity instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let m = Minterm::anonymous(&[Some(true), Some(false)]);
    /// // Widen: position 2 becomes a trailing don't-care.
    /// assert_eq!(m.project_to_arity(3).num_vars(), 3);
    /// // Contract: only position 0 survives.
    /// assert_eq!(m.project_to_arity(1).num_vars(), 1);
    /// // Equal arity is a no-op.
    /// assert_eq!(m.project_to_arity(2), m);
    /// ```
    #[must_use]
    pub fn project_to_arity(&self, arity: usize) -> Minterm<Anonymous> {
        self.project_onto(&Symbols::anonymous(arity))
    }
}

impl<L: Label> Minterm<L> {
    /// Shared core of [`labeled`](Self::labeled)/[`with_labels`](Self::with_labels): build over a fresh
    /// symbol table, proxying [`Symbols::new`]'s duplicate-identity check into the input-side
    /// [`DuplicateLabel::Input`] (a duplicate would collapse two columns onto one and drop a value).
    pub(crate) fn from_label_arcs(
        labels: Arc<[L]>,
        values: impl IntoIterator<Item = Option<bool>>,
    ) -> Result<Minterm<L>, DuplicateLabel> {
        let symbols = Symbols::new(labels).map_err(|e| DuplicateLabel::Input { index: e.index })?;
        Ok(Minterm::from_symbols(symbols, values))
    }

    /// Build a **labelled** minterm from `(label, value)` pairs.
    ///
    /// Each pair is `(label, value)` where `value` is `Some(true)`/`Some(false)`, or `None` for a
    /// don't-care. Pairing each label with its value makes a length mismatch unrepresentable. The
    /// labels need no particular order — a minterm aligns by variable [identity](crate::Label). Pair
    /// with an [`OutputSet`](super::OutputSet) of the same label style through
    /// [`Cube::new`](super::Cube::new) to build a labelled cube.
    ///
    /// Works for any label type — [`Symbol`](crate::Symbol), `String`, `u32`, … For `&str` names,
    /// [`with_labels`](Self::with_labels) avoids naming the label type at each pair.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateLabel`] if a label is repeated — the columns would otherwise collapse onto
    /// one and silently drop a value.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Minterm, Symbol};
    ///
    /// // a=1; b is not named, so it reads as a don't-care (a & -).
    /// let m = Minterm::<Symbol>::labeled(&[(Symbol::new("a"), Some(true))]).unwrap();
    /// assert_eq!(m.value_of("a"), Some(true));
    /// assert_eq!(m.value_of("b"), None);
    /// ```
    pub fn labeled(values: &[(L, Option<bool>)]) -> Result<Minterm<L>, DuplicateLabel> {
        Self::from_label_arcs(
            values.iter().map(|(l, _)| l.clone()).collect(),
            values.iter().map(|(_, v)| *v),
        )
    }
}

impl<L: StringLabel> Minterm<L> {
    /// Build a labelled minterm from `(name, value)` pairs, naming variables with any `&str`-like type.
    ///
    /// A string-name convenience over [`labeled`](Self::labeled): each label is built via `From<&str>`,
    /// so no string type is privileged (`&str`, `String`, `Arc<str>`, … all work). The label type is
    /// inferred from context (e.g. `Minterm::<Symbol>::with_labels`).
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateLabel`] if a name is repeated (see [`labeled`](Self::labeled)).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Minterm, Symbol};
    ///
    /// let m = Minterm::<Symbol>::with_labels(&[("a", Some(true)), ("b", Some(false))]).unwrap();
    /// assert_eq!(m.value_of("a"), Some(true));
    /// assert_eq!(m.value_of("b"), Some(false));
    /// ```
    pub fn with_labels<S: AsRef<str>>(
        values: &[(S, Option<bool>)],
    ) -> Result<Minterm<L>, DuplicateLabel> {
        Self::from_label_arcs(
            values.iter().map(|(s, _)| L::from(s.as_ref())).collect(),
            values.iter().map(|(_, v)| *v),
        )
    }

    /// Project this minterm onto a target variable *set* given by name.
    ///
    /// A string-name convenience over [`project_to_labels`](Self::project_to_labels): each name is
    /// built into a label via `From<&str>`, so no string type is privileged (`&str`, `String`,
    /// `Arc<str>`, … all work). Structurally re-homes the minterm onto `vars`: variables shared by
    /// `self` and the target keep their value, aligned by variable [`identity`](Label::identity) and
    /// reordered as needed; target-only variables come in as don't-care; self-only variables are
    /// dropped. Exactly one minterm out, defined over `vars`.
    ///
    /// `vars` names a set: repeats are deduplicated keeping the first occurrence (mirroring
    /// [`Cover::over_vars`](crate::Cover::over_vars) and [`Cube::expand_to`](crate::Cube::expand_to)).
    /// Anonymous covers carry no labels and project on arity via
    /// [`project_to_arity`](Minterm::project_to_arity).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Minterm, Symbol};
    ///
    /// let m = Minterm::<Symbol>::with_labels(&[("a", Some(true)), ("b", Some(false))]).unwrap();
    /// // Keep "b", widen with the target-only "c"; "a" is dropped.
    /// let p = m.project_to(["b", "c"]);
    /// assert_eq!(p.value_of("b"), Some(false));
    /// assert_eq!(p.value_of("c"), None);
    /// assert_eq!(p.vars(), &[Symbol::from("b"), Symbol::from("c")]);
    /// assert_eq!(p.num_vars(), 2);
    /// ```
    #[must_use]
    pub fn project_to<S: AsRef<str>>(&self, vars: impl IntoIterator<Item = S>) -> Minterm<L> {
        self.project_onto(&Symbols::deduped(
            vars.into_iter().map(|s| L::from(s.as_ref())),
        ))
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

    /// Set the value of a named variable, in place. The counterpart setter to
    /// [`value_of`](Self::value_of).
    ///
    /// Accepts any borrowed form of the label (so a `Minterm<Symbol>` can be set with `&str`). Unlike
    /// `value_of`, an absent label is always an error — even when `value` is `None` — so a typo in
    /// the label surfaces rather than silently doing nothing.
    ///
    /// # Errors
    ///
    /// Returns [`LabelNotFound`] if `label` is not present in this minterm.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Minterm, Symbol};
    ///
    /// let mut m = Minterm::<Symbol>::with_labels(&[("a", Some(true)), ("b", None)]).unwrap();
    /// m.set_value_of("b", Some(false)).unwrap();
    /// assert_eq!(m.value_of("b"), Some(false));
    /// assert!(m.set_value_of("c", Some(true)).is_err());
    /// ```
    pub fn set_value_of<Q>(&mut self, label: &Q, value: Option<bool>) -> Result<(), LabelNotFound>
    where
        L: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let i = self.symbols.index_of(label).ok_or(LabelNotFound)?;
        // `i` came straight from this minterm's own symbol table, so it is in range by
        // construction; `set_value_at` cannot fail here.
        self.set_value_at(i as usize, value)
            .expect("index from symbol table is in range");
        Ok(())
    }
}

impl<L: Label> Minterm<L> {
    /// Re-express this minterm over a `target` symbol table (the union-extend primitive).
    ///
    /// Variables of `target` absent from `self` become don't-care; variables of `self` absent from
    /// `target` are dropped. The result shares `target` as its symbol table. Alignment is by variable
    /// [`identity`](Label::identity): by name for labelled headers, by position for anonymous ones —
    /// one path for every `L: Label`. The public, `Arc`-free faces of this primitive are
    /// [`project_to`](Self::project_to), [`project_to_labels`](Self::project_to_labels) and
    /// [`project_to_arity`](Minterm::project_to_arity).
    #[must_use]
    pub(crate) fn project_onto(&self, target: &Arc<Symbols<L>>) -> Minterm<L> {
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

impl<L: NamedLabel> Minterm<L> {
    /// Project this minterm onto a target variable *set* given as label values.
    ///
    /// Structurally re-homes the minterm onto the named `vars`: variables shared by `self` and the
    /// target keep their value, aligned by variable [`identity`](Label::identity) and reordered as
    /// needed; target-only variables come in as don't-care; self-only variables are dropped. Exactly
    /// one minterm out, defined over `vars`.
    ///
    /// `vars` names a set: repeats are deduplicated keeping the first occurrence (mirroring
    /// [`Cover::over_vars`](crate::Cover::over_vars) and [`Cube::expand_to`](crate::Cube::expand_to)).
    /// The [`NamedLabel`] bound keeps this off `Minterm<Anonymous>` — anonymous covers carry no
    /// labels, so they project on arity via [`project_to_arity`](Minterm::project_to_arity). For
    /// `&str`-named headers, [`project_to`](Self::project_to) avoids naming the label type at each
    /// variable.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let m = Minterm::<u32>::labeled(&[(1, Some(true)), (2, Some(false))]).unwrap();
    /// // Keep variable 2, widen with the target-only variable 3; variable 1 is dropped.
    /// let p = m.project_to_labels([2, 3]);
    /// assert_eq!(p.value_of(&2), Some(false));
    /// assert_eq!(p.value_of(&3), None);
    /// ```
    #[must_use]
    pub fn project_to_labels(&self, vars: impl IntoIterator<Item = L>) -> Minterm<L> {
        self.project_onto(&Symbols::deduped(vars))
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
    /// minterms (e.g. the output of [`Cube::expand_to`](crate::Cube::expand_to)), where this is exactly
    /// the Hamming distance. `hamming_distance == disagreement().len()`.
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
                // Even bit set per variable whose field (on each side, and on the intersection) is
                // non-empty.
                let x_ne = (x & ALLOWS0_MASK) | ((x >> 1) & ALLOWS0_MASK);
                let y_ne = (y & ALLOWS0_MASK) | ((y >> 1) & ALLOWS0_MASK);
                let inter = x & y;
                let inter_ne = (inter & ALLOWS0_MASK) | ((inter >> 1) & ALLOWS0_MASK);
                let valid = valid_even_mask(k, n);
                // A variable disagrees only when both sides are non-empty yet their value-sets do
                // not intersect; an empty field (`?`) on either side is never a disagreement.
                count += (x_ne & y_ne & !inter_ne & valid).count_ones() as usize;
            }
            count
        } else {
            self.merged_fields(other)
                .filter(|&(sf, of)| fields_disagree(sf, of))
                .count()
        }
    }

    /// The variables on which `self` and `other` disagree (see [`hamming_distance`](Self::hamming_distance)
    /// for the disagreement rule), as a lazy [`Disagreement`] iterator that yields each disagreeing
    /// label (cloned) on demand. Ordering is unspecified.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let a = Minterm::<String>::with_labels(&[
    ///     ("a", Some(true)),
    ///     ("b", Some(false)),
    ///     ("c", Some(true)),
    /// ])
    /// .unwrap();
    /// let b = Minterm::<String>::with_labels(&[
    ///     ("a", Some(true)),
    ///     ("b", Some(true)),
    ///     ("c", Some(false)),
    /// ])
    /// .unwrap();
    /// let mut d: Vec<_> = a.disagreement(&b).collect();
    /// d.sort();
    /// assert_eq!(d, vec!["b".to_string(), "c".to_string()]);
    /// ```
    #[must_use]
    pub fn disagreement<'a>(&'a self, other: &'a Self) -> Disagreement<'a, L> {
        let state = if self.symbols == other.symbols {
            DisagreementState::SameHeader { pos: 0 }
        } else {
            DisagreementState::MergeJoin { i: 0, j: 0 }
        };
        Disagreement {
            a: self,
            b: other,
            state,
        }
    }

    /// Enumerate every fully-assigned minterm covered by `self`, expressed over the `target` header.
    ///
    /// This is the inverse of minimisation ("maximise"): `self` is first re-expressed over `target`
    /// (variables of `target` absent from `self` become don't-care, variables of `self` absent from
    /// `target` are dropped — the structural re-homing behind `project_to`), then every remaining don't-care
    /// is split into both polarities. The result is the `2^k` concrete minterms (where `k` is the number
    /// of don't-cares after projection), each assigning **every** variable in `target`, all sharing
    /// `target` as their canonical header (so they stay on the fast-comparison path and are usable as
    /// `BTreeSet`/`HashSet` keys). Expanding an already-maximal minterm over its own header is a no-op.
    ///
    /// Returns a lazy [`ExpandedMinterms`] iterator that packs each of the `2^k` minterms on demand,
    /// so a cube with many don't-cares costs O(1) memory rather than materialising the whole set. The
    /// public entry point is [`Cube::expand_to`](crate::Cube::expand_to).
    #[must_use]
    pub(crate) fn expand_over(&self, target: &Arc<Symbols<L>>) -> ExpandedMinterms<L> {
        // A cube with any empty literal (`?`) denotes the empty set, so it covers no minterm — a
        // vacuous (zero-length) expansion, short-circuited before the don't-care split rather than
        // copying the malformed field through. `base` is unread when `count == 0`.
        if self.has_empty_field() {
            return ExpandedMinterms {
                base: Arc::clone(&self.values),
                positions: Arc::from([]),
                target: Arc::clone(target),
                mask: 0,
                count: 0,
            };
        }
        let base = self.project_onto(target);
        let positions: Arc<[usize]> = (0..base.num_vars())
            .filter(|&i| field_at(&base.values, i) == FIELD_DC)
            .collect();
        let k = positions.len();
        // The expansion yields 2^k minterms. Guard `k` so the count neither overflows the `1 << k`
        // shift nor exceeds what a `usize` can address on this platform (the iterator reports its
        // length as a `usize`); beyond that the enumeration is not sane, so fail loudly rather than
        // wrap silently.
        assert!(
            k < usize::BITS as usize,
            "expand_over: {k} don't-care positions would expand to 2^{k} minterms, \
             exceeding the addressable range on this {}-bit platform",
            usize::BITS
        );
        ExpandedMinterms {
            base: Arc::clone(&base.values),
            positions,
            target: Arc::clone(target),
            mask: 0,
            count: 1u64 << k,
        }
    }
}

/// Lazy iterator over the concrete minterms of an expansion, created by
/// [`Cube::expand_to`](crate::Cube::expand_to).
///
/// Each `next()` packs one of the `2^k` minterms on demand (where `k` is the number of don't-care
/// positions after projection), so the full expansion is never materialised — expanding a cube with
/// many don't-cares costs O(1) memory rather than O(2^k). The number of minterms is known up front, so
/// this is an [`ExactSizeIterator`].
pub struct ExpandedMinterms<L> {
    /// The projected base words shared by every emitted minterm; the don't-care positions are patched
    /// per mask. Unread when `count == 0` (a vacuous cube).
    base: Arc<[u64]>,
    /// The don't-care positions (in `target`'s index space) split across the expansion.
    positions: Arc<[usize]>,
    /// Shared header every emitted minterm is defined over.
    target: Arc<Symbols<L>>,
    /// Next mask to emit, in `0..count`.
    mask: u64,
    /// Total number of minterms (`2^k`), or `0` for a vacuous (empty-field) cube.
    count: u64,
}

/// Opaque: the packed base words carry no useful `Debug`, so only the remaining count is shown.
impl<L> fmt::Debug for ExpandedMinterms<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpandedMinterms")
            .field("remaining", &(self.count - self.mask))
            .finish_non_exhaustive()
    }
}

impl<L> Iterator for ExpandedMinterms<L> {
    type Item = Minterm<L>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.mask >= self.count {
            return None;
        }
        let mask = self.mask;
        self.mask += 1;
        let mut words: Vec<u64> = self.base.to_vec();
        for (b, &pos) in self.positions.iter().enumerate() {
            let field = if (mask >> b) & 1 == 1 {
                FIELD_TRUE
            } else {
                FIELD_FALSE
            };
            let shift = (pos % VARS_PER_WORD) * 2;
            let wi = pos / VARS_PER_WORD;
            words[wi] = (words[wi] & !(0b11u64 << shift)) | ((field as u64) << shift);
        }
        Some(Minterm::from_packed_words(
            Arc::clone(&self.target),
            words.into(),
        ))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // `count` is bounded below `2^usize::BITS` by the guard in `expand_over`, so this fits `usize`.
        let remaining = usize::try_from(self.count - self.mask).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

// The remaining count is known exactly and in O(1).
impl<L> ExactSizeIterator for ExpandedMinterms<L> {}
impl<L> std::iter::FusedIterator for ExpandedMinterms<L> {}

/// Traversal state of a [`Disagreement`] iterator: one branch per header relationship.
enum DisagreementState {
    /// Shared header: scan positions `pos..num_vars`, yielding those whose fields disagree.
    SameHeader { pos: usize },
    /// Distinct headers: merge-join the two sorted-identity sequences at indices `(i, j)`.
    MergeJoin { i: usize, j: usize },
}

/// Lazy iterator over the variables on which two minterms disagree, created by
/// [`Minterm::disagreement`]. Each `next()` advances to the next disagreeing variable and yields its
/// cloned label; nothing is materialised up front. Ordering is unspecified.
pub struct Disagreement<'a, L> {
    a: &'a Minterm<L>,
    b: &'a Minterm<L>,
    state: DisagreementState,
}

/// Opaque: the borrowed minterms are elided (no `Debug` bound on `L`).
impl<L> fmt::Debug for Disagreement<'_, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Disagreement").finish_non_exhaustive()
    }
}

impl<L: Label> Iterator for Disagreement<'_, L> {
    type Item = L;

    fn next(&mut self) -> Option<L> {
        match &mut self.state {
            DisagreementState::SameHeader { pos } => {
                let labels = self.a.symbols.labels();
                while *pos < self.a.num_vars() {
                    let i = *pos;
                    *pos += 1;
                    if fields_disagree(field_at(&self.a.values, i), field_at(&self.b.values, i)) {
                        return Some(labels[i].clone());
                    }
                }
                None
            }
            DisagreementState::MergeJoin { i, j } => {
                // Only variables present on both sides with disjoint fields can disagree (a
                // single-sided variable reads as don't-care), so merge-join the two sorted-identity
                // label sequences and test the coincident ones.
                let (la, lb) = (self.a.symbols.labels(), self.b.symbols.labels());
                let (sa, sb) = (self.a.symbols.sorted_order(), self.b.symbols.sorted_order());
                while *i < sa.len() && *j < sb.len() {
                    let (ia, ib) = (sa[*i] as usize, sb[*j] as usize);
                    match la[ia].identity(ia).cmp(&lb[ib].identity(ib)) {
                        Ordering::Less => *i += 1,
                        Ordering::Greater => *j += 1,
                        Ordering::Equal => {
                            *i += 1;
                            *j += 1;
                            if fields_disagree(
                                field_at(&self.a.values, ia),
                                field_at(&self.b.values, ib),
                            ) {
                                return Some(la[ia].clone());
                            }
                        }
                    }
                }
                None
            }
        }
    }
}

// Both branches leave their cursors at the end once exhausted, so `None` is terminal.
impl<L: Label> std::iter::FusedIterator for Disagreement<'_, L> {}

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

impl<L: Label> Minterm<L> {
    /// Combine `self` and `other` element-wise with `word_op`, a three-valued operation over one
    /// packed word (all 32 fields in parallel).
    ///
    /// When both minterms share a header (pointer- or structurally-equal `Symbols`), the words are
    /// combined directly and the result reuses `self`'s header. Otherwise both operands are re-homed
    /// onto the [`identity_union`] of their headers — where a variable present in only one operand
    /// reads as don't-care (`-`) — so the combination is well-defined regardless of header ordering;
    /// this makes `a op b == b op a` under the crate's identity-based [`Eq`].
    ///
    /// Each operand word is [`normalise_empty`]d first (so an empty literal `?` behaves as `-`, matching
    /// the public `Option<bool>` view), and each result word is masked back to its valid fields with
    /// [`valid_field_mask`] so the padding past the arity stays zero.
    fn combine(&self, other: &Self, word_op: impl Fn(u64, u64) -> u64) -> Self {
        if self.symbols == other.symbols {
            let n = self.num_vars();
            let values: Arc<[u64]> = self
                .values
                .iter()
                .zip(other.values.iter())
                .enumerate()
                .map(|(k, (&x, &y))| {
                    word_op(normalise_empty(x), normalise_empty(y)) & valid_field_mask(k, n)
                })
                .collect();
            Minterm::from_packed_words(Arc::clone(&self.symbols), values)
        } else {
            // Re-home both onto the union header; absent variables become don't-care, then the shared
            // header hits the word-wise path above.
            let (union, _, _) = identity_union(&self.symbols, &other.symbols);
            let a = self.project_onto(&union);
            let b = other.project_onto(&union);
            a.combine(&b, word_op)
        }
    }

    /// Element-wise three-valued (Kleene) **AND** of two rows, aligned by variable identity, where
    /// `None` is the unknown/don't-care value `-`:
    ///
    /// ```text
    ///  &  0 1 -
    ///  0  0 0 0
    ///  1  0 1 -
    ///  -  0 - -
    /// ```
    ///
    /// AND shortcuts on a `0` (`0 & anything = 0`, including `0 & - = 0`), while `1 & - = -` and
    /// `- & - = -`. A variable present in only one operand reads as `-`, and the result is aligned by
    /// identity, so `a.and(&b) == b.and(&a)` even when the two carry differently-ordered headers.
    ///
    /// This is truth-value logic, **not** cube/set intersection: plain `x & y` on the raw value-set
    /// fields (what [`is_disjoint_with`](Self::is_disjoint_with) uses) is a different operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let a = Minterm::anonymous(&[Some(false), Some(true), None]);
    /// let b = Minterm::anonymous(&[None, None, Some(true)]);
    /// let r = a.and(&b);
    /// assert_eq!(r.value_at(0), Some(false)); // 0 & - = 0
    /// assert_eq!(r.value_at(1), None); //        1 & - = -
    /// assert_eq!(r.value_at(2), None); //        - & 1 = -
    /// ```
    #[must_use]
    pub fn and(&self, other: &Self) -> Self {
        self.combine(other, word_and)
    }

    /// Element-wise three-valued (Kleene) **OR** of two rows, aligned by variable identity, where
    /// `None` is the unknown/don't-care value `-`:
    ///
    /// ```text
    ///  |  0 1 -
    ///  0  0 1 -
    ///  1  1 1 1
    ///  -  - 1 -
    /// ```
    ///
    /// OR shortcuts on a `1` (`1 | anything = 1`, including `1 | - = 1`), while `0 | - = -` and
    /// `- | - = -`. A variable present in only one operand reads as `-`, and the result is aligned by
    /// identity, so `a.or(&b) == b.or(&a)` even when the two carry differently-ordered headers.
    ///
    /// This is truth-value logic, **not** cube/set intersection: plain `x & y` on the raw value-set
    /// fields (what [`is_disjoint_with`](Self::is_disjoint_with) uses) is a different operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let a = Minterm::anonymous(&[Some(true), Some(false), None]);
    /// let b = Minterm::anonymous(&[None, None, Some(false)]);
    /// let r = a.or(&b);
    /// assert_eq!(r.value_at(0), Some(true)); // 1 | - = 1
    /// assert_eq!(r.value_at(1), None); //       0 | - = -
    /// assert_eq!(r.value_at(2), None); //       - | 0 = -
    /// ```
    #[must_use]
    pub fn or(&self, other: &Self) -> Self {
        self.combine(other, word_or)
    }

    /// Element-wise three-valued (Kleene) **XOR** of two rows, aligned by variable identity, where
    /// `None` is the unknown/don't-care value `-`:
    ///
    /// ```text
    ///  ^  0 1 -
    ///  0  0 1 -
    ///  1  1 0 -
    ///  -  - - -
    /// ```
    ///
    /// XOR is unknown whenever either operand is unknown (`- ^ anything = -`, including `- ^ - = -`).
    /// A variable present in only one operand reads as `-`, and the result is aligned by identity, so
    /// `a.xor(&b) == b.xor(&a)` even when the two carry differently-ordered headers.
    ///
    /// This is truth-value logic, **not** cube/set intersection: plain `x & y` on the raw value-set
    /// fields (what [`is_disjoint_with`](Self::is_disjoint_with) uses) is a different operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let a = Minterm::anonymous(&[Some(true), Some(true), None]);
    /// let b = Minterm::anonymous(&[Some(false), Some(true), Some(false)]);
    /// let r = a.xor(&b);
    /// assert_eq!(r.value_at(0), Some(true)); //  1 ^ 0 = 1
    /// assert_eq!(r.value_at(1), Some(false)); // 1 ^ 1 = 0
    /// assert_eq!(r.value_at(2), None); //        - ^ 0 = -
    /// ```
    #[must_use]
    pub fn xor(&self, other: &Self) -> Self {
        self.combine(other, word_xor)
    }

    /// Element-wise three-valued (Kleene) **complement** of a row, where `None` is the
    /// unknown/don't-care value `-`:
    ///
    /// ```text
    ///  !  →
    ///  0    1
    ///  1    0
    ///  -    -
    /// ```
    ///
    /// A fixed value flips polarity; `-` is its own complement (`!- = -`), and an empty literal (`?`)
    /// reads as `-`, so `!? = -`. Equivalent to the unary `!` operator. The header is preserved.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::Minterm;
    ///
    /// let m = Minterm::anonymous(&[Some(false), Some(true), None]);
    /// let r = m.not();
    /// assert_eq!(r.value_at(0), Some(true)); //  !0 = 1
    /// assert_eq!(r.value_at(1), Some(false)); // !1 = 0
    /// assert_eq!(r.value_at(2), None); //        !- = -
    /// ```
    #[must_use]
    pub fn not(&self) -> Self {
        let n = self.num_vars();
        let values: Arc<[u64]> = self
            .values
            .iter()
            .enumerate()
            .map(|(k, &x)| word_not(normalise_empty(x)) & valid_field_mask(k, n))
            .collect();
        Minterm::from_packed_words(Arc::clone(&self.symbols), values)
    }
}

// The four owned/borrowed combinations of each three-valued bitwise operator, all delegating to the
// named `&self, &Self` method above, via the shared `impl_binary_operator!` macro (generic over
// `L: Label`).
impl_binary_operator!({L: Label} Minterm<L>, BitAnd, bitand, and);
impl_binary_operator!({L: Label} Minterm<L>, BitOr, bitor, or);
impl_binary_operator!({L: Label} Minterm<L>, BitXor, bitxor, xor);

// Unary complement, by value and by reference, both delegating to the inherent `not` method (the
// binary macro does not cover the unary case).
impl<L: Label> std::ops::Not for Minterm<L> {
    type Output = Minterm<L>;
    fn not(self) -> Minterm<L> {
        Minterm::not(&self)
    }
}

impl<L: Label> std::ops::Not for &Minterm<L> {
    type Output = Minterm<L>;
    fn not(self) -> Minterm<L> {
        Minterm::not(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol;

    fn syms(names: &[&str]) -> Arc<Symbols<Symbol>> {
        Symbols::new(names.iter().map(|s| Symbol::from(*s)).collect()).unwrap()
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

        let mut got: Vec<_> = a.disagreement(&b).collect();
        got.sort();
        assert_eq!(got, vec![Symbol::from("b"), Symbol::from("c")]);
        // The disagreement relation is symmetric.
        let mut got_rev: Vec<_> = b.disagreement(&a).collect();
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
        assert_eq!(a.disagreement(&b).count(), 0);
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
                                assert_eq!(a.hamming_distance(&b), a.disagreement(&b).count());
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
        let a_perm = Minterm::from_symbols(
            syms(&["c", "a", "b"]),
            [Some(true), Some(true), Some(false)],
        );
        let b_perm = Minterm::from_symbols(
            syms(&["b", "c", "a"]),
            [Some(true), Some(false), Some(true)],
        );

        assert_eq!(a_shared, a_perm);
        assert_eq!(b_shared, b_perm);

        assert_eq!(
            a_shared.hamming_distance(&b_shared),
            a_perm.hamming_distance(&b_perm)
        );

        let mut d_shared: Vec<_> = a_shared.disagreement(&b_shared).collect();
        d_shared.sort();
        let mut d_perm: Vec<_> = a_perm.disagreement(&b_perm).collect();
        d_perm.sort();
        assert_eq!(d_shared, d_perm);
        assert_eq!(d_shared, vec![Symbol::from("b"), Symbol::from("c")]);
    }

    /// A minterm carrying an empty literal (`?`) is at distance 0 from itself with an empty
    /// disagreement set: an empty field is never counted as a disagreement, so reflexivity holds.
    #[test]
    fn hamming_and_disagreement_reflexive_on_empty_literal() {
        let s = syms(&["a", "b", "c"]);
        let x = Minterm::from_symbols_input_fields(
            Arc::clone(&s),
            [InputField::One, InputField::Empty, InputField::Zero],
        );
        assert!(x.has_empty_field());
        assert_eq!(x.hamming_distance(&x), 0);
        assert_eq!(x.disagreement(&x).count(), 0);
    }

    // --- Requirement 2: minterm expansion over an explicit variable set ------------------------

    /// `a` fixed, expanded over [a, b], splits b into both polarities: {a:1,b:0}, {a:1,b:1}.
    #[test]
    fn expand_over_splits_absent_dont_care() {
        let vars = syms(&["a", "b"]);
        // b is don't-care in the source.
        let m = Minterm::from_symbols(Arc::clone(&vars), [Some(true), None]);
        let got: std::collections::BTreeSet<_> = m.expand_over(&vars).collect();
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
        let got: std::collections::BTreeSet<_> = m.expand_over(&target).collect();
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
        let got: std::collections::BTreeSet<_> = m.expand_over(&vars).collect();
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
        let got: Vec<_> = m.expand_over(&vars).collect();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], m);
    }

    /// A cube carrying an empty literal (`?`) denotes the empty set, so it expands to no minterms.
    #[test]
    fn expand_over_empty_literal_yields_no_minterms() {
        let vars = syms(&["a", "b"]);
        let m = Minterm::from_symbols_input_fields(
            Arc::clone(&vars),
            [InputField::Empty, InputField::DontCare],
        );
        assert!(m.has_empty_field());
        assert_eq!(m.expand_over(&vars).count(), 0);
    }

    /// Expanding past the don't-care guard fails loudly rather than overflowing the shift or
    /// truncating: 64 don't-care positions cannot be addressed within `usize`. The guard is eager
    /// (in `expand_over` itself), so the panic fires at construction, before any `next()`.
    #[test]
    #[should_panic(expected = "exceeding the addressable range")]
    fn expand_over_panics_past_capacity_guard() {
        let values = vec![None; 64];
        let m = Minterm::anonymous(&values);
        let _ = m.expand_over(m.symbols());
    }

    // --- Arc-free projection faces ------------------------------------------------------------

    /// Projecting onto a re-ordered variable set keeps each value aligned by name, not position.
    #[test]
    fn project_to_reorders_by_name() {
        let m =
            Minterm::<Symbol>::with_labels(&[("a", Some(true)), ("b", Some(false)), ("c", None)])
                .unwrap();
        // Target names the same variables in a different order.
        let p = m.project_to(["c", "b", "a"]);
        assert_eq!(
            p.vars(),
            &[Symbol::from("c"), Symbol::from("b"), Symbol::from("a")]
        );
        assert_eq!(p.value_of("a"), Some(true));
        assert_eq!(p.value_of("b"), Some(false));
        assert_eq!(p.value_of("c"), None);
    }

    /// A repeated variable in the target set is deduplicated, keeping the first occurrence.
    #[test]
    fn project_to_deduplicates_target_set() {
        let m = Minterm::<Symbol>::with_labels(&[("a", Some(true)), ("b", Some(false))]).unwrap();
        let p = m.project_to(["b", "b", "a"]);
        assert_eq!(p.num_vars(), 2);
        assert_eq!(p.vars(), &[Symbol::from("b"), Symbol::from("a")]);
        assert_eq!(p.value_of("b"), Some(false));
        assert_eq!(p.value_of("a"), Some(true));
    }

    /// An empty target set drops every variable, leaving a zero-width minterm.
    #[test]
    fn project_to_empty_set_yields_no_vars() {
        let m = Minterm::<Symbol>::with_labels(&[("a", Some(true)), ("b", Some(false))]).unwrap();
        let p = m.project_to(std::iter::empty::<&str>());
        assert_eq!(p.num_vars(), 0);
    }

    /// `project_to_arity` at the minterm's own arity yields an equal minterm.
    #[test]
    fn project_to_arity_equal_is_noop() {
        let m = Minterm::anonymous(&[Some(true), None, Some(false)]);
        assert_eq!(m.project_to_arity(3), m);
    }

    /// The public `project_to_labels` face is exactly the `project_onto` primitive over the same set.
    #[test]
    fn project_to_labels_equals_primitive() {
        let m = Minterm::<u32>::labeled(&[(1, Some(true)), (2, Some(false)), (3, None)]).unwrap();
        let vars = [2u32, 3, 4];
        let via_face = m.project_to_labels(vars);
        let via_primitive = m.project_onto(&Symbols::deduped(vars));
        assert_eq!(via_face, via_primitive);
    }

    // --- In-place setters: set_value_at / set_value_of ----------------------------------------

    /// Setting each of `Some(true)`, `Some(false)`, `None` at a position round-trips through
    /// `value_at`.
    #[test]
    fn set_value_at_roundtrips() {
        let mut m = Minterm::anonymous(&[Some(true), Some(false), None]);
        m.set_value_at(0, Some(false)).unwrap();
        assert_eq!(m.value_at(0), Some(false));
        m.set_value_at(1, None).unwrap();
        assert_eq!(m.value_at(1), None);
        m.set_value_at(2, Some(true)).unwrap();
        assert_eq!(m.value_at(2), Some(true));
    }

    /// A variable at index >= 32 lands in the second packed word; setting it must apply the
    /// correct word/shift maths across the 32-var word boundary.
    #[test]
    fn set_value_at_crosses_word_boundary() {
        let mut m = Minterm::anonymous(&[None; 40]);
        m.set_value_at(35, Some(true)).unwrap();
        assert_eq!(m.value_at(35), Some(true));
        // Neighbouring fields, in particular the last one of the first word, are untouched.
        assert_eq!(m.value_at(31), None);
        assert_eq!(m.value_at(34), None);
        assert_eq!(m.value_at(36), None);
    }

    /// Mutating one clone via `set_value_at` must not affect another clone sharing the same
    /// packed buffer: `Arc::make_mut` has to detach (copy-on-write) rather than mutate in place.
    #[test]
    fn set_value_at_copy_on_write_isolates_clones() {
        let original = Minterm::anonymous(&[Some(true), None, Some(false)]);
        let mut mutated = original.clone();
        mutated.set_value_at(1, Some(true)).unwrap();

        assert_eq!(original.value_at(1), None);
        assert_eq!(mutated.value_at(1), Some(true));
        assert_ne!(original, mutated);
        assert_eq!(
            original,
            Minterm::anonymous(&[Some(true), None, Some(false)])
        );
    }

    /// Overwriting a position that carries the empty literal (`?`, from the PLA read path) with a
    /// normal value must make it read back as that value, no longer as empty.
    #[test]
    fn set_value_at_overwrites_empty_field() {
        let s = syms(&["a", "b", "c"]);
        let mut m = Minterm::from_symbols_input_fields(
            Arc::clone(&s),
            [InputField::One, InputField::Empty, InputField::Zero],
        );
        assert!(m.has_empty_field());
        m.set_value_at(1, Some(true)).unwrap();
        assert!(!m.has_empty_field());
        assert_eq!(m.value_at(1), Some(true));
    }

    /// An out-of-range index is rejected with `IndexOutOfRange`, reporting the requested index and
    /// the row's actual arity.
    #[test]
    fn set_value_at_out_of_range_errors() {
        let mut m = Minterm::anonymous(&[Some(true), None]);
        let err = m.set_value_at(2, Some(false)).unwrap_err();
        assert_eq!(err, IndexOutOfRange { index: 2, arity: 2 });
    }

    /// `set_value_of` writes by name and round-trips through `value_of`.
    #[test]
    fn set_value_of_roundtrips() {
        let mut m = Minterm::<Symbol>::with_labels(&[("a", Some(true)), ("b", None)]).unwrap();
        m.set_value_of("b", Some(false)).unwrap();
        assert_eq!(m.value_of("b"), Some(false));
        m.set_value_of("a", None).unwrap();
        assert_eq!(m.value_of("a"), None);
    }

    /// `set_value_of` on a label absent from the row is an error, even when `value` is `None` — a
    /// typo in the label must surface rather than silently succeed.
    #[test]
    fn set_value_of_absent_label_errors() {
        let mut m = Minterm::<Symbol>::with_labels(&[("a", Some(true))]).unwrap();
        assert_eq!(m.set_value_of("missing", Some(true)), Err(LabelNotFound));
        assert_eq!(m.set_value_of("missing", None), Err(LabelNotFound));
    }

    /// A minterm mutated into a given value pattern equals (and hashes the same as) a freshly-built
    /// minterm of that pattern.
    #[test]
    fn set_value_at_matches_fresh_build_eq_and_hash() {
        use std::collections::HashSet;

        let s = syms(&["a", "b", "c"]);
        let mut mutated = Minterm::from_symbols(Arc::clone(&s), [Some(false), Some(false), None]);
        mutated.set_value_at(0, Some(true)).unwrap();
        mutated.set_value_at(2, Some(true)).unwrap();

        let fresh = Minterm::from_symbols(Arc::clone(&s), [Some(true), Some(false), Some(true)]);
        assert_eq!(mutated, fresh);

        let mut set = HashSet::new();
        set.insert(fresh);
        assert!(set.contains(&mutated));
    }

    /// Mutating a variable to `None` (don't-care) makes the minterm equal to one that simply omits
    /// that variable — the crate treats an absent variable as don't-care (mirrors
    /// `absent_variable_equals_dont_care`).
    #[test]
    fn set_value_at_to_dont_care_equals_omitted_variable() {
        let mut m = Minterm::from_symbols(syms(&["a", "b"]), [Some(true), Some(false)]);
        m.set_value_at(1, None).unwrap();

        let short = Minterm::from_symbols(syms(&["a"]), [Some(true)]);
        assert_eq!(m, short);
    }

    // --- Three-valued (Kleene) bitwise operators: & | ^ ---------------------------------------

    /// Scalar reference for three-valued AND (see the truth table on `Minterm::and`).
    fn ref_and(a: Option<bool>, b: Option<bool>) -> Option<bool> {
        match (a, b) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        }
    }

    /// Scalar reference for three-valued OR (see the truth table on `Minterm::or`).
    fn ref_or(a: Option<bool>, b: Option<bool>) -> Option<bool> {
        match (a, b) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        }
    }

    /// Scalar reference for three-valued XOR (see the truth table on `Minterm::xor`).
    fn ref_xor(a: Option<bool>, b: Option<bool>) -> Option<bool> {
        match (a, b) {
            (Some(x), Some(y)) => Some(x ^ y),
            _ => None,
        }
    }

    /// Exhaustive 3x3 check of every operator against its scalar reference, on 1-variable minterms.
    /// This is the primary catch for any SWAR mistake.
    #[test]
    fn ops_match_scalar_reference_exhaustively() {
        let opts = [None, Some(false), Some(true)];
        for &a in &opts {
            for &b in &opts {
                let ma = Minterm::anonymous(&[a]);
                let mb = Minterm::anonymous(&[b]);
                assert_eq!(ma.and(&mb).value_at(0), ref_and(a, b), "AND {a:?} {b:?}");
                assert_eq!(ma.or(&mb).value_at(0), ref_or(a, b), "OR {a:?} {b:?}");
                assert_eq!(ma.xor(&mb).value_at(0), ref_xor(a, b), "XOR {a:?} {b:?}");
            }
        }
    }

    /// The by-value/by-ref operator forms all delegate to the named methods.
    #[test]
    fn operator_forms_agree_with_methods() {
        let a = Minterm::anonymous(&[Some(true), None, Some(false)]);
        let b = Minterm::anonymous(&[Some(false), Some(true), None]);
        assert_eq!(a.clone() & b.clone(), a.and(&b));
        assert_eq!(&a & b.clone(), a.and(&b));
        assert_eq!(a.clone() & &b, a.and(&b));
        assert_eq!(&a & &b, a.and(&b));
        assert_eq!(&a | &b, a.or(&b));
        assert_eq!(&a ^ &b, a.xor(&b));
    }

    /// An empty literal (`?`, from the PLA read path) behaves as `-` under every operator, matching
    /// the public `Option<bool>` fold of `?`→`None`.
    #[test]
    fn empty_field_behaves_as_dont_care() {
        let s = syms(&["a"]);
        let empty = Minterm::from_symbols_input_fields(Arc::clone(&s), [InputField::Empty]);
        assert!(empty.has_empty_field());

        let f = Minterm::from_symbols(Arc::clone(&s), [Some(false)]);
        let t = Minterm::from_symbols(Arc::clone(&s), [Some(true)]);

        assert_eq!(empty.and(&f).value_at(0), Some(false)); // - & 0 = 0
        assert_eq!(empty.and(&t).value_at(0), None); //         - & 1 = -
        assert_eq!(empty.or(&t).value_at(0), Some(true)); //    - | 1 = 1
        assert_eq!(empty.or(&f).value_at(0), None); //          - | 0 = -
        assert_eq!(empty.xor(&f).value_at(0), None); //         - ^ 0 = -
        assert_eq!(empty.xor(&t).value_at(0), None); //         - ^ 1 = -
    }

    /// A reordered header (same variables, different order): the result aligns by identity.
    #[test]
    fn reordered_header_aligns_by_identity() {
        let a = Minterm::from_symbols(syms(&["a", "b"]), [Some(true), Some(false)]);
        let b = Minterm::from_symbols(syms(&["b", "a"]), [Some(true), Some(false)]);
        // a: {a:1, b:0}; b: {a:0, b:1}. AND ⇒ {a:0, b:0}.
        let r = a.and(&b);
        assert_eq!(r.value_of("a"), Some(false));
        assert_eq!(r.value_of("b"), Some(false));
    }

    /// A subset header: a variable missing from one operand is treated as `-`.
    #[test]
    fn subset_header_missing_var_is_dont_care() {
        let a = Minterm::from_symbols(syms(&["a", "b"]), [Some(false), Some(true)]);
        let b = Minterm::from_symbols(syms(&["a"]), [Some(true)]);
        // b has no "b", so b reads {a:1, b:-}. AND ⇒ {a:0, b:-}; OR ⇒ {a:1, b:1}.
        let and = a.and(&b);
        assert_eq!(and.value_of("a"), Some(false));
        assert_eq!(and.value_of("b"), None);
        let or = a.or(&b);
        assert_eq!(or.value_of("a"), Some(true));
        assert_eq!(or.value_of("b"), Some(true));
    }

    /// Disjoint headers: the union widens both, each absent variable reading as `-`.
    #[test]
    fn disjoint_headers_widen_to_union() {
        let a = Minterm::from_symbols(syms(&["a"]), [Some(false)]);
        let b = Minterm::from_symbols(syms(&["b"]), [Some(true)]);
        // Union {a, b}: a reads {a:0, b:-}; b reads {a:-, b:1}.
        let and = a.and(&b);
        assert_eq!(and.value_of("a"), Some(false)); // 0 & - = 0
        assert_eq!(and.value_of("b"), None); //         - & 1 = -
        let or = a.or(&b);
        assert_eq!(or.value_of("a"), None); //          0 | - = -
        assert_eq!(or.value_of("b"), Some(true)); //    - | 1 = 1
    }

    /// The same-header fast path and the differing-header merge/union path give identical results.
    #[test]
    fn same_header_and_union_paths_agree() {
        let shared = syms(&["a", "b", "c"]);
        let a_shared = Minterm::from_symbols(Arc::clone(&shared), [Some(true), None, Some(false)]);
        let b_shared = Minterm::from_symbols(Arc::clone(&shared), [Some(false), Some(true), None]);
        // The same two functions over independent, differently-permuted headers.
        let a_perm = Minterm::from_symbols(syms(&["c", "a", "b"]), [Some(false), Some(true), None]);
        let b_perm = Minterm::from_symbols(syms(&["b", "c", "a"]), [Some(true), None, Some(false)]);
        assert_eq!(a_shared, a_perm);
        assert_eq!(b_shared, b_perm);

        assert_eq!(a_shared.and(&b_shared), a_perm.and(&b_perm));
        assert_eq!(a_shared.or(&b_shared), a_perm.or(&b_perm));
        assert_eq!(a_shared.xor(&b_shared), a_perm.xor(&b_perm));
    }

    /// Operands wider than one 32-variable word: the operators combine across the word boundary.
    #[test]
    fn ops_cross_word_boundary() {
        let mut va = vec![None; 40];
        let mut vb = vec![None; 40];
        va[5] = Some(true);
        va[35] = Some(false);
        vb[5] = Some(false);
        vb[35] = Some(true);
        let a = Minterm::anonymous(&va);
        let b = Minterm::anonymous(&vb);

        let and = a.and(&b);
        assert_eq!(and.value_at(5), Some(false)); //  1 & 0 = 0
        assert_eq!(and.value_at(35), Some(false)); // 0 & 1 = 0
        assert_eq!(and.value_at(10), None); //        - & - = -
        let xor = a.xor(&b);
        assert_eq!(xor.value_at(5), Some(true)); //   1 ^ 0 = 1
        assert_eq!(xor.value_at(35), Some(true)); //  0 ^ 1 = 1
        assert_eq!(xor.value_at(36), None); //        - ^ - = -
    }

    /// Commutativity `a op b == b op a` for several nontrivial pairs, including differing headers.
    #[test]
    fn operators_are_commutative() {
        let a = Minterm::from_symbols(syms(&["a", "b", "c"]), [Some(true), None, Some(false)]);
        let b = Minterm::from_symbols(syms(&["a", "b", "c"]), [Some(false), Some(true), None]);
        assert_eq!(a.and(&b), b.and(&a));
        assert_eq!(a.or(&b), b.or(&a));
        assert_eq!(a.xor(&b), b.xor(&a));

        // Differing headers must still commute under identity-based equality.
        let p = Minterm::from_symbols(syms(&["x", "y"]), [Some(true), Some(false)]);
        let q = Minterm::from_symbols(syms(&["y", "z"]), [Some(true), None]);
        assert_eq!(p.and(&q), q.and(&p));
        assert_eq!(p.or(&q), q.or(&p));
        assert_eq!(p.xor(&q), q.xor(&p));
    }

    /// An operator result is in canonical form: it equals — and hashes identically to — a freshly
    /// built minterm of the expected pattern, so its padding past the arity is zero (a stray padding
    /// bit would break the word-wise `Eq`/`Hash` fast path).
    #[test]
    fn result_is_canonical_eq_and_hash() {
        use std::collections::HashSet;

        // 33 variables ⇒ two words, so the second word's padding (31 unused fields) is exercised.
        let mut va = vec![None; 33];
        let mut vb = vec![None; 33];
        va[0] = Some(true);
        vb[0] = Some(true);
        va[32] = Some(false);
        vb[32] = None;
        let a = Minterm::anonymous(&va);
        let b = Minterm::anonymous(&vb);
        let got = a.and(&b);

        let mut expected = vec![None; 33];
        expected[0] = Some(true); //  1 & 1 = 1
        expected[32] = Some(false); // 0 & - = 0
        let fresh = Minterm::anonymous(&expected);
        assert_eq!(got, fresh);
        assert_eq!(got.raw_words(), fresh.raw_words());

        let mut set = HashSet::new();
        set.insert(fresh);
        assert!(set.contains(&got));
    }

    /// The full complement truth table, including `!- = -` and the `?` empty literal reading as `-`.
    #[test]
    fn not_truth_table() {
        assert_eq!(
            Minterm::anonymous(&[Some(false)]).not().value_at(0),
            Some(true)
        );
        assert_eq!(
            Minterm::anonymous(&[Some(true)]).not().value_at(0),
            Some(false)
        );
        assert_eq!(Minterm::anonymous(&[None]).not().value_at(0), None);

        // An empty literal (`?`) reads as `-`, so its complement is `-`.
        let empty = Minterm::from_symbols_input_fields(syms(&["a"]), [InputField::Empty]);
        assert!(empty.has_empty_field());
        assert_eq!(empty.not().value_at(0), None);
    }

    /// The by-value and by-reference `!` operator forms both delegate to the inherent `not`.
    #[test]
    fn not_operator_forms_agree_with_method() {
        let m = Minterm::anonymous(&[Some(true), None, Some(false)]);
        assert_eq!(!m.clone(), m.not());
        assert_eq!(!&m, m.not());
    }

    /// Double complement is the identity for defined values, and `!!- == -`.
    #[test]
    fn not_is_self_inverse() {
        let m = Minterm::anonymous(&[Some(false), Some(true), None]);
        assert_eq!(m.not().not(), m);
    }

    /// Complement across the 32-variable word boundary.
    #[test]
    fn not_crosses_word_boundary() {
        let mut v = vec![None; 40];
        v[5] = Some(true);
        v[35] = Some(false);
        let m = Minterm::anonymous(&v);
        let r = m.not();
        assert_eq!(r.value_at(5), Some(false));
        assert_eq!(r.value_at(35), Some(true));
        assert_eq!(r.value_at(10), None);
    }

    /// A complement result is canonical: it equals — and hashes identically to — a freshly built
    /// minterm of the complemented pattern, so the padding past the arity stays zero.
    #[test]
    fn not_result_is_canonical_eq_and_hash() {
        use std::collections::HashSet;

        // 33 variables ⇒ two words, exercising the second word's padding.
        let mut v = vec![None; 33];
        v[0] = Some(true);
        v[32] = Some(false);
        let got = Minterm::anonymous(&v).not();

        let mut expected = vec![None; 33];
        expected[0] = Some(false);
        expected[32] = Some(true);
        let fresh = Minterm::anonymous(&expected);
        assert_eq!(got, fresh);
        assert_eq!(got.raw_words(), fresh.raw_words());

        let mut set = HashSet::new();
        set.insert(fresh);
        assert!(set.contains(&got));
    }
}
