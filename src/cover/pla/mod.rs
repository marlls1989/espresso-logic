//! PLA (Programmable Logic Array) format support
//!
//! This module provides traits and utilities for reading and writing Boolean functions
//! in the PLA (Programmable Logic Array) format, a standard text-based format developed
//! at UC Berkeley for representing Boolean functions.
//!
//! # Overview
//!
//! The PLA format represents Boolean functions as truth tables with:
//! - Input variables and their patterns (0, 1, or don't-care)
//! - Output variables and their values
//! - Optional variable labels for readability
//!
//! This module provides:
//!
//! - [`PlaCover`] - the reader output: a cover typed by which label sections (`.ilb`/`.ob`) the file
//!   carried (named sides use the label type `S`, absent sides are [`Anonymous`])
//! - [`PLAWriter`] - serialising any [`Cover`] whose labels can render (see [`PlaLabel`])
//!
//! Reading is provided by [`PlaCover`]; writing by [`PLAWriter`], making PLA file I/O straightforward.
//!
//! # Quick Example
//!
//! ```
//! use espresso_logic::{Cover, CoverType, Minimizable, PlaCover, Symbol, PLAWriter};
//!
//! # fn main() -> std::io::Result<()> {
//! # let pla_text = ".i 2\n.o 1\n.p 2\n01 1\n10 1\n.e\n";
//! # let cover = PlaCover::<Symbol>::from_pla_string(pla_text)?;
//! // Read PLA file
//! // let cover = PlaCover::<Symbol>::from_pla_file("input.pla")?;
//!
//! // Minimise
//! let minimised = cover.minimize()?;
//!
//! // Write result
//! // minimised.to_pla_file("output.pla", CoverType::F)?;
//! let pla_string = minimised.to_pla_string(CoverType::F)?;
//! # Ok(())
//! # }
//! ```
//!
//! # PLA Format Specification
//!
//! For complete details on the PLA file format including directives, encoding rules,
//! and examples, see the comprehensive guide below:
#![doc = include_str!("../../../docs/PLA_FORMAT.md")]

pub mod error;

pub use error::{PLAError, PLAReadError, PLAWriteError};

use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Cursor, Write};
use std::path::Path;
use std::sync::Arc;

use super::conversions::{anonymous_cover_from_raw, RawCube};
use super::label::{Anonymous, Label, StringLabel};
use super::minimisation::Minimizable;
use super::symbols::Symbols;
use super::{Cover, CoverType, CubeType};
use crate::espresso::error::MinimizationError;
use crate::EspressoConfig;

/// How a label type renders into a PLA `.ilb`/`.ob` section — the type-level "is this a name?" test.
///
/// A label that implements [`Display`](std::fmt::Display) is a *name* and renders itself; [`Anonymous`]
/// is positional and renders nothing, so its section is omitted. This is what makes label-presence a
/// compile-time fact rather than a runtime flag: a `Cover<Anonymous, _>` cannot emit input names *by
/// construction*, and a named `Cover<Symbol, _>` always can.
///
/// Implemented (blanket) for every `Label + Display` — `Symbol`, `String`, `Arc<str>`, `u32`, … — and
/// for [`Anonymous`] (which deliberately is not `Display`, so the two impls never overlap).
pub trait PlaLabel: Label {
    /// The label strings to write for a section, or `None` to omit it (positional labels, or an empty
    /// header — a zero-width cover writes no `.ilb`/`.ob`).
    fn pla_labels(labels: &[Self]) -> Option<Vec<String>>;
}

impl<T: Label + fmt::Display> PlaLabel for T {
    fn pla_labels(labels: &[Self]) -> Option<Vec<String>> {
        if labels.is_empty() {
            None
        } else {
            Some(labels.iter().map(|l| l.to_string()).collect())
        }
    }
}

impl PlaLabel for Anonymous {
    fn pla_labels(_labels: &[Self]) -> Option<Vec<String>> {
        None
    }
}

/// Trait for types that support PLA serialisation (writing)
///
/// This trait provides methods for serialising covers to PLA format. It is implemented for
/// [`Cover<I, O>`](crate::Cover) and [`PlaCover<S>`](crate::PlaCover) whose label types implement
/// [`PlaLabel`].
pub trait PLAWriter {
    /// Write this cover to PLA format using a writer
    ///
    /// This is the core serialisation method that writes directly to any `Write` implementation.
    /// Both `to_pla_string` and `to_pla_file` delegate to this method.
    fn write_pla<W: Write>(&self, writer: &mut W, pla_type: CoverType)
        -> Result<(), PLAWriteError>;

