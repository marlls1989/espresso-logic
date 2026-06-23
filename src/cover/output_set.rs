//! [`OutputSet`]: a cube's output-membership as a compact binary bitmap.
//!
//! A cube belongs to exactly one of a cover's sets (ON/`F`, don't-care/`D`, OFF/`R`); its *outputs*
//! record, per output column, whether the cube asserts that output **in its set**. That is inherently
//! two-state (asserted / not) and positional, so — unlike the tri-state input [`Minterm`](super::Minterm)
//! — it needs only **one bit per output**, packed 64 to a `u64` word. This is exactly the C cube's
//! output-region encoding, which lets the Espresso boundary copy the words verbatim.
//!
//! Output **labels** are still meaningful (named `.ob` outputs, relabelling), so `OutputSet` keeps a
//! shared [`Symbols<O>`] handle — the same one the cover holds. The bit packing itself is independent of
//! the label type `O`, so re-homing onto another `Symbols<O>` of the same arity is a cheap `Arc` clone.

use super::label::Anonymous;
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
    pub fn symbols(&self) -> &Arc<Symbols<O>> {
        &self.symbols
    }

    /// The output labels, in index order.
    #[must_use]
    pub fn vars(&self) -> &[O] {
        self.symbols.labels()
    }

    /// The packed bit words, for cheap re-home onto another [`Symbols`] table of the same arity (the
    /// packing is independent of the label type). Same layout the Espresso output region uses.
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

/// Renders the membership as a bare `1`/`0` row in index order — the output field of a PLA line. Needs
/// no bound on `O` (positional).
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
    /// Build an anonymous (positional) output set from a per-output membership slice.
    pub(crate) fn anonymous(membership: &[bool]) -> Self {
        OutputSet::from_symbols(
            Symbols::<Anonymous>::anonymous(membership.len()),
            membership.iter().copied(),
        )
    }
}
