//! [`OutputSet`]: a cube's output-membership as a compact binary bitmap.
//!
//! A cube belongs to exactly one of a cover's sets (ON/`F`, don't-care/`D`, OFF/`R`); its *outputs*
//! record, per output column, whether the cube asserts that output **in its set**. That is inherently
//! two-state (asserted / not) and positional, so — unlike the tri-state input [`Minterm`](super::Minterm)
//! — it needs only **one bit per output**, packed 64 to a `u64` word. This is the Rust-side
//! representation; at the Espresso boundary outputs are decoded and encoded one bit at a time (the C cube
//! packs 32 bits to a word at a non-word-aligned offset, so its layout does not match these `u64` words).
//! Whole words are copied verbatim only when re-homing one `OutputSet` onto another `Symbols<O>` of equal
//! arity.
//!
//! Output **labels** are still meaningful (named `.ob` outputs, relabelling), so `OutputSet` keeps a
//! shared `Symbols<O>` handle — the same one the cover holds. The bit packing itself is independent of
//! the label type `O`, so re-homing onto another `Symbols<O>` of the same arity is an `Arc` clone.

use super::error::{DuplicateLabel, IndexOutOfRange, LabelNotFound};
use super::label::{Anonymous, Label, StringLabel};
use super::symbols::{identity_union, Symbols};
use crate::impl_binary_operator;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Outputs packed per `u64` word (1 bit each).
const OUTPUTS_PER_WORD: usize = 64;

#[inline]
fn words_for(num_vars: usize) -> usize {
    num_vars.div_ceil(OUTPUTS_PER_WORD)
}

/// Bitmask of the valid (in-range) output bits within word `word` of a `num_vars`-output row: all-ones
/// for a full word below the arity, the low `num_vars % 64` bits for the final partial word (all-ones
/// when the arity divides evenly), and zero for words wholly past the arity. Complementing through it
/// (`!bits & mask`) keeps the padding bits past the arity zero.
#[inline]
fn valid_bits_mask(num_vars: usize, word: usize) -> u64 {
    let full_words = num_vars / OUTPUTS_PER_WORD;
    match word.cmp(&full_words) {
        // A word wholly below the arity is entirely valid; a word wholly past it is entirely padding.
        Ordering::Less => u64::MAX,
        Ordering::Greater => 0,
        // The final partial word keeps only its low `num_vars % 64` bits. When the arity divides
        // evenly there is no partial word (`words_for` stops at `full_words`), so this is unreached.
        Ordering::Equal => (1u64 << (num_vars % OUTPUTS_PER_WORD)) - 1,
    }
}

/// A label-carrying output-membership bitmap: bit *i* set ⇔ the cube asserts output *i* in its set.
///
/// Simpler than a tri-state [`Minterm`](super::Minterm): outputs are two-state (asserted / not),
/// positional, one bit each.
///
/// # Examples
///
/// An `OutputSet` is read from a [`Cube`](super::Cube)'s [`outputs`](super::Cube::outputs); each bit
/// records whether the cube asserts that output column in its set.
///
/// ```
/// use espresso_logic::{Cube, CubeType};
///
/// // Two inputs (the second a don't-care), three outputs; assert outputs 0 and 2.
/// let cube = Cube::anonymous(&[Some(true), None], &[true, false, true], CubeType::F);
/// let outputs = cube.outputs();
///
/// assert_eq!(outputs.num_vars(), 3);
/// assert!(outputs.value_at(0));
/// assert!(!outputs.value_at(1));
/// assert!(outputs.value_at(2));
/// assert!(!outputs.value_at(3)); // out-of-range reads false
/// assert_eq!(outputs.iter().collect::<Vec<bool>>(), vec![true, false, true]);
/// assert_eq!(outputs.to_string(), "101"); // bare 1/0 membership row
/// ```
#[derive(Clone)]
pub struct OutputSet<O> {
    symbols: Arc<Symbols<O>>,
    /// One bit per output, 64 outputs per word; padding bits past the arity are zero.
    bits: Arc<[u64]>,
}

impl<O> OutputSet<O> {
    /// Build from a shared symbol table and a per-output "asserted?" sequence (read positionally).
    pub(crate) fn from_symbols<I>(symbols: Arc<Symbols<O>>, asserted: I) -> Self
    where
        I: IntoIterator<Item = bool>,
    {
        let mut bits = vec![0u64; words_for(symbols.arity())];
        for (i, a) in asserted.into_iter().enumerate() {
            if a {
                bits[i / OUTPUTS_PER_WORD] |= 1u64 << (i % OUTPUTS_PER_WORD);
            }
        }
        OutputSet {
            symbols,
            bits: bits.into(),
        }
    }

    /// Build from an already-packed bit buffer, taken verbatim (the inverse of [`packed`](Self::packed)).
    /// The buffer must hold exactly `words_for(arity)` words with zero padding past the final output.
    /// Crate-internal; used to decode an Espresso cube and to re-home onto another `Symbols` table.
    pub(crate) fn from_packed_bits(symbols: Arc<Symbols<O>>, bits: Arc<[u64]>) -> Self {
        debug_assert_eq!(
            bits.len(),
            words_for(symbols.arity()),
            "packed word count must match the output arity"
        );
        OutputSet { symbols, bits }
    }

    /// Whether output `i` is asserted. Out-of-range indices read as `false`.
    #[must_use]
    pub fn value_at(&self, i: usize) -> bool {
        i < self.num_vars() && (self.bits[i / OUTPUTS_PER_WORD] >> (i % OUTPUTS_PER_WORD)) & 1 != 0
    }

