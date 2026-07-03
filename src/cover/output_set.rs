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

use super::error::DuplicateLabel;
use super::label::{Anonymous, Label, StringLabel};
use super::symbols::Symbols;
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

/// Two output sets are equal when they have the same number of outputs and the same membership bits
/// (positional — within a cover all cubes share one output `Symbols`, so this matches by column).
impl<O> PartialEq for OutputSet<O> {
    fn eq(&self, other: &Self) -> bool {
        self.num_vars() == other.num_vars() && self.bits == other.bits
    }
}

impl<O> Eq for OutputSet<O> {}

impl<O> Hash for OutputSet<O> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.num_vars().hash(state);
        self.bits.hash(state);
    }
}

impl<O> PartialOrd for OutputSet<O> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<O> Ord for OutputSet<O> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.num_vars()
            .cmp(&other.num_vars())
            .then_with(|| self.bits.cmp(&other.bits))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash_of(o: &OutputSet<Anonymous>) -> u64 {
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
    fn ord_breaks_ties_by_arity_then_bits() {
        let one = OutputSet::<Anonymous>::anonymous(&[true]);
        let two = OutputSet::<Anonymous>::anonymous(&[false, false]);
        assert!(one < two, "fewer outputs orders first, regardless of bits");

        // Same arity: ordered by the packed words. Bit 1 set (word value 2) > bit 0 set (value 1).
        let bit0 = OutputSet::<Anonymous>::anonymous(&[true, false]);
        let bit1 = OutputSet::<Anonymous>::anonymous(&[false, true]);
        assert!(bit0 < bit1);
        assert_ne!(bit0, bit1);
    }
}