    /// Convert this cover to a PLA format string
    ///
    /// This is a convenience method that delegates to `write_pla`.
    /// For better performance when writing to files, use `to_pla_file` instead.
    fn to_pla_string(&self, pla_type: CoverType) -> Result<String, PLAWriteError> {
        let mut buffer = Vec::new();
        self.write_pla(&mut buffer, pla_type)?;
        // `write_pla` only ever writes formatted Rust strings (directives, `0/1/-` cube chars, and
        // each label's `Display` output), so every byte originates from valid UTF-8 — the conversion
        // cannot fail.
        Ok(String::from_utf8(buffer).expect("PLA output is built from UTF-8 Rust strings"))
    }

    /// Write this cover to a PLA file
    ///
    /// This method delegates to `write_pla` for efficient file writing without
    /// building the entire string in memory first.
    fn to_pla_file<P: AsRef<Path>>(
        &self,
        path: P,
        pla_type: CoverType,
    ) -> Result<(), PLAWriteError> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.write_pla(&mut writer, pla_type)?;
        writer.flush()?;
        Ok(())
    }
}

/// `PLAWriter` for any cover whose label types can render PLA sections ([`PlaLabel`]).
///
/// `.ilb`/`.ob` are emitted iff the input/output label types are *names* (`Display`); an `Anonymous`
/// side omits its section by construction. No runtime label-presence flag is consulted.
impl<I: PlaLabel, O: PlaLabel> PLAWriter for Cover<I, O> {
    fn write_pla<W: Write>(
        &self,
        writer: &mut W,
        pla_type: CoverType,
    ) -> Result<(), PLAWriteError> {
        // Write .type directive first for FD, FR, FDR (matching C output order)
        match pla_type {
            CoverType::FD => writeln!(writer, ".type fd")?,
            CoverType::FR => writeln!(writer, ".type fr")?,
            CoverType::FDR => writeln!(writer, ".type fdr")?,
            CoverType::F => {} // F is default, no .type needed
        }

        // Write PLA header (matching C output order: .i, .o, .ilb, .ob)
        writeln!(writer, ".i {}", self.num_inputs())?;
        writeln!(writer, ".o {}", self.num_outputs())?;

        // Write input labels iff the input label type is a name (Display); Anonymous omits.
        if let Some(labels) = I::pla_labels(self.input_symbols().labels()) {
            write!(writer, ".ilb")?;
            for label in labels {
                write!(writer, " {}", label)?;
            }
            writeln!(writer)?;
        }

        // Write output labels iff the output label type is a name; Anonymous omits.
        if let Some(labels) = O::pla_labels(self.output_symbols().labels()) {
            write!(writer, ".ob")?;
            for label in labels {
                write!(writer, " {}", label)?;
            }
            writeln!(writer)?;
        }

        // Filter cubes based on output type using the cube's set tag
        let filtered_cubes: Vec<_> = self
            .cubes
            .iter()
            .filter(|cube| match pla_type {
                CoverType::F => cube.set == CubeType::F,
                CoverType::FD => cube.set == CubeType::F || cube.set == CubeType::D,
                CoverType::FR => cube.set == CubeType::F || cube.set == CubeType::R,
                CoverType::FDR => true, // All cubes
            })
            .collect();

        // Add .p directive with filtered cube count
        writeln!(writer, ".p {}", filtered_cubes.len())?;

        // Write filtered cubes
        for cube in filtered_cubes {
            // Write inputs
            for inp in cube.inputs.iter() {
                write!(
                    writer,
                    "{}",
                    match inp {
                        Some(false) => '0',
                        Some(true) => '1',
                        None => '-',
                    }
                )?;
            }

            write!(writer, " ")?;

            // Encode outputs. `cube.outputs` is a membership mask: Some(true) = asserted.
            match pla_type {
                CoverType::F => {
                    // F-type: '1' for asserted, '0' otherwise
                    for out in cube.outputs.iter() {
                        write!(writer, "{}", if out == Some(true) { '1' } else { '0' })?;
                    }
                }
                CoverType::FD | CoverType::FDR | CoverType::FR => {
                    // The cube's set determines the character for asserted bits; '~' otherwise.
                    let set_char = match cube.set {
                        CubeType::F => '1', // ON
                        CubeType::D => '2', // DC
                        CubeType::R => '0', // OFF
                    };

                    for out in cube.outputs.iter() {
                        write!(writer, "{}", if out == Some(true) { set_char } else { '~' })?;
                    }
                }
            }

            writeln!(writer)?;
        }

        // C version uses ".e" for F-type, ".end" for FD/FR/FDR types
        match pla_type {
            CoverType::F => writeln!(writer, ".e")?,
            _ => writeln!(writer, ".end")?,
        }

        Ok(())
    }
}