    /// Set whether output `i` is asserted, in place.
    ///
    /// The positional counterpart of [`value_at`](Self::value_at); [`set_value_of`](Self::set_value_of)
    /// is the by-label form.
    ///
    /// # Errors
    ///
    /// Returns [`IndexOutOfRange`] if `i` is at or past [`num_vars`](Self::num_vars) — rows are dense,
    /// so there is no output to set.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::OutputSet;
    ///
    /// let mut outputs = OutputSet::anonymous(&[false, false]);
    /// outputs.set_value_at(1, true).unwrap();
    /// assert!(!outputs.value_at(0) && outputs.value_at(1));
    /// ```
    pub fn set_value_at(&mut self, i: usize, asserted: bool) -> Result<(), IndexOutOfRange> {
        if i >= self.num_vars() {
            return Err(IndexOutOfRange {
                index: i,
                arity: self.num_vars(),
            });
        }
        // Copy-on-write: clone the backing words only if another `OutputSet` shares them. Only the
        // bit for a validated index `i < num_vars()` is ever touched, so padding bits past the arity
        // stay zero (the invariant `from_packed_bits`/equality rely on).
        let words = Arc::make_mut(&mut self.bits);
        let word = i / OUTPUTS_PER_WORD;
        let bit = i % OUTPUTS_PER_WORD;
        if asserted {
            words[word] |= 1u64 << bit;
        } else {
            words[word] &= !(1u64 << bit);
        }
        Ok(())
    }

    /// The number of output columns.
    #[must_use]
    pub fn num_vars(&self) -> usize {
        self.symbols.arity()
    }

    /// Iterate the per-output "asserted?" flags in index order.
    pub fn iter(&self) -> impl Iterator<Item = bool> + '_ {
        (0..self.num_vars()).map(move |i| self.value_at(i))
    }

    /// The shared symbol table these outputs are defined over.
    #[must_use]
    pub(crate) fn symbols(&self) -> &Arc<Symbols<O>> {
        &self.symbols
    }

    /// The output labels, in index order.
    #[must_use]
    pub fn vars(&self) -> &[O] {
        self.symbols.labels()
    }

    /// The packed bit words, for re-homing onto another [`Symbols`] table of the same arity (the packing
    /// is independent of the label type). This is the Rust-side `u64` packing, not the C cube's output
    /// layout, which is packed and unpacked one bit at a time at the Espresso boundary.
    pub(crate) fn packed(&self) -> &Arc<[u64]> {
        &self.bits
    }
}

impl<O: Label> OutputSet<O> {
    /// Merge-join the two output sets' membership flags, aligned by variable identity in sorted-label
    /// order.
    ///
    /// Yields `(self_asserted, other_asserted)` per output of the union; an output absent from one side
    /// reads as `false` (unasserted) — the boolean analogue of the don't-care padding
    /// [`Minterm`](super::Minterm) uses. O(n+m) over the two sorted label sequences — no union set, no
    /// widening. Callers that share a symbol table take the faster word-wise path directly instead of
    /// this.
    fn merged_bits<'a>(&'a self, other: &'a Self) -> MergedBits<'a, O> {
        MergedBits {
            a: self,
            b: other,
            sa: self.symbols.sorted_order(),
            sb: other.symbols.sorted_order(),
            i: 0,
            j: 0,
        }
    }
}

/// Two output sets are equal when they assert the **same set of output labels**, aligned by variable
/// [identity](crate::Label) rather than by column position. A shared header (pointer- or
/// identity-equal `Symbols`) takes the word-wise fast path; otherwise the two are merge-joined over
/// the union of their labels, an output absent from one side reading as unasserted (`0`). So an output
/// set equals a longer one that differs only by unasserted outputs (e.g. `{f:1}` equals `{f:1, g:0}`,
/// but not `{f:1, g:1}`). Within a cover every cube shares one output `Symbols`, so this reduces to
/// the by-column comparison there.
impl<O: Label> PartialEq for OutputSet<O> {
    fn eq(&self, other: &Self) -> bool {
        if self.symbols == other.symbols {
            // Same layout: equal membership packs to identical words.
            self.bits == other.bits
        } else {
            self.merged_bits(other).all(|(a, b)| a == b)
        }
    }
}

impl<O: Label> Eq for OutputSet<O> {}

/// Hashes the same identity-aligned canonical sequence that [`Eq`]/[`Ord`] compare over, so
/// `a == b` always implies `hash(a) == hash(b)`.
///
/// Equality aligns outputs by [`identity`](Label::identity), reading an absent output as unasserted
/// (`0`), and is independent of physical word/position order. We therefore hash, in sorted-identity
/// order, the identity of every output that **is** asserted — skipping the unasserted outputs that
/// `Eq` treats as absent — then the count of asserted outputs, so a shorter set cannot collide with a
/// longer one that asserts strictly more. The raw words, the `Arc` pointer and the arity that `Eq`
/// ignores are deliberately left out, preserving the `Hash`/`Eq` contract.
impl<O: Label> Hash for OutputSet<O> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let labels = self.symbols.labels();
        let mut len = 0usize;
        for &pos in self.symbols.sorted_order() {
            let pos = pos as usize;
            if self.value_at(pos) {
                // Walk sorted-identity order so the sequence is canonical regardless of header
                // ordering; unasserted outputs are skipped to match `Eq`'s absent-equals-`0` rule.
                labels[pos].identity(pos).hash(state);
                len += 1;
            }
        }
        len.hash(state);
    }
}

impl<O: Label> PartialOrd for OutputSet<O> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Total order over the sorted union of output identities, an absent output reading as unasserted
/// (`false < true`), independent of header ordering — so `OutputSet` keys a `BTreeSet`/`BTreeMap`
/// consistently with the identity-based [`Eq`]. Computed by a single O(n+m) merge of the two sorted
/// label sequences.
impl<O: Label> Ord for OutputSet<O> {
    fn cmp(&self, other: &Self) -> Ordering {
        for (a, b) in self.merged_bits(other) {
            match a.cmp(&b) {
                Ordering::Equal => continue,
                non_eq => return non_eq,
            }
        }
        Ordering::Equal
    }
}

