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
/// This trait provides methods for serialising covers to PLA format.
/// It is automatically implemented for all types that implement `PLASerialisable`.
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
        // PLA format is ASCII, so this conversion is safe
        Ok(String::from_utf8(buffer).unwrap())
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

    // Read all lines into memory since we need lookahead for multi-line format
    let lines: Vec<String> = reader.lines().collect::<io::Result<Vec<_>>>()?;
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        i += 1;

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
                    if let Some(type_str) = parts.get(1) {
                        cover_type = match *type_str {
                            "f" => CoverType::F,
                            "fd" => CoverType::FD,
                            "fr" => CoverType::FR,
                            "fdr" => CoverType::FDR,
                            _ => CoverType::F,
                        };
                    }
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

        // Parse cube line(s) - supports both single-line and multi-line formats
        // Some PLA files use | as separator between inputs and outputs
        let (input_part, output_part) = if line.contains('|') {
            let parts: Vec<&str> = line.splitn(2, '|').collect();
            (
                parts.first().copied().unwrap_or(""),
                parts.get(1).copied().unwrap_or(""),
            )
        } else {
            (line, "")
        };

        // Remove ALL whitespace to handle column-based formatting
        // (e.g., files where inputs/outputs are formatted in columns with spaces)
        let line_no_spaces: String = if !output_part.is_empty() {
            // Format with |: remove spaces from each part separately
            let inp = input_part
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>();
            let out = output_part
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>();
            format!("{}{}", inp, out)
        } else {
            // No |: remove all spaces from whole line
            line.chars().filter(|c| !c.is_whitespace()).collect()
        };

        if line_no_spaces.is_empty() {
            continue;
        }

        // Determine input and output strings based on declared dimensions
        let (input_str, output_str) = if let (Some(ni), Some(no)) = (num_inputs, num_outputs) {
            // We know the dimensions, so split at the boundary
            if line_no_spaces.len() >= ni + no {
                // Line has enough characters - split at boundary
                let (inp, out) = line_no_spaces.split_at(ni);
                (inp.to_string(), out.to_string())
            } else {
                // Line too short, might be multi-line format
                let mut accumulated = line_no_spaces.clone();

                // Look ahead to accumulate more lines until we have enough characters
                while accumulated.len() < ni + no && i < lines.len() {
                    let next_line = lines[i].trim();

                    // Skip empty lines
                    if next_line.is_empty() || next_line.starts_with('#') {
                        i += 1;
                        continue;
                    }

                    // Stop at directives
                    if next_line.starts_with('.') {
                        break;
                    }

                    // Remove whitespace from next line and append
                    let next_no_spaces: String =
                        next_line.chars().filter(|c| !c.is_whitespace()).collect();
                    if next_no_spaces.is_empty() {
                        i += 1;
                        continue;
                    }

                    accumulated.push_str(&next_no_spaces);
                    i += 1; // Consume this line

                    if accumulated.len() >= ni + no {
                        break;
                    }
                }

                // Check if we have the right amount of data
                if accumulated.len() < ni + no {
                    // Truncated cube: ran out of input before reaching the declared width.
                    return Err(PLAError::CubeDimensionMismatch {
                        expected_inputs: ni,
                        actual_inputs: accumulated.len().min(ni),
                        expected_outputs: no,
                        actual_outputs: accumulated.len().saturating_sub(ni),
                    }
                    .into());
                }

                // An over-long multi-line cube (more characters than the declared width) is rejected
                // here too, mirroring the single-line path, rather than silently truncating the excess.
                if accumulated.len() > ni + no {
                    return Err(PLAError::CubeDimensionMismatch {
                        expected_inputs: ni,
                        actual_inputs: ni,
                        expected_outputs: no,
                        actual_outputs: accumulated.len() - ni,
                    }
                    .into());
                }
                // Split accumulated data at the input/output boundary (now exactly `ni + no` wide).
                let (inp, out) = accumulated.split_at(ni);
                (inp.to_string(), out.to_string())
            }
        } else {
            // Dimensions not yet known - use whitespace splitting as before
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                // No .i/.o directive declared the dimensions, and this line can't be split into
                // input/output halves to infer them.
                return Err(PLAError::MissingDimensions.into());
            }
            (parts[0].to_string(), parts[1].to_string())
        };

        // Infer dimensions from first cube if not specified
        if num_inputs.is_none() {
            num_inputs = Some(input_str.len());
        }
        if num_outputs.is_none() {
            num_outputs = Some(output_str.len());
        }

        let ni = num_inputs.unwrap();
        let no = num_outputs.unwrap();

        // Verify dimensions are consistent with the declared/inferred width.
        if input_str.len() != ni || output_str.len() != no {
            return Err(PLAError::CubeDimensionMismatch {
                expected_inputs: ni,
                actual_inputs: input_str.len(),
                expected_outputs: no,
                actual_outputs: output_str.len(),
            }
            .into());
        }

        // Parse inputs
        let mut inputs = Vec::with_capacity(ni);
        for (pos, ch) in input_str.chars().enumerate() {
            inputs.push(match ch {
                '0' => Some(false),
                '1' => Some(true),
                '-' | '~' | 'x' | 'X' => None,
                _ => {
                    return Err(PLAError::InvalidInputCharacter {
                        character: ch,
                        position: pos,
                    }
                    .into())
                }
            });
        }

        // Parse outputs following Espresso C convention (cvrin.c lines 176-199)
        // The C code creates separate F, D, R cubes from a single line:
        // - '1' or '4' → bit set in F cube
        // - '0' or '3' → bit set in R cube
        // - '-' or '2' → bit set in D cube (if pla_type includes D_type)
        // - '~' → does NOTHING (cvrin.c line 190: just breaks)
        //
        // Simplified: outputs are Vec<bool> where true = bit set in this cube
        let mut f_outputs = Vec::with_capacity(no);
        let mut d_outputs = Vec::with_capacity(no);
        let mut r_outputs = Vec::with_capacity(no);
        let mut has_f = false;
        let mut has_d = false;
        let mut has_r = false;

        for (pos, ch) in output_str.chars().enumerate() {
            match ch {
                '1' | '4' if cover_type.has_f() => {
                    f_outputs.push(true); // Bit set in F cube
                    d_outputs.push(false); // Not in D cube
                    r_outputs.push(false); // Not in R cube
                    has_f = true;
                }
                '0' | '3' if cover_type.has_r() => {
                    f_outputs.push(false); // Not in F cube
                    d_outputs.push(false); // Not in D cube
                    r_outputs.push(true); // Bit set in R cube
                    has_r = true;
                }
                '-' | '2' if cover_type.has_d() => {
                    // Only '-' and '2' create D cubes, NOT '~'
                    f_outputs.push(false); // Not in F cube
                    d_outputs.push(true); // Bit set in D cube
                    r_outputs.push(false); // Not in R cube
                    has_d = true;
                }
                '~' | '-' | '2' => {
                    // '~' does nothing (C code line 190)
                    // If '-' or '2' but D_type not set, also do nothing
                    f_outputs.push(false);
                    d_outputs.push(false);
                    r_outputs.push(false);
                }
                '1' | '4' | '0' | '3' => {
                    // Type flag not set, don't set bits
                    f_outputs.push(false);
                    d_outputs.push(false);
                    r_outputs.push(false);
                }
                _ => {
                    return Err(PLAError::InvalidOutputCharacter {
                        character: ch,
                        position: pos,
                    }
                    .into())
                }
            }
        }

        // Add cubes only if they have meaningful outputs
        if has_f {
            cubes.push((inputs.clone(), f_outputs, CubeType::F));
        }
        if has_d {
            cubes.push((inputs.clone(), d_outputs, CubeType::D));
        }
        if has_r {
            cubes.push((inputs, r_outputs, CubeType::R));
        }
    }

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
    pub fn num_inputs(&self) -> usize {
        on_inner_cover!(self, c => c.num_inputs())
    }

    /// The number of output variables.
    pub fn num_outputs(&self) -> usize {
        on_inner_cover!(self, c => c.num_outputs())
    }

    /// The number of cubes (counted per the cover type).
    pub fn num_cubes(&self) -> usize {
        on_inner_cover!(self, c => c.num_cubes())
    }

    /// The cover type (F/FD/FR/FDR).
    pub fn cover_type(&self) -> CoverType {
        on_inner_cover!(self, c => c.cover_type())
    }
}

impl<S: AsRef<str>> PlaCover<S> {
    /// The input labels, or `&[]` when the inputs are positional (no `.ilb` in the file).
    pub fn input_labels(&self) -> &[S] {
        match self {
            PlaCover::InputsOutputsNamed(c) => c.input_labels(),
            PlaCover::InputsNamed(c) => c.input_labels(),
            PlaCover::OutputsNamed(_) | PlaCover::Positional(_) => &[],
        }
    }

    /// The output labels, or `&[]` when the outputs are positional (no `.ob` in the file).
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

/// Minimisation preserves which sides are named (the label types are carried through).
impl<S> Minimizable for PlaCover<S> {
    fn minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError> {
        Ok(map_inner_cover!(self, c => c.minimize_with_config(config)?))
    }

    fn minimize_exact_with_config(
        &self,
        config: &EspressoConfig,
    ) -> Result<Self, MinimizationError> {
        Ok(map_inner_cover!(self, c => c.minimize_exact_with_config(config)?))
    }
}