/// A cover read from a PLA file, typed by **which label sections the file carried**.
///
/// PLA `.ilb` and `.ob` are independent and optional, so a file's label content is reflected in the
/// **type**: a present section makes that side a *name* (label type `S`), an absent one makes it
/// [`Anonymous`]. There is no runtime "is it labelled" flag — the writer reproduces exactly the
/// sections the file carried because the absence of names is encoded as `Anonymous` (which cannot emit
/// a section). The label type `S` is whatever string-like type you read into (`Symbol`, `String`,
/// `Arc<str>`, …); none is privileged.
///
/// Read with [`from_pla_file`](Self::from_pla_file) / [`from_pla_string`](Self::from_pla_string), then
/// [`minimize`](crate::Minimizable::minimize) and [`to_pla_string`](crate::PLAWriter::to_pla_string)
/// dispatch across the variants. Match on it to recover the concrete [`Cover`].
pub enum PlaCover<S> {
    /// Both `.ilb` and `.ob` were present.
    InputsOutputsNamed(Cover<S, S>),
    /// Only `.ilb` was present (named inputs, positional outputs).
    InputsNamed(Cover<S, Anonymous>),
    /// Only `.ob` was present (positional inputs, named outputs).
    OutputsNamed(Cover<Anonymous, S>),
    /// Neither section was present — a purely positional cover.
    Positional(Cover<Anonymous, Anonymous>),
}

/// Two `PlaCover`s are equal only when they carry the same label sections (same variant) and their
/// inner covers are equal. A named and a positional cover are never equal even if their cubes match,
/// because their types — and the PLA they would write — differ.
impl<S: Label> PartialEq for PlaCover<S> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InputsOutputsNamed(a), Self::InputsOutputsNamed(b)) => a == b,
            (Self::InputsNamed(a), Self::InputsNamed(b)) => a == b,
            (Self::OutputsNamed(a), Self::OutputsNamed(b)) => a == b,
            (Self::Positional(a), Self::Positional(b)) => a == b,
            _ => false,
        }
    }
}

impl<S: Label> Eq for PlaCover<S> {}

/// Shows the variant and its inner [`Cover`]. (A manual impl rather than `derive`, which would demand
/// the wrong `S: Debug` placement across the mixed `Cover<S, Anonymous>` etc. variants.)
impl<S: fmt::Debug> fmt::Debug for PlaCover<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (name, cover): (&str, &dyn fmt::Debug) = match self {
            Self::InputsOutputsNamed(c) => ("InputsOutputsNamed", c),
            Self::InputsNamed(c) => ("InputsNamed", c),
            Self::OutputsNamed(c) => ("OutputsNamed", c),
            Self::Positional(c) => ("Positional", c),
        };
        f.debug_tuple(name).field(cover).finish()
    }
}

/// Clones the inner cover, preserving the variant.
impl<S: Clone> Clone for PlaCover<S> {
    fn clone(&self) -> Self {
        match self {
            Self::InputsOutputsNamed(c) => Self::InputsOutputsNamed(c.clone()),
            Self::InputsNamed(c) => Self::InputsNamed(c.clone()),
            Self::OutputsNamed(c) => Self::OutputsNamed(c.clone()),
            Self::Positional(c) => Self::Positional(c.clone()),
        }
    }
}