/// Merge-join iterator backing [`OutputSet::merged_bits`]; an output absent from one side reads as
/// unasserted (`false`).
struct MergedBits<'a, O> {
    a: &'a OutputSet<O>,
    b: &'a OutputSet<O>,
    sa: &'a [u32],
    sb: &'a [u32],
    i: usize,
    j: usize,
}

impl<O: Label> Iterator for MergedBits<'_, O> {
    type Item = (bool, bool);

    fn next(&mut self) -> Option<(bool, bool)> {
        let la = self.a.symbols.labels();
        let lb = self.b.symbols.labels();
        match (self.sa.get(self.i), self.sb.get(self.j)) {
            (Some(&ia), Some(&ib)) => match la[ia as usize]
                .identity(ia as usize)
                .cmp(&lb[ib as usize].identity(ib as usize))
            {
                Ordering::Less => {
                    self.i += 1;
                    Some((self.a.value_at(ia as usize), false))
                }
                Ordering::Greater => {
                    self.j += 1;
                    Some((false, self.b.value_at(ib as usize)))
                }
                Ordering::Equal => {
                    self.i += 1;
                    self.j += 1;
                    Some((self.a.value_at(ia as usize), self.b.value_at(ib as usize)))
                }
            },
            (Some(&ia), None) => {
                self.i += 1;
                Some((self.a.value_at(ia as usize), false))
            }
            (None, Some(&ib)) => {
                self.j += 1;
                Some((false, self.b.value_at(ib as usize)))
            }
            (None, None) => None,
        }
    }
}

/// Renders the membership as a bare `1`/`0` row in index order. Needs no bound on `O` (positional).
///
/// This is the raw per-output membership, *not* the formatted PLA output field: the PLA writer emits
/// `2`/`0`/`~` per [`CubeType`](super::CubeType) for `fd`/`fr`/`fdr` covers; this bare `1`/`0` row is the
/// `f`-type rendering.
impl<O> fmt::Display for OutputSet<O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for asserted in self.iter() {
            f.write_str(if asserted { "1" } else { "0" })?;
        }
        Ok(())
    }
}

/// Positional debug (`OutputSet { 0: 1, 1: 0 }`) — no label bound, mirroring the bare-bits view.
impl<O> fmt::Debug for OutputSet<O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OutputSet {{")?;
        for (i, asserted) in self.iter().enumerate() {
            let sep = if i == 0 { "" } else { "," };
            write!(f, "{sep} {i}: {}", u8::from(asserted))?;
        }
        write!(f, " }}")
    }
}

impl OutputSet<Anonymous> {
    /// Build an **anonymous** (positional) output set from a per-output membership slice.
    ///
    /// `membership[i]` says whether the cube asserts output `i` **in its set** (the cube's
    /// [`CubeType`](super::CubeType)); the arity is `membership.len()`. This is the output-side
    /// counterpart of [`Minterm::anonymous`](super::Minterm::anonymous), and pairs with it to build a
    /// positional cube through [`Cube::new`](super::Cube::new).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::OutputSet;
    ///
    /// // assert outputs 0 and 2 of three.
    /// let outputs = OutputSet::anonymous(&[true, false, true]);
    /// assert_eq!(outputs.num_vars(), 3);
    /// assert!(outputs.value_at(0) && !outputs.value_at(1) && outputs.value_at(2));
    /// ```
    #[must_use]
    pub fn anonymous(membership: &[bool]) -> Self {
        OutputSet::from_symbols(
            Symbols::<Anonymous>::anonymous(membership.len()),
            membership.iter().copied(),
        )
    }
}

impl<O: Label> OutputSet<O> {
    /// Shared core of [`labeled`](Self::labeled)/[`with_labels`](Self::with_labels): build over a fresh
    /// symbol table, proxying [`Symbols::new`]'s duplicate-identity check into the output-side
    /// [`DuplicateLabel::Output`] (a duplicate would collapse two columns onto one).
    pub(crate) fn from_label_arcs(
        labels: Arc<[O]>,
        asserted: impl IntoIterator<Item = bool>,
    ) -> Result<OutputSet<O>, DuplicateLabel> {
        let symbols =
            Symbols::new(labels).map_err(|e| DuplicateLabel::Output { index: e.index })?;
        Ok(OutputSet::from_symbols(symbols, asserted))
    }

    /// Build a **labelled** output set from `(label, asserted)` pairs.
    ///
    /// Each pair is `(label, asserted)`, where `asserted` says whether the cube asserts that output
    /// **in its set** (an `F` cube asserts the ON-set outputs, a `D` cube the don't-care outputs, an
    /// `R` cube the OFF-set outputs). Pairing each label with its flag makes a length mismatch
    /// unrepresentable. The labels need no particular order — outputs align by variable
    /// [identity](crate::Label). Pair with a [`Minterm`](super::Minterm) of the same label style
    /// through [`Cube::new`](super::Cube::new) to build a labelled cube.
    ///
    /// Works for any label type — [`Symbol`](crate::Symbol), `String`, `u32`, … For `&str` names,
    /// [`with_labels`](Self::with_labels) avoids naming the label type at each pair.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateLabel`] if a label is repeated — the columns would otherwise collapse onto
    /// one.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{OutputSet, Symbol};
    ///
    /// // assert output `f`, not output `g`.
    /// let outputs = OutputSet::<Symbol>::labeled(
    ///     &[(Symbol::new("f"), true), (Symbol::new("g"), false)],
    /// )
    /// .unwrap();
    /// assert_eq!(outputs.num_vars(), 2);
    /// assert!(outputs.value_at(0) && !outputs.value_at(1));
    /// ```
    pub fn labeled(outputs: &[(O, bool)]) -> Result<OutputSet<O>, DuplicateLabel> {
        Self::from_label_arcs(
            outputs.iter().map(|(l, _)| l.clone()).collect(),
            outputs.iter().map(|(_, a)| *a),
        )
    }
}

