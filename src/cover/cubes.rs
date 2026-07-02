//! The [`Cube`] product term: a tri-state input [`Minterm`] + a binary output [`OutputSet`].
//!
//! A cube is one product term of a cover. It belongs to exactly one of the cover's three sets
//! (ON/`F`, don't-care/`D`, OFF/`R`) — recorded by its [`CubeType`] — and stores:
//!
//! - `inputs`: the input pattern minterm (`Some(true)`/`Some(false)`/`None` per input variable);
//! - `outputs`: a binary membership bitmap ([`OutputSet`]) — one bit per output: set means "this cube
//!   asserts the output (in its set)", clear means "not asserted".
//!
//! Keeping the per-cube [`CubeType`] (rather than merging all three into one tri-state output) is
//! what makes the representation **lossless**: the PLA `~` ("not asserted") state stays distinct
//! from the `-` (don't-care) state, so multi-output FD/FDR covers round-trip and minimise
//! byte-identically to the C library.

use super::error::DuplicateLabel;
use super::label::{Anonymous, Label, StringLabel};
use super::minterm::{ExpandedMinterms, Minterm};
use super::output_set::OutputSet;
use super::symbols::Symbols;
use std::fmt;
use std::sync::Arc;

/// Which of a cover's three sets a cube belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CubeType {
    /// ON-set (the function is 1).
    #[default]
    F,
    /// Don't-care set (the function is `-`).
    D,
    /// OFF-set (the function is 0).
    R,
}

/// A cube (product term) in a cover: an input pattern, an output-membership mask, and a set tag.
///
/// Generic over the input label type `I` and the output label type `O`, so a cover can have, e.g.,
/// labelled inputs and an anonymous output.
#[derive(Clone)]
pub struct Cube<I, O> {
    pub(crate) inputs: Minterm<I>,
    /// Output-membership bitmap: bit set where this cube asserts the output (in its set).
    pub(crate) outputs: OutputSet<O>,
    pub(crate) set: CubeType,
}

impl<I: Label + fmt::Debug, O> fmt::Debug for Cube<I, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cube")
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .field("set", &self.set)
            .finish()
    }
}

/// Renders the cube as a PLA-style row — `<inputs> <outputs>` (the inputs a bare `1`/`0`/`-` string
/// from [`Minterm`]'s `Display`, the outputs a bare `1`/`0` string from [`OutputSet`]'s) — annotating
/// the set tag for don't-care/off-set cubes (`F` is the default and left unmarked). Needs no bound on
/// the label types.
impl<I, O> fmt::Display for Cube<I, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.inputs, self.outputs)?;
        match self.set {
            CubeType::F => Ok(()),
            CubeType::D => write!(f, " (D)"),
            CubeType::R => write!(f, " (R)"),
        }
    }
}

/// Two cubes are equal when they belong to the same set, their input patterns are equal, and their
/// output bitmaps are equal. Inputs compare by [`Minterm`]'s identity-based equality (aligning by
/// variable name, absent variables as don't-cares); outputs compare positionally by [`OutputSet`].
impl<I: Label, O> PartialEq for Cube<I, O> {
    fn eq(&self, other: &Self) -> bool {
        self.set == other.set && self.inputs == other.inputs && self.outputs == other.outputs
    }
}

impl<I: Label, O> Eq for Cube<I, O> {}

/// Hashes the same fields the [`PartialEq`] impl compares (set tag + input minterm + output bitmap),
/// keeping the `Hash`/`Eq` contract so a `Cube` can key a `HashMap`/`HashSet`.
impl<I: Label, O> std::hash::Hash for Cube<I, O> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.set.hash(state);
        self.inputs.hash(state);
        self.outputs.hash(state);
    }
}