/// Hashes the variant discriminant then the inner cover, matching [`PartialEq`]'s variant-aware
/// equality (a named and a positional cover never compare equal, so they must not collide by content).
impl<S: Label> std::hash::Hash for PlaCover<S> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::InputsOutputsNamed(c) => c.hash(state),
            Self::InputsNamed(c) => c.hash(state),
            Self::OutputsNamed(c) => c.hash(state),
            Self::Positional(c) => c.hash(state),
        }
    }
}

impl<S> PlaCover<S> {
    /// Recover the inner cover as a positional [`Cover<Anonymous, Anonymous>`](Cover), dropping the
    /// label sections — a uniform escape hatch that works for every variant (unlike matching, where
    /// each arm yields a differently-typed cover). Use a `match` when you need to keep the labels.
    ///
    /// To build a `PlaCover` from a typed cover, construct the variant directly (the variants are
    /// public) — e.g. `PlaCover::InputsOutputsNamed(cover)`. A blanket `From` is impossible: when
    /// `S = Anonymous` all four variants share the same inner type, so the conversions would overlap.
    #[must_use]
    pub fn into_anonymous(self) -> Cover<Anonymous, Anonymous> {
        match self {
            Self::InputsOutputsNamed(c) => c.anonymize(),
            Self::InputsNamed(c) => c.anonymize(),
            Self::OutputsNamed(c) => c.anonymize(),
            Self::Positional(c) => c.anonymize(),
        }
    }
}

/// Raw PLA components from [`parse_pla`]: label sections kept as the strings read from the file (an
/// absent section is `None`), to be turned into a concrete label type by [`PlaCover`].
struct ParsedPla {
    num_inputs: usize,
    num_outputs: usize,
    input_labels: Option<Vec<String>>,
    output_labels: Option<Vec<String>>,
    cubes: Vec<RawCube>,
    cover_type: CoverType,
}