impl<O: StringLabel> OutputSet<O> {
    /// Build a labelled output set from `(name, asserted)` pairs, naming outputs with any `&str`-like
    /// type.
    ///
    /// A string-name convenience over [`labeled`](Self::labeled): each label is built via `From<&str>`,
    /// so no string type is privileged (`&str`, `String`, `Arc<str>`, … all work). The label type is
    /// inferred from context (e.g. `OutputSet::<Symbol>::with_labels`).
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateLabel`] if a name is repeated (see [`labeled`](Self::labeled)).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{OutputSet, Symbol};
    ///
    /// let outputs = OutputSet::<Symbol>::with_labels(&[("f", true), ("g", false)]).unwrap();
    /// assert!(outputs.value_at(0) && !outputs.value_at(1));
    /// ```
    pub fn with_labels<S: AsRef<str>>(
        outputs: &[(S, bool)],
    ) -> Result<OutputSet<O>, DuplicateLabel> {
        Self::from_label_arcs(
            outputs.iter().map(|(s, _)| O::from(s.as_ref())).collect(),
            outputs.iter().map(|(_, a)| *a),
        )
    }
}

impl<O: Label> OutputSet<O> {
    /// Whether the named output is asserted (`false` if the label is absent — an absent output is
    /// unasserted).
    ///
    /// The by-label counterpart of [`value_at`](Self::value_at). Accepts any borrowed form of the
    /// label (so an `OutputSet<Symbol>` can be queried with `&str`).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{OutputSet, Symbol};
    ///
    /// let outputs = OutputSet::<Symbol>::with_labels(&[("f", true), ("g", false)]).unwrap();
    /// assert!(outputs.value_of("f"));
    /// assert!(!outputs.value_of("g"));
    /// assert!(!outputs.value_of("missing"));
    /// ```
    #[must_use]
    pub fn value_of<Q>(&self, label: &Q) -> bool
    where
        O: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        match self.symbols.index_of(label) {
            Some(i) => self.value_at(i as usize),
            None => false,
        }
    }

    /// Set whether the named output is asserted, in place.
    ///
    /// The by-label counterpart of [`set_value_at`](Self::set_value_at). Accepts any borrowed form
    /// of the label (so an `OutputSet<Symbol>` can be set with `&str`).
    ///
    /// # Errors
    ///
    /// Returns [`LabelNotFound`] if `label` is not present — even when `asserted` is `false`, since
    /// there is no output column to (not) set.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{OutputSet, Symbol};
    ///
    /// let mut outputs = OutputSet::<Symbol>::with_labels(&[("f", false), ("g", false)]).unwrap();
    /// outputs.set_value_of("f", true).unwrap();
    /// assert!(outputs.value_of("f"));
    /// assert!(outputs.set_value_of("missing", true).is_err());
    /// ```
    pub fn set_value_of<Q>(&mut self, label: &Q, asserted: bool) -> Result<(), LabelNotFound>
    where
        O: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let i = self.symbols.index_of(label).ok_or(LabelNotFound)?;
        self.set_value_at(i as usize, asserted)
            .expect("index_of only returns indices within the row's arity");
        Ok(())
    }
}

impl<O> OutputSet<O> {
    /// Element-wise binary complement: flip every output's asserted bit over the row's own arity, so an
    /// asserted output becomes unasserted and vice versa. Equivalent to the unary [`!`](std::ops::Not)
    /// operator.
    ///
    /// The operation is **binary**, not `None`-aware: an `OutputSet` output is two-state (asserted or
    /// not), so this is a plain bitwise complement of the packed membership bitmap — unlike
    /// [`Minterm`](super::Minterm), whose don't-care state gives it a three-valued complement. Padding
    /// bits past the arity are masked off, so they stay zero. Needs no label bound — the complement
    /// touches only the membership bits and reuses the shared header.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::OutputSet;
    ///
    /// let a = OutputSet::anonymous(&[true, false, true]);
    /// assert_eq!(a.not(), OutputSet::anonymous(&[false, true, false]));
    /// ```
    #[must_use]
    pub fn not(&self) -> Self {
        let num_vars = self.num_vars();
        let bits: Arc<[u64]> = self
            .bits
            .iter()
            .enumerate()
            .map(|(word, &bits)| !bits & valid_bits_mask(num_vars, word))
            .collect();
        OutputSet::from_packed_bits(Arc::clone(&self.symbols), bits)
    }
}

impl<O: Label> OutputSet<O> {
    /// Widen this set's asserted outputs onto a `words`-word zero bitmap, re-homing each output by the
    /// old→union position `map` (the per-operand remap vector from [`identity_union`]). Mirrors the
    /// [`assert_mask`](super::assert_mask) widening in [`Cover`](super::Cover): only mapped positions
    /// are set, so an absent output stays `0` and padding past the union arity stays zero.
    fn widen(&self, words: usize, map: &[usize]) -> Vec<u64> {
        // `map` has one entry per output of this operand (its remap vector), so enumerating it walks
        // every old position exactly once.
        let mut bits = vec![0u64; words];
        for (old, &new) in map.iter().enumerate() {
            if self.value_at(old) {
                bits[new / OUTPUTS_PER_WORD] |= 1u64 << (new % OUTPUTS_PER_WORD);
            }
        }
        bits
    }

    /// Combine two output sets word-by-word under `word_op`, aligning outputs by identity.
    ///
    /// A shared header (pointer-equal or identity-equal `Symbols`) combines the packed words directly,
    /// reusing that header. Otherwise both operands are widened onto the identity-union header (see
    /// [`identity_union`]) before the word-wise op; an output present in only one operand reads as `0`
    /// on the absent side. Padding past the arity is zero in both operands, and `word_op` maps
    /// `(0, 0)` to `0` for `&`/`|`/`^`, so the result's padding stays zero.
    fn combine(&self, other: &Self, word_op: impl Fn(u64, u64) -> u64) -> Self {
        if self.symbols == other.symbols {
            let bits: Arc<[u64]> = self
                .bits
                .iter()
                .zip(other.bits.iter())
                .map(|(&a, &b)| word_op(a, b))
                .collect();
            return OutputSet::from_packed_bits(Arc::clone(&self.symbols), bits);
        }
        let (union, self_map, other_map) = identity_union(&self.symbols, &other.symbols);
        let words = words_for(union.arity());
        let lhs = self.widen(words, &self_map);
        let rhs = other.widen(words, &other_map);
        let bits: Arc<[u64]> = lhs
            .iter()
            .zip(rhs.iter())
            .map(|(&a, &b)| word_op(a, b))
            .collect();
        OutputSet::from_packed_bits(union, bits)
    }