impl<I, O> Cube<I, O> {
    /// Build a cube from its input pattern, output-membership bitmap, and set tag.
    ///
    /// This is the raw constructor: it pairs a pre-built input [`Minterm`] with a per-output
    /// [`OutputSet`] and does no validation — the two halves carry their own independent symbol
    /// tables. The label types flow through: two labelled halves give a labelled `Cube<I, O>`, two
    /// anonymous halves an anonymous one. Build each half with its own
    /// [`Minterm`](Minterm::labeled)/[`OutputSet`](OutputSet::labeled) constructor (`labeled` /
    /// `with_labels` / `anonymous`). For a cube built in one shot from `(label, value)` pairs use
    /// [`with_labels`](Self::with_labels) / [`labeled`](Self::labeled) / [`anonymous`](Self::anonymous),
    /// which are convenience wrappers over this.
    ///
    /// A cube carries its own tables until it is added to a [`Cover`](crate::Cover) (via
    /// [`push`](crate::Cover::push) / [`from_cubes`](crate::Cover::from_cubes)), which re-points it
    /// onto the cover's shared input/output tables by variable identity.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cube, CubeType, Minterm, OutputSet};
    ///
    /// // input a=1 (b a don't-care); assert output 0, not output 1; ON-set cube.
    /// let inputs = Minterm::anonymous(&[Some(true), None]);
    /// let outputs = OutputSet::anonymous(&[true, false]);
    /// let cube = Cube::new(inputs, outputs, CubeType::F);
    ///
    /// assert_eq!(cube.cube_type(), CubeType::F);
    /// assert_eq!(cube.inputs().value_at(0), Some(true));
    /// assert!(cube.outputs().value_at(0) && !cube.outputs().value_at(1));
    /// ```
    ///
    /// Two labelled halves compose into a labelled `Cube<Symbol, Symbol>`:
    ///
    /// ```
    /// use espresso_logic::{Cube, CubeType, Minterm, OutputSet, Symbol};
    ///
    /// let inputs = Minterm::<Symbol>::with_labels(&[("a", Some(true))]).unwrap();
    /// let outputs = OutputSet::<Symbol>::with_labels(&[("f", true)]).unwrap();
    /// let cube: Cube<Symbol, Symbol> = Cube::new(inputs, outputs, CubeType::F);
    ///
    /// assert_eq!(cube.inputs().value_of("a"), Some(true));
    /// ```
    #[must_use]
    pub fn new(inputs: Minterm<I>, outputs: OutputSet<O>, set: CubeType) -> Self {
        Cube {
            inputs,
            outputs,
            set,
        }
    }

    /// The input pattern of this cube.
    #[must_use]
    pub fn inputs(&self) -> &Minterm<I> {
        &self.inputs
    }

    /// The output-membership bitmap of this cube (a set bit where the output is asserted).
    ///
    /// This is a per-cube, per-set membership bitmap, **not** a merged tri-state output: an asserted
    /// bit means "this cube asserts the output in its own set ([`cube_type`](Self::cube_type))". For an
    /// FR/FDR cover, a given input pattern can therefore appear in more than one cube (e.g. an F cube
    /// and an R cube), each with its own bitmap.
    #[must_use]
    pub fn outputs(&self) -> &OutputSet<O> {
        &self.outputs
    }

    /// Which set (F/D/R) this cube belongs to.
    #[must_use]
    pub fn cube_type(&self) -> CubeType {
        self.set
    }

    /// Whether output `i` is asserted by this cube.
    pub(crate) fn asserts(&self, i: usize) -> bool {
        self.outputs.value_at(i)
    }
}

impl<I: Label, O> Cube<I, O> {
    /// Expand this cube's input pattern into every fully-assigned minterm over `vars`.
    ///
    /// `vars` is the explicit target header and MAY be a superset of the cube's own inputs: variables
    /// of `vars` absent from the cube are split into both polarities, the cube's own don't-cares are
    /// expanded, and any input variable not in `vars` is dropped. Every returned minterm assigns every
    /// variable in `vars`, all sharing one canonical header. Returns a lazy [`ExpandedMinterms`]
    /// iterator that packs each minterm on demand. See [`Minterm::expand_over`].
    ///
    /// `vars` names a variable *set*: a repeated variable is deduplicated (the first occurrence is
    /// kept), so `[a, b, a]` and `[a, b]` expand over the same header.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cube, CubeType, Symbol};
    /// use std::collections::BTreeSet;
    ///
    /// // input a=1 (b unconstrained), expanded over [a, b].
    /// let cube = Cube::<Symbol, Symbol>::with_labels(&[("a", Some(true))], &[("f", true)], CubeType::F)
    ///     .unwrap();
    /// let got: BTreeSet<_> = cube
    ///     .expand_to(&[Symbol::new("a"), Symbol::new("b")])
    ///     .into_iter()
    ///     .collect();
    /// assert_eq!(got.len(), 2); // {a:1,b:0}, {a:1,b:1}
    /// ```
    #[must_use]
    pub fn expand_to(&self, vars: &[I]) -> ExpandedMinterms<I> {
        let target = Symbols::deduped(vars.iter().cloned());
        self.inputs.expand_over(&target)
    }
}

impl Cube<Anonymous, Anonymous> {
    /// Build an **anonymous** (positional) cube from an input pattern and an output-membership mask.
    ///
    /// - `inputs[i]` is the value of input `i`: `Some(true)`/`Some(false)`, or `None` for don't-care.
    /// - `membership[j]` is whether this cube asserts output `j` **in its own set** (`set`): an `F`
    ///   cube asserts the ON-set outputs, a `D` cube the don't-care outputs, an `R` cube the OFF-set
    ///   outputs.
    ///
    /// The cube carries its own anonymous symbol tables; [`Cover::from_cubes`](crate::Cover::from_cubes)
    /// and [`Cover::push`](crate::Cover::push) re-point it onto the cover's shared tables.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cube, CubeType};
    ///
    /// // 01 -> output 0 asserted, in the ON-set.
    /// let cube = Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F);
    /// assert_eq!(cube.cube_type(), CubeType::F);
    /// ```
    #[must_use]
    pub fn anonymous(
        inputs: &[Option<bool>],
        membership: &[bool],
        set: CubeType,
    ) -> Cube<Anonymous, Anonymous> {
        let im = Minterm::from_symbols(
            Symbols::<Anonymous>::anonymous(inputs.len()),
            inputs.iter().copied(),
        );
        let om = OutputSet::anonymous(membership);
        Cube::new(im, om, set)
    }
}