/// Parse a PLA stream into its raw components (dimensions, optional `.ilb`/`.ob` strings, cubes). The
/// label type is decided later by [`PlaCover`], so this stays label-type-agnostic.
fn parse_pla<R: std::io::BufRead>(reader: R) -> Result<ParsedPla, PLAReadError> {
    let mut num_inputs: Option<usize> = None;
    let mut num_outputs: Option<usize> = None;
    let mut cubes: Vec<RawCube> = Vec::new();
    // Default to FD_type to match C espresso behaviour (main.c line 21)
    // This causes '-' in outputs to be parsed as D cubes, not just don't-care bits
    let mut cover_type = CoverType::FD;
    let mut input_labels: Option<Vec<String>> = None;
    let mut output_labels: Option<Vec<String>> = None;

    let lines: Vec<String> = reader.lines().collect::<io::Result<Vec<_>>>()?;

    // C's `parse_pla` (cvrin.c) reads cube data as a single character stream: space, tab, `|` and
    // *newlines* are all insignificant, and one cube is exactly `ni + no` significant characters —
    // there are no cube separators. We mirror that by accumulating significant cube characters and
    // draining complete `ni + no` chunks as they form, instead of treating each line as a cube.
    let is_pla_delimiter = |c: char| c.is_whitespace() || c == '|';
    let mut cube_stream: Vec<char> = Vec::new();

    for raw_line in &lines {
        let line = raw_line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse directives
        if line.starts_with('.') {
            let parts: Vec<&str> = line.split_whitespace().collect();

            match parts.first().copied() {
                Some(".i") => {
                    let val: usize =
                        parts.get(1).and_then(|s| s.parse().ok()).ok_or_else(|| {
                            PLAError::InvalidInputDirective {
                                value: Arc::from(*parts.get(1).unwrap_or(&"")),
                            }
                        })?;
                    num_inputs = Some(val);
                }
                Some(".o") => {
                    let val: usize =
                        parts.get(1).and_then(|s| s.parse().ok()).ok_or_else(|| {
                            PLAError::InvalidOutputDirective {
                                value: Arc::from(*parts.get(1).unwrap_or(&"")),
                            }
                        })?;
                    num_outputs = Some(val);
                }
                Some(".type") => {
                    // An unrecognised or missing `.type` value is rejected, mirroring how bad
                    // `.i`/`.o` values error (rather than silently falling back to a default).
                    cover_type = match parts.get(1).copied() {
                        Some("f") => CoverType::F,
                        Some("fd") => CoverType::FD,
                        Some("fr") => CoverType::FR,
                        Some("fdr") => CoverType::FDR,
                        other => {
                            return Err(PLAError::InvalidTypeDirective {
                                value: Arc::from(other.unwrap_or("")),
                            }
                            .into())
                        }
                    };
                }
                Some(".ilb") => {
                    // Parse input labels: .ilb label1 label2 label3 ...
                    let labels: Vec<String> = parts.iter().skip(1).map(|s| s.to_string()).collect();
                    if !labels.is_empty() {
                        input_labels = Some(labels);
                    }
                }
                Some(".ob") => {
                    // Parse output labels: .ob label1 label2 label3 ...
                    let labels: Vec<String> = parts.iter().skip(1).map(|s| s.to_string()).collect();
                    if !labels.is_empty() {
                        output_labels = Some(labels);
                    }
                }
                Some(".e") | Some(".end") => break,
                Some(".p") => {}
                _ => {}
            }
            continue;
        }

        // Cube-data line. Dimensions must already be declared: there is no inference, because
        // space/tab/`|`/newlines are all insignificant, so cube data alone cannot locate the
        // input/output split (C requires `.i`/`.o` before any cube — cvrin.c).
        let (ni, no) = match (num_inputs, num_outputs) {
            (Some(ni), Some(no)) => (ni, no),
            _ => {
                return Err(match (num_inputs.is_none(), num_outputs.is_none()) {
                    (true, true) => PLAError::MissingDimensions,
                    (true, false) => PLAError::MissingInputDirective,
                    (false, true) => PLAError::MissingOutputDirective,
                    (false, false) => unreachable!("both-declared is the Some/Some arm above"),
                }
                .into());
            }
        };

        // Append this line's significant characters to the stream, then drain every complete
        // `ni + no` cube now available. A cube may span several lines, and several cubes may share a
        // line — exactly as C reads it.
        cube_stream.extend(line.chars().filter(|c| !is_pla_delimiter(*c)));
        let width = ni + no;
        // `checked_div` yields `None` only for a degenerate zero-width cover (`.i 0 .o 0`), where no
        // cube can ever form; skip draining in that case rather than dividing by zero.
        if let Some(complete) = cube_stream.len().checked_div(width) {
            for k in 0..complete {
                let chunk = &cube_stream[k * width..(k + 1) * width];
                push_cube(&chunk[..ni], &chunk[ni..], cover_type, &mut cubes)?;
            }
            cube_stream.drain(..complete * width);
        }
    }

    // Any trailing characters that do not complete a final `ni + no` cube are ignored, matching C
    // (which warns about and skips an incomplete final product term).

    // Verify we got dimensions
    let num_inputs = num_inputs.ok_or(PLAError::MissingInputDirective)?;
    let num_outputs = num_outputs.ok_or(PLAError::MissingOutputDirective)?;

    // Validate label counts if present
    if let Some(ref labels) = input_labels {
        if labels.len() != num_inputs {
            return Err(PLAError::LabelCountMismatch {
                label_type: Arc::from("input"),
                expected: num_inputs,
                actual: labels.len(),
            }
            .into());
        }
    }
    if let Some(ref labels) = output_labels {
        if labels.len() != num_outputs {
            return Err(PLAError::LabelCountMismatch {
                label_type: Arc::from("output"),
                expected: num_outputs,
                actual: labels.len(),
            }
            .into());
        }
    }

    // Label sections stay `Option`: their presence/absence is what selects the `PlaCover` variant
    // (and thus whether the writer re-emits them).
    Ok(ParsedPla {
        num_inputs,
        num_outputs,
        input_labels,
        output_labels,
        cubes,
        cover_type,
    })
}