    /// Element-wise binary AND of two output sets: an output is asserted in the result iff it is
    /// asserted in **both** operands. Equivalent to the [`&`](std::ops::BitAnd) operator.
    ///
    /// The operation is **binary**, not `None`-aware: an `OutputSet` output is two-state (asserted or
    /// not), so this is a plain bitwise AND of the packed membership bitmaps — unlike
    /// [`Minterm`](super::Minterm), whose don't-care state gives it three-valued operators. Outputs are
    /// aligned by variable [identity](crate::Label) via the identity union of the two headers; an output
    /// present in only one operand reads as `0` (unasserted) on the absent side, matching
    /// [`value_at`](Self::value_at)/[`value_of`](Self::value_of), and by the same identity alignment
    /// that `OutputSet`'s [`Eq`] uses.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::OutputSet;
    ///
    /// let a = OutputSet::anonymous(&[true, false]);
    /// let b = OutputSet::anonymous(&[true, true]);
    /// assert_eq!(a.and(&b), OutputSet::anonymous(&[true, false]));
    /// ```
    #[must_use]
    pub fn and(&self, other: &Self) -> Self {
        self.combine(other, |a, b| a & b)
    }

    /// Element-wise binary OR of two output sets: an output is asserted in the result iff it is
    /// asserted in **either** operand. Equivalent to the [`|`](std::ops::BitOr) operator.
    ///
    /// The operation is **binary**, not `None`-aware: an `OutputSet` output is two-state (asserted or
    /// not), so this is a plain bitwise OR of the packed membership bitmaps — unlike
    /// [`Minterm`](super::Minterm), whose don't-care state gives it three-valued operators. Outputs are
    /// aligned by variable [identity](crate::Label) via the identity union of the two headers; an output
    /// present in only one operand reads as `0` (unasserted) on the absent side, matching
    /// [`value_at`](Self::value_at)/[`value_of`](Self::value_of), and by the same identity alignment
    /// that `OutputSet`'s [`Eq`] uses.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::OutputSet;
    ///
    /// let a = OutputSet::anonymous(&[true, false]);
    /// let b = OutputSet::anonymous(&[true, true]);
    /// assert_eq!(a.or(&b), OutputSet::anonymous(&[true, true]));
    /// ```
    #[must_use]
    pub fn or(&self, other: &Self) -> Self {
        self.combine(other, |a, b| a | b)
    }

    /// Element-wise binary XOR of two output sets: an output is asserted in the result iff it is
    /// asserted in **exactly one** operand. Equivalent to the [`^`](std::ops::BitXor) operator.
    ///
    /// The operation is **binary**, not `None`-aware: an `OutputSet` output is two-state (asserted or
    /// not), so this is a plain bitwise XOR of the packed membership bitmaps — unlike
    /// [`Minterm`](super::Minterm), whose don't-care state gives it three-valued operators. Outputs are
    /// aligned by variable [identity](crate::Label) via the identity union of the two headers; an output
    /// present in only one operand reads as `0` (unasserted) on the absent side, so XOR over the union
    /// yields the symmetric difference (an output asserted in just one operand stays asserted), by the
    /// same identity alignment that `OutputSet`'s [`Eq`] uses.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::OutputSet;
    ///
    /// let a = OutputSet::anonymous(&[true, false]);
    /// let b = OutputSet::anonymous(&[true, true]);
    /// assert_eq!(a.xor(&b), OutputSet::anonymous(&[false, true]));
    /// ```
    #[must_use]
    pub fn xor(&self, other: &Self) -> Self {
        self.combine(other, |a, b| a ^ b)
    }
}

// Implement each binary bitwise operator for every owned/borrowed combination of operands, all
// delegating to the named `&self, &Self` [`OutputSet`] method, via the shared `impl_binary_operator!`
// macro. The `Label` bound aligns the operands by identity (see [`OutputSet::and`]).
impl_binary_operator!({O: Label} OutputSet<O>, BitAnd, bitand, and);
impl_binary_operator!({O: Label} OutputSet<O>, BitOr, bitor, or);
impl_binary_operator!({O: Label} OutputSet<O>, BitXor, bitxor, xor);

// The unary complement, provided by value and by reference, both delegating to the inherent `not`. It
// needs no `Label` bound (the complement only touches bits/arity), so it is written by hand rather than
// through `impl_binary_operator!`.
impl<O> std::ops::Not for OutputSet<O> {
    type Output = OutputSet<O>;
    fn not(self) -> OutputSet<O> {
        OutputSet::not(&self)
    }
}