impl<I: Label, O: Label> Cube<I, O> {
    /// Build a **labelled** cube from `(label, value)` pairs.
    ///
    /// - Each input pair is `(label, value)` where `value` is `Some(true)`/`Some(false)`, or `None`
    ///   for a don't-care.
    /// - Each output pair is `(label, asserted)` where `asserted` says whether this cube asserts that
    ///   output **in its own set** (`set`): an `F` cube asserts the ON-set outputs, a `D` cube the
    ///   don't-care outputs, an `R` cube the OFF-set outputs.
    ///
    /// Pairing each label with its value makes a label/value length mismatch unrepresentable. The cube
    /// carries its own symbol tables; [`Cover::push`](crate::Cover::push) /
    /// [`Cover::from_cubes`](crate::Cover::from_cubes) align it onto the cover by variable
    /// [identity](crate::Label), so the labels need no particular order.
    ///
    /// Works for any label type — [`Symbol`](crate::Symbol), `String`, `u32`, … For `&str` names,
    /// [`with_labels`](Self::with_labels) avoids naming the label type at each pair.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateLabel`] if either side repeats a label. Variables align by identity, so a
    /// side's labels must be distinct — duplicates would collapse onto one column and drop a value.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cube, CubeType, Symbol};
    ///
    /// // a·b in the ON-set, asserting output `f`.
    /// let cube = Cube::<Symbol, Symbol>::labeled(
    ///     &[(Symbol::new("a"), Some(true)), (Symbol::new("b"), Some(true))],
    ///     &[(Symbol::new("f"), true)],
    ///     CubeType::F,
    /// )
    /// .unwrap();
    /// assert_eq!(cube.inputs().num_vars(), 2);
    /// assert_eq!(cube.outputs().num_vars(), 1);
    /// ```
    pub fn labeled(
        inputs: &[(I, Option<bool>)],
        outputs: &[(O, bool)],
        set: CubeType,
    ) -> Result<Cube<I, O>, DuplicateLabel> {
        Self::from_label_arcs(
            inputs.iter().map(|(l, _)| l.clone()).collect(),
            inputs.iter().map(|(_, v)| *v),
            outputs.iter().map(|(l, _)| l.clone()).collect(),
            outputs.iter().map(|(_, a)| *a),
            set,
        )
    }

    /// Shared core of [`labeled`](Self::labeled)/[`with_labels`](Self::with_labels): build each half
    /// from its label `Arc` and value iterator (each validates its own distinctness), then pair them.
    /// The input side reports [`DuplicateLabel::Input`], the output side [`DuplicateLabel::Output`].
    fn from_label_arcs(
        in_labels: Arc<[I]>,
        in_values: impl IntoIterator<Item = Option<bool>>,
        out_labels: Arc<[O]>,
        out_values: impl IntoIterator<Item = bool>,
        set: CubeType,
    ) -> Result<Cube<I, O>, DuplicateLabel> {
        Ok(Cube::new(
            Minterm::from_label_arcs(in_labels, in_values)?,
            OutputSet::from_label_arcs(out_labels, out_values)?,
            set,
        ))
    }
}

impl<I: StringLabel, O: StringLabel> Cube<I, O> {
    /// Build a labelled cube from `(name, value)` pairs, naming variables with any `&str`-like type.
    ///
    /// A string-name convenience over [`labeled`](Self::labeled): each label is built via `From<&str>`,
    /// so no string type is privileged (`&str`, `String`, `Arc<str>`, … all work). The label type is
    /// inferred from context (e.g. `Cube::<Symbol, Symbol>::with_labels`).
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateLabel`] if either side repeats a name (see [`labeled`](Self::labeled)).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cube, CubeType, Symbol};
    ///
    /// let cube = Cube::<Symbol, Symbol>::with_labels(
    ///     &[("a", Some(true)), ("b", Some(true))],
    ///     &[("f", true)],
    ///     CubeType::F,
    /// )
    /// .unwrap();
    /// assert_eq!(cube.inputs().num_vars(), 2);
    /// ```
    pub fn with_labels<SI: AsRef<str>, SO: AsRef<str>>(
        inputs: &[(SI, Option<bool>)],
        outputs: &[(SO, bool)],
        set: CubeType,
    ) -> Result<Cube<I, O>, DuplicateLabel> {
        Self::from_label_arcs(
            inputs.iter().map(|(s, _)| I::from(s.as_ref())).collect(),
            inputs.iter().map(|(_, v)| *v),
            outputs.iter().map(|(s, _)| O::from(s.as_ref())).collect(),
            outputs.iter().map(|(_, a)| *a),
            set,
        )
    }
}