/// Parse one cube's worth of significant characters — already split at the input/output boundary, so
/// `input_chars` is exactly `ni` long and `output_chars` exactly `no` — and append the resulting
/// F/D/R raw cubes. Mirrors C's `read_cube` output convention (cvrin.c).
fn push_cube(
    input_chars: &[char],
    output_chars: &[char],
    cover_type: CoverType,
    cubes: &mut Vec<RawCube>,
) -> Result<(), PLAError> {
    // Parse the input field.
    let mut inputs = Vec::with_capacity(input_chars.len());
    for (pos, &ch) in input_chars.iter().enumerate() {
        inputs.push(match ch {
            '0' => Some(false),
            '1' => Some(true),
            '-' | '~' | 'x' | 'X' => None,
            _ => {
                return Err(PLAError::InvalidInputCharacter {
                    character: ch,
                    position: pos,
                })
            }
        });
    }

    // Parse the output field following the Espresso C convention (cvrin.c lines 176-199): the one
    // line yields separate F, D, R cubes — '1'/'4' set an F bit, '0'/'3' an R bit, '-'/'2' a D bit
    // (when the cover type carries that set), and '~' contributes nothing.
    let mut f_outputs = Vec::with_capacity(output_chars.len());
    let mut d_outputs = Vec::with_capacity(output_chars.len());
    let mut r_outputs = Vec::with_capacity(output_chars.len());
    let mut has_f = false;
    let mut has_d = false;
    let mut has_r = false;

    for (pos, &ch) in output_chars.iter().enumerate() {
        match ch {
            '1' | '4' if cover_type.has_f() => {
                f_outputs.push(true);
                d_outputs.push(false);
                r_outputs.push(false);
                has_f = true;
            }
            '0' | '3' if cover_type.has_r() => {
                f_outputs.push(false);
                d_outputs.push(false);
                r_outputs.push(true);
                has_r = true;
            }
            '-' | '2' if cover_type.has_d() => {
                f_outputs.push(false);
                d_outputs.push(true);
                r_outputs.push(false);
                has_d = true;
            }
            '~' | '-' | '2' => {
                // '~' contributes nothing; '-'/'2' with the D set disabled also contribute nothing.
                f_outputs.push(false);
                d_outputs.push(false);
                r_outputs.push(false);
            }
            '0' | '3' => {
                // R-set disabled: an OFF bit sets nothing. ('1'/'4' can't reach here — has_f() is
                // always true, so they always match the first arm.)
                f_outputs.push(false);
                d_outputs.push(false);
                r_outputs.push(false);
            }
            _ => {
                return Err(PLAError::InvalidOutputCharacter {
                    character: ch,
                    position: pos,
                })
            }
        }
    }

    // Add cubes only if they carry meaningful outputs.
    if has_f {
        cubes.push((inputs.clone(), f_outputs, CubeType::F));
    }
    if has_d {
        cubes.push((inputs.clone(), d_outputs, CubeType::D));
    }
    if has_r {
        cubes.push((inputs, r_outputs, CubeType::R));
    }
    Ok(())
}

/// Run `$c` (bound to the inner [`Cover`]) for every [`PlaCover`] variant — used by the accessors and
/// writer that behave identically regardless of which sides are named.
macro_rules! on_inner_cover {
    ($self:expr, $c:ident => $body:expr) => {
        match $self {
            PlaCover::InputsOutputsNamed($c) => $body,
            PlaCover::InputsNamed($c) => $body,
            PlaCover::OutputsNamed($c) => $body,
            PlaCover::Positional($c) => $body,
        }
    };
}

/// Like [`on_inner_cover!`], but **re-wraps** `$body`'s result in the same variant — used by the
/// transforms (e.g. minimisation) that map the inner [`Cover`] and must preserve which sides are named.
macro_rules! map_inner_cover {
    ($self:expr, $c:ident => $body:expr) => {
        match $self {
            PlaCover::InputsOutputsNamed($c) => PlaCover::InputsOutputsNamed($body),
            PlaCover::InputsNamed($c) => PlaCover::InputsNamed($body),
            PlaCover::OutputsNamed($c) => PlaCover::OutputsNamed($body),
            PlaCover::Positional($c) => PlaCover::Positional($body),
        }
    };
}