impl<O> std::ops::Not for &OutputSet<O> {
    type Output = OutputSet<O>;
    fn not(self) -> OutputSet<O> {
        OutputSet::not(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash_of<O: Label>(o: &OutputSet<O>) -> u64 {
        let mut h = DefaultHasher::new();
        o.hash(&mut h);
        h.finish()
    }

    #[test]
    fn eq_and_hash_ignore_construction_path_and_padding() {
        // 65 outputs, only output 64 asserted — the single set bit lives in the *second* word, so this
        // exercises both the multi-word packing and the zero-padding of the unused high bits.
        let mut membership = vec![false; 65];
        membership[64] = true;
        let from_iter = OutputSet::<Anonymous>::anonymous(&membership);

        // Rebuild from the packed words through the other constructor: must compare and hash equal.
        let symbols = Symbols::<Anonymous>::anonymous(membership.len());
        let from_packed = OutputSet::from_packed_bits(symbols, Arc::clone(from_iter.packed()));
        assert_eq!(from_iter, from_packed);
        assert_eq!(hash_of(&from_iter), hash_of(&from_packed));

        // Padding above the arity must be zero, so the second word holds exactly bit 0 (output 64).
        assert_eq!(from_iter.packed().len(), 2);
        assert_eq!(from_iter.packed()[1], 1u64);
    }

    #[test]
    fn value_at_and_iter_track_multi_word_packing() {
        let mut membership = vec![false; 70];
        for &i in &[0usize, 64, 69] {
            membership[i] = true;
        }
        let o = OutputSet::<Anonymous>::anonymous(&membership);

        assert_eq!(o.num_vars(), 70);
        assert!(o.value_at(0) && o.value_at(64) && o.value_at(69));
        assert!(!o.value_at(1) && !o.value_at(63) && !o.value_at(65));
        assert!(!o.value_at(70), "an index at the arity reads false");
        assert!(!o.value_at(1000), "a far out-of-range index reads false");
        assert_eq!(o.iter().collect::<Vec<bool>>(), membership);
    }

    #[test]
    fn ord_runs_lexicographically_over_the_sorted_identity_union() {
        // Ordering runs over the sorted union of positions with an absent output reading as unasserted
        // (`false < true`), so it turns on the lowest position where the two differ, not on arity.
        let bit0 = OutputSet::<Anonymous>::anonymous(&[true, false]);
        let bit1 = OutputSet::<Anonymous>::anonymous(&[false, true]);
        // Position 0 differs first: `bit0` asserts it, `bit1` does not, so `bit0` orders after `bit1`.
        assert!(bit1 < bit0);
        assert_ne!(bit0, bit1);

        // A shorter set equals a longer one that only adds unasserted outputs — they are `Equal`, not
        // ordered by arity.
        let short = OutputSet::<Anonymous>::anonymous(&[true]);
        let padded = OutputSet::<Anonymous>::anonymous(&[true, false]);
        assert_eq!(short.cmp(&padded), Ordering::Equal);
        assert_eq!(short, padded);
    }

    #[test]
    fn reordered_header_compares_and_hashes_equal() {
        use crate::Symbol;

        // The same outputs asserted identically, but the two headers list their labels in opposite
        // order. Positional comparison would call these unequal; identity alignment must see through
        // the reordering.
        let a =
            OutputSet::<Symbol>::with_labels(&[("f", true), ("g", false), ("h", true)]).unwrap();
        let b =
            OutputSet::<Symbol>::with_labels(&[("h", true), ("g", false), ("f", true)]).unwrap();

        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));
    }

    #[test]
    fn absent_output_reads_as_unasserted_in_eq_and_hash() {
        use crate::Symbol;

        // `{f:1}` equals `{f:1, g:0}` — the extra output is unasserted, which is how an absent output
        // reads. It does not equal `{f:1, g:1}`.
        let one = OutputSet::<Symbol>::with_labels(&[("f", true)]).unwrap();
        let padded = OutputSet::<Symbol>::with_labels(&[("f", true), ("g", false)]).unwrap();
        let asserted = OutputSet::<Symbol>::with_labels(&[("f", true), ("g", true)]).unwrap();

        assert_eq!(one, padded);
        assert_eq!(hash_of(&one), hash_of(&padded));
        assert_ne!(one, asserted);
    }