impl<S: StringLabel> PlaCover<S> {
    /// Parse a `PlaCover` from any `BufRead`, reading label sections into the label type `S`.
    ///
    /// The cubes are read positionally into a `Cover<Anonymous, Anonymous>`, then each present label
    /// section relabels that side, selecting the variant.
    pub fn from_pla_reader<R: std::io::BufRead>(reader: R) -> Result<Self, PLAReadError> {
        let p = parse_pla(reader)?;
        let base = anonymous_cover_from_raw(p.num_inputs, p.num_outputs, p.cubes, p.cover_type);
        let to_syms = |labels: Vec<String>| -> Arc<Symbols<S>> {
            Symbols::new(labels.iter().map(|s| S::from(s.as_str())).collect())
        };
        // `parse_pla` has already checked each present label section against the cube width
        // (PLAError::LabelCountMismatch), so these relabels match by construction.
        let arity = "label sections were validated against the cube width during parsing";
        Ok(match (p.input_labels, p.output_labels) {
            (Some(i), Some(o)) => {
                Self::InputsOutputsNamed(base.relabel(to_syms(i), to_syms(o)).expect(arity))
            }
            (Some(i), None) => Self::InputsNamed(base.relabel_inputs(to_syms(i)).expect(arity)),
            (None, Some(o)) => Self::OutputsNamed(base.relabel_outputs(to_syms(o)).expect(arity)),
            (None, None) => Self::Positional(base),
        })
    }

    /// Parse a `PlaCover` from a PLA-format string.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{PlaCover, Symbol};
    ///
    /// let pla = ".i 2\n.o 1\n.p 1\n01 1\n.e\n";
    /// let cover = PlaCover::<Symbol>::from_pla_string(pla).unwrap();
    /// assert_eq!(cover.num_inputs(), 2);
    /// assert_eq!(cover.num_outputs(), 1);
    /// ```
    pub fn from_pla_string(s: &str) -> Result<Self, PLAReadError> {
        Self::from_pla_reader(Cursor::new(s.as_bytes()))
    }

    /// Load a `PlaCover` from a PLA-format file.
    pub fn from_pla_file<P: AsRef<Path>>(path: P) -> Result<Self, PLAReadError> {
        Self::from_pla_reader(BufReader::new(File::open(path)?))
    }
}

impl<S> PlaCover<S> {
    /// The number of input variables.
    #[must_use]
    pub fn num_inputs(&self) -> usize {
        on_inner_cover!(self, c => c.num_inputs())
    }

    /// The number of output variables.
    #[must_use]
    pub fn num_outputs(&self) -> usize {
        on_inner_cover!(self, c => c.num_outputs())
    }

    /// The number of cubes (counted per the cover type).
    #[must_use]
    pub fn num_cubes(&self) -> usize {
        on_inner_cover!(self, c => c.num_cubes())
    }

    /// The cover type (F/FD/FR/FDR).
    #[must_use]
    pub fn cover_type(&self) -> CoverType {
        on_inner_cover!(self, c => c.cover_type())
    }
}

impl<S: AsRef<str>> PlaCover<S> {
    /// The input labels, or `&[]` when the inputs are positional (no `.ilb` in the file).
    #[must_use]
    pub fn input_labels(&self) -> &[S] {
        match self {
            PlaCover::InputsOutputsNamed(c) => c.input_labels(),
            PlaCover::InputsNamed(c) => c.input_labels(),
            PlaCover::OutputsNamed(_) | PlaCover::Positional(_) => &[],
        }
    }

    /// The output labels, or `&[]` when the outputs are positional (no `.ob` in the file).
    #[must_use]
    pub fn output_labels(&self) -> &[S] {
        match self {
            PlaCover::InputsOutputsNamed(c) => c.output_labels(),
            PlaCover::OutputsNamed(c) => c.output_labels(),
            PlaCover::InputsNamed(_) | PlaCover::Positional(_) => &[],
        }
    }
}

/// Writing dispatches to the inner cover; each variant's named sides emit, positional sides omit.
impl<S: PlaLabel> PLAWriter for PlaCover<S> {
    fn write_pla<W: Write>(
        &self,
        writer: &mut W,
        pla_type: CoverType,
    ) -> Result<(), PLAWriteError> {
        on_inner_cover!(self, c => c.write_pla(writer, pla_type))
    }
}

/// Minimisation preserves which sides are named (the label types are carried through). The fallible
/// `try_*` primitives delegate to the inner cover; the panicking `minimize*` methods are the trait
/// defaults built on top.
impl<S> Minimizable for PlaCover<S> {
    fn try_minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError> {
        Ok(map_inner_cover!(self, c => c.try_minimize_with_config(config)?))
    }

    fn try_minimize_exact_with_config(
        &self,
        config: &EspressoConfig,
    ) -> Result<Self, MinimizationError> {
        Ok(map_inner_cover!(self, c => c.try_minimize_exact_with_config(config)?))
    }
}