    #[test]
    fn anonymous_alignment_reduces_to_positional() {
        // `Anonymous`'s identity is its position, so identity alignment gives the same answers as the
        // old positional comparison for equal-arity operands.
        let a = OutputSet::<Anonymous>::anonymous(&[true, false, true]);
        let b = OutputSet::<Anonymous>::anonymous(&[true, false, true]);
        let c = OutputSet::<Anonymous>::anonymous(&[true, true, true]);

        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));
        assert_ne!(a, c);
    }

    #[test]
    fn differing_header_operators_are_commutative_under_eq() {
        use crate::Symbol;

        // Partially overlapping headers (`g` shared, `f`/`h` one-sided). The identity-based `Eq` now
        // sees the two column orderings `identity_union` produces as equal, so commutativity holds
        // under plain `==`.
        let a = OutputSet::<Symbol>::with_labels(&[("f", true), ("g", true)]).unwrap();
        let b = OutputSet::<Symbol>::with_labels(&[("g", true), ("h", true)]).unwrap();

        assert_eq!(&a & &b, &b & &a);
        assert_eq!(&a | &b, &b | &a);
        assert_eq!(&a ^ &b, &b ^ &a);
    }

    #[test]
    fn ord_is_a_total_order_usable_as_a_btreeset_key() {
        use crate::Symbol;
        use std::collections::BTreeSet;

        // Two headers listing the same asserted labels in different orders are equal by identity, so a
        // `BTreeSet` deduplicates them to a single element.
        let a = OutputSet::<Symbol>::with_labels(&[("f", true), ("g", false)]).unwrap();
        let b = OutputSet::<Symbol>::with_labels(&[("g", false), ("f", true)]).unwrap();
        let c = OutputSet::<Symbol>::with_labels(&[("f", true), ("g", true)]).unwrap();

        let mut set = BTreeSet::new();
        set.insert(a);
        set.insert(b); // equal-by-identity to `a`, so dedupes away
        set.insert(c);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn in_cover_cube_outputs_compare_unchanged() {
        use crate::{Cover, CoverType, Cube, CubeType};

        // Within a cover every cube shares the one output header, so the identity path lands on the
        // word-wise fast path and behaves exactly as before. Two cubes asserting the same outputs
        // compare equal; a differing one does not.
        let cubes = [
            Cube::anonymous(&[Some(true), None], &[true, false], CubeType::F),
            Cube::anonymous(&[Some(false), None], &[true, false], CubeType::F),
            Cube::anonymous(&[None, Some(true)], &[false, true], CubeType::F),
        ];
        let cover = Cover::from_cubes(CoverType::F, cubes.iter().cloned());
        let rows: Vec<_> = cover.cubes().collect();
        assert_eq!(rows[0].outputs(), rows[1].outputs());
        assert_ne!(rows[0].outputs(), rows[2].outputs());
    }

    #[test]
    fn set_value_at_round_trips_through_value_at_and_value_of() {
        use crate::Symbol;

        let mut o = OutputSet::<Symbol>::with_labels(&[("f", false), ("g", false)]).unwrap();
        assert!(!o.value_at(0) && !o.value_of("f"));

        o.set_value_at(0, true).unwrap();
        assert!(o.value_at(0) && o.value_of("f"));

        o.set_value_at(0, false).unwrap();
        assert!(!o.value_at(0) && !o.value_of("f"));
    }

    #[test]
    fn set_value_at_crosses_the_word_boundary() {
        // 80 outputs: index 70 lives in the second 64-bit word.
        let mut o = OutputSet::<Anonymous>::anonymous(&[false; 80]);
        o.set_value_at(70, true).unwrap();

        assert!(o.value_at(70));
        assert!(!o.value_at(69) && !o.value_at(71));
        assert_eq!(o.packed().len(), 2);
        assert_eq!(o.packed()[1], 1u64 << (70 - 64));
    }

    #[test]
    fn set_value_at_is_copy_on_write() {
        let original = OutputSet::<Anonymous>::anonymous(&[false, false]);
        let mut mutated = original.clone();
        mutated.set_value_at(1, true).unwrap();

        // The clone diverges from the original: the original's bit is untouched.
        assert!(!original.value_at(1));
        assert!(mutated.value_at(1));
        assert_ne!(original, mutated);
        assert_eq!(original, OutputSet::<Anonymous>::anonymous(&[false, false]));
    }

    #[test]
    fn set_value_at_out_of_range_reports_index_and_arity() {
        let mut o = OutputSet::<Anonymous>::anonymous(&[false, false]);
        let err = o.set_value_at(2, true).unwrap_err();
        assert_eq!(err, IndexOutOfRange { index: 2, arity: 2 });
    }

    #[test]
    fn set_value_of_absent_label_errors_even_when_clearing() {
        use crate::Symbol;

        let mut o = OutputSet::<Symbol>::with_labels(&[("f", true)]).unwrap();
        assert_eq!(o.set_value_of("missing", false), Err(LabelNotFound));
        assert_eq!(o.set_value_of("missing", true), Err(LabelNotFound));
    }

    #[test]
    fn value_of_on_absent_label_is_false() {
        use crate::Symbol;

        let o = OutputSet::<Symbol>::with_labels(&[("f", true)]).unwrap();
        assert!(!o.value_of("missing"));
    }

    #[test]
    fn setting_then_clearing_the_highest_output_leaves_padding_zero() {
        // 65 outputs: the highest valid index (64) is the sole occupant of the second word.
        let mut o = OutputSet::<Anonymous>::anonymous(&[false; 65]);
        o.set_value_at(64, true).unwrap();
        o.set_value_at(64, false).unwrap();

        // Must compare and hash equal to a fresh all-`false` build — a stray padding bit would break
        // both, since `Eq`/`Hash` compare the packed words directly.
        let fresh = OutputSet::<Anonymous>::anonymous(&[false; 65]);
        assert_eq!(o, fresh);
        assert_eq!(hash_of(&o), hash_of(&fresh));
        assert_eq!(o.packed()[1], 0u64);
    }

    #[test]
    fn same_header_and_or_xor_are_per_output_bitwise() {
        let a = OutputSet::<Anonymous>::anonymous(&[true, false]);
        let b = OutputSet::<Anonymous>::anonymous(&[true, true]);

        assert_eq!(a.and(&b), OutputSet::<Anonymous>::anonymous(&[true, false]));
        assert_eq!(a.or(&b), OutputSet::<Anonymous>::anonymous(&[true, true]));
        assert_eq!(a.xor(&b), OutputSet::<Anonymous>::anonymous(&[false, true]));
    }

    #[test]
    fn operator_forms_agree_with_the_named_methods() {
        // Exercise all four owned/borrowed combos the macro generates against the inherent method.
        let a = OutputSet::<Anonymous>::anonymous(&[true, false]);
        let b = OutputSet::<Anonymous>::anonymous(&[true, true]);
        let expected = a.and(&b);

        assert_eq!(a.clone() & b.clone(), expected);
        assert_eq!(&a & b.clone(), expected);
        assert_eq!(a.clone() & &b, expected);
        assert_eq!(&a & &b, expected);

        assert_eq!(&a | &b, a.or(&b));
        assert_eq!(&a ^ &b, a.xor(&b));
    }

    #[test]
    fn differing_labels_align_by_identity_with_absent_reading_zero() {
        use crate::Symbol;

        // Partially overlapping named outputs: `g` is shared, `f`/`h` sit on one side each. The union
        // header is built self-first (`f`, `g`) then extended with `other`'s new labels (`h`).
        let a = OutputSet::<Symbol>::with_labels(&[("f", true), ("g", true)]).unwrap();
        let b = OutputSet::<Symbol>::with_labels(&[("g", false), ("h", true)]).unwrap();

        // AND: only `g` overlaps, and it is asserted in `a` but not `b`, so nothing survives. Outputs
        // present in only one operand read `0` on the absent side.
        let and = a.and(&b);
        assert_eq!(and.num_vars(), 3);
        assert!(!and.value_of("f") && !and.value_of("g") && !and.value_of("h"));

        // OR overlays both — this is the identity union the OFF-set/`Cover::merge` output overlay uses.
        let or = a.or(&b);
        assert_eq!(or.num_vars(), 3);
        assert!(or.value_of("f") && or.value_of("g") && or.value_of("h"));

        // XOR yields the symmetric difference: `f` (only in `a`) and `h` (only in `b`) survive; `g`,
        // asserted in `a` and absent-as-`0` in `b`, is also asserted.
        let xor = a.xor(&b);
        assert_eq!(xor.num_vars(), 3);
        assert!(xor.value_of("f") && xor.value_of("g") && xor.value_of("h"));
    }

    #[test]
    fn xor_symmetric_difference_clears_shared_asserted_outputs() {
        use crate::Symbol;

        // Both assert the shared output `g`; XOR must clear it (asserted on both sides), while the
        // one-sided `f`/`h` survive.
        let a = OutputSet::<Symbol>::with_labels(&[("f", true), ("g", true)]).unwrap();
        let b = OutputSet::<Symbol>::with_labels(&[("g", true), ("h", true)]).unwrap();

        let xor = a.xor(&b);
        assert!(xor.value_of("f") && !xor.value_of("g") && xor.value_of("h"));
    }

    #[test]
    fn ops_cross_the_word_boundary() {
        // 70 outputs: bits span two words. Assert overlapping and disjoint high/low outputs.
        let mut ma = vec![false; 70];
        let mut mb = vec![false; 70];
        for &i in &[0usize, 64, 69] {
            ma[i] = true;
        }
        for &i in &[0usize, 65, 69] {
            mb[i] = true;
        }
        let a = OutputSet::<Anonymous>::anonymous(&ma);
        let b = OutputSet::<Anonymous>::anonymous(&mb);

        let and: Vec<bool> = a.and(&b).iter().collect();
        let or: Vec<bool> = a.or(&b).iter().collect();
        let xor: Vec<bool> = a.xor(&b).iter().collect();
        for i in 0..70 {
            assert_eq!(and[i], ma[i] && mb[i], "AND output {i}");
            assert_eq!(or[i], ma[i] || mb[i], "OR output {i}");
            assert_eq!(xor[i], ma[i] ^ mb[i], "XOR output {i}");
        }
    }

    #[test]
    fn ops_leave_padding_zero() {
        // 65 outputs: the sole high output (64) is the only occupant of the second word. Comparing
        // against a freshly-built expected value would fail on any stray padding bit, since `Eq`/`Hash`
        // read the packed words directly.
        let mut ma = vec![false; 65];
        let mut mb = vec![false; 65];
        ma[64] = true;
        mb[0] = true;
        let a = OutputSet::<Anonymous>::anonymous(&ma);
        let b = OutputSet::<Anonymous>::anonymous(&mb);

        let expected = OutputSet::<Anonymous>::anonymous(&{
            let mut m = vec![false; 65];
            m[0] = true;
            m[64] = true;
            m
        });
        let or = a.or(&b);
        assert_eq!(or, expected);
        assert_eq!(hash_of(&or), hash_of(&expected));
        assert_eq!(
            or.packed()[1],
            1u64,
            "only output 64's bit is set in the high word"
        );
    }

    #[test]
    fn ops_are_commutative_for_same_and_differing_headers() {
        use crate::Symbol;

        // Same header.
        let a = OutputSet::<Anonymous>::anonymous(&[true, false, true]);
        let b = OutputSet::<Anonymous>::anonymous(&[false, true, true]);
        assert_eq!(a.and(&b), b.and(&a));
        assert_eq!(a.or(&b), b.or(&a));
        assert_eq!(a.xor(&b), b.xor(&a));

        // Differing headers: `identity_union` builds the union self-first, so `x.op(y)` and `y.op(x)`
        // lay their columns out in different orders. Now that `Eq` aligns by output identity, the
        // operators are commutative under plain `==` regardless of that column order.
        let c = OutputSet::<Symbol>::with_labels(&[("f", true), ("g", true)]).unwrap();
        let d = OutputSet::<Symbol>::with_labels(&[("g", false), ("h", true)]).unwrap();
        assert_eq!(c.and(&d), d.and(&c));
        assert_eq!(c.or(&d), d.or(&c));
        assert_eq!(c.xor(&d), d.xor(&c));
    }

    #[test]
    fn complement_flips_each_output_and_round_trips() {
        let a = OutputSet::<Anonymous>::anonymous(&[true, false, true]);
        assert_eq!(
            a.not(),
            OutputSet::<Anonymous>::anonymous(&[false, true, false])
        );
        // Double complement is the identity.
        assert_eq!(a.not().not(), a);

        // Empty (all-`false`) and full (all-`true`) rows are exact opposites under `!`.
        let empty = OutputSet::<Anonymous>::anonymous(&[false, false, false]);
        let full = OutputSet::<Anonymous>::anonymous(&[true, true, true]);
        assert_eq!(empty.not(), full);
        assert_eq!(full.not(), empty);
    }

    #[test]
    fn complement_operator_forms_agree_with_the_method() {
        let a = OutputSet::<Anonymous>::anonymous(&[true, false, true]);
        assert_eq!(!a.clone(), a.not());
        assert_eq!(!&a, a.not());
    }

    #[test]
    fn complement_crosses_the_word_boundary_and_leaves_padding_zero() {
        // 70 outputs span two words; the high word holds only outputs 64..=69 (padding above must stay
        // zero after the complement's bit-flip).
        let mut membership = vec![false; 70];
        for &i in &[0usize, 64, 69] {
            membership[i] = true;
        }
        let a = OutputSet::<Anonymous>::anonymous(&membership);
        let complement = a.not();

        let expected: Vec<bool> = membership.iter().map(|&b| !b).collect();
        assert_eq!(complement.iter().collect::<Vec<bool>>(), expected);

        // Compare against a freshly-built row of the complemented pattern: a stray padding bit in the
        // high word would break both this equality and the hash.
        let fresh = OutputSet::<Anonymous>::anonymous(&expected);
        assert_eq!(complement, fresh);
        assert_eq!(hash_of(&complement), hash_of(&fresh));
        // Outputs 65..=68 are the only high-word bits flipped on (64 and 69 were asserted, so clear).
        assert_eq!(complement.packed()[1], 0b1_1110);
    }
}
