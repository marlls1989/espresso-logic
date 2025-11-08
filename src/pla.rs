//! PLA (Programmable Logic Array) format support
//!
//! This module handles PLA file I/O and provides `PLACover`, a dynamic cover type
//! for working with PLA files where dimensions are not known at compile time.

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::Path;
use std::sync::Arc;

use crate::cover::{CoverType, Cube, CubeType};
use crate::error::{PLAError, PLAReadError, PLAWriteError};

/// Internal trait for types that can be serialized to and deserialized from PLA format
///
/// This trait provides the necessary methods for both reading (serialization) and
/// constructing (deserialization) types from PLA format. It is used as the basis
/// for the public `PLAReader` and `PLAWriter` traits.
pub(crate) trait PLASerialisable: Sized {
    /// Associated type for iterating over cubes
    type CubesIter<'a>: Iterator<Item = &'a Cube>
    where
        Self: 'a;

    // Read access (for serialization)

    /// Get the number of inputs
    fn num_inputs(&self) -> usize;

    /// Get the number of outputs
    fn num_outputs(&self) -> usize;

    /// Iterate over all cubes (internal use)
    fn internal_cubes_iter(&self) -> Self::CubesIter<'_>;

    /// Get input variable labels if available
    fn get_input_labels(&self) -> Option<&[Arc<str>]>;

    /// Get output variable labels if available
    fn get_output_labels(&self) -> Option<&[Arc<str>]>;

    // Constructor (for deserialization)

    /// Create an instance from parsed PLA components
    fn create_from_pla_parts(
        num_inputs: usize,
        num_outputs: usize,
        input_labels: Vec<Arc<str>>,
        output_labels: Vec<Arc<str>>,
        cubes: Vec<Cube>,
        cover_type: CoverType,
    ) -> Self;
}

/// Trait for types that support PLA serialization (writing)
///
/// This trait provides methods for serializing covers to PLA format.
/// It is automatically implemented for all types that implement `PLASerialisable`.
pub trait PLAWriter {
    /// Write this cover to PLA format using a writer
    ///
    /// This is the core serialization method that writes directly to any `Write` implementation.
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

/// Blanket implementation of PLAWriter for all PLASerialisable types
impl<T: PLASerialisable> PLAWriter for T {
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

        // Write PLA header
        writeln!(writer, ".i {}", self.num_inputs())?;

        // Write input labels if available
        if let Some(labels) = self.get_input_labels() {
            write!(writer, ".ilb")?;
            for label in labels {
                write!(writer, " {}", label)?;
            }
            writeln!(writer)?;
        }

        writeln!(writer, ".o {}", self.num_outputs())?;

        // Write output labels if available
        if let Some(labels) = self.get_output_labels() {
            write!(writer, ".ob")?;
            for label in labels {
                write!(writer, " {}", label)?;
            }
            writeln!(writer)?;
        }

        // Filter cubes based on output type using cube_type field
        let filtered_cubes: Vec<_> = self
            .internal_cubes_iter()
            .filter(|cube| {
                match pla_type {
                    CoverType::F => cube.cube_type == CubeType::F,
                    CoverType::FD => cube.cube_type == CubeType::F || cube.cube_type == CubeType::D,
                    CoverType::FR => cube.cube_type == CubeType::F || cube.cube_type == CubeType::R,
                    CoverType::FDR => true, // All cubes
                }
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

            // Encode outputs based on cube type and output format
            // With bool outputs: true = bit set in this cube, false = bit not set
            match pla_type {
                CoverType::F => {
                    // F-type: '1' for bit set, '0' for bit not set
                    for &out in cube.outputs.iter() {
                        write!(writer, "{}", if out { '1' } else { '0' })?;
                    }
                }
                CoverType::FD | CoverType::FDR | CoverType::FR => {
                    // Use cube_type to determine character for set/unset bits
                    let (set_char, unset_char) = match cube.cube_type {
                        CubeType::F => ('1', '~'), // F cube: 1=ON, ~=not in cube
                        CubeType::D => ('2', '~'), // D cube: 2=DC, ~=not in cube
                        CubeType::R => ('0', '~'), // R cube: 0=OFF, ~=not in cube
                    };

                    for &out in cube.outputs.iter() {
                        write!(writer, "{}", if out { set_char } else { unset_char })?;
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

/// Trait for types that support PLA deserialization (reading/parsing)
///
/// This trait provides methods for deserializing covers from PLA format.
/// It is automatically implemented for all types that implement `PLASerialisable`.
///
/// The trait provides default implementations for convenience methods that
/// delegate to the core `from_pla_reader` method.
pub trait PLAReader: Sized {
    /// Parse a cover from a PLA format reader
    ///
    /// This is the core deserialization method that reads from any `BufRead` implementation.
    /// Both `from_pla_string` and `from_pla_file` delegate to this method.
    fn from_pla_reader<R: std::io::BufRead>(reader: R) -> Result<Self, PLAReadError>;

    /// Parse a cover from a PLA format string
    ///
    /// This is a convenience method that delegates to `from_pla_reader`.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cover, PLAReader};
    ///
    /// let pla = ".i 2\n.o 1\n.p 1\n01 1\n.e\n";
    /// let cover = Cover::from_pla_string(pla).unwrap();
    /// assert_eq!(cover.num_inputs(), 2);
    /// assert_eq!(cover.num_outputs(), 1);
    /// ```
    fn from_pla_string(s: &str) -> Result<Self, PLAReadError> {
        use std::io::Cursor;
        let cursor = Cursor::new(s.as_bytes());
        Self::from_pla_reader(cursor)
    }

    /// Load a cover from a PLA format file
    ///
    /// This is a convenience method that delegates to `from_pla_reader`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use espresso_logic::{Cover, PLAReader};
    ///
    /// let cover = Cover::from_pla_file("input.pla").unwrap();
    /// println!("Loaded {} inputs, {} outputs", cover.num_inputs(), cover.num_outputs());
    /// ```
    fn from_pla_file<P: AsRef<Path>>(path: P) -> Result<Self, PLAReadError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Self::from_pla_reader(reader)
    }
}

/// Blanket implementation of PLAReader for all PLASerialisable types
impl<T: PLASerialisable> PLAReader for T {
    fn from_pla_reader<R: std::io::BufRead>(reader: R) -> Result<Self, PLAReadError> {
        let mut num_inputs: Option<usize> = None;
        let mut num_outputs: Option<usize> = None;
        let mut cubes = Vec::new();
        // Default to FD_type to match C espresso behavior (main.c line 21)
        // This causes '-' in outputs to be parsed as D cubes, not just don't-care bits
        let mut cover_type = CoverType::FD;
        let mut input_labels: Option<Vec<Arc<str>>> = None;
        let mut output_labels: Option<Vec<Arc<str>>> = None;

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
                                    value: parts.get(1).unwrap_or(&"").to_string(),
                                }
                            })?;
                        num_inputs = Some(val);
                    }
                    Some(".o") => {
                        let val: usize =
                            parts.get(1).and_then(|s| s.parse().ok()).ok_or_else(|| {
                                PLAError::InvalidOutputDirective {
                                    value: parts.get(1).unwrap_or(&"").to_string(),
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
                        let labels: Vec<Arc<str>> =
                            parts.iter().skip(1).map(|s| Arc::from(*s)).collect();
                        if !labels.is_empty() {
                            input_labels = Some(labels);
                        }
                    }
                    Some(".ob") => {
                        // Parse output labels: .ob label1 label2 label3 ...
                        let labels: Vec<Arc<str>> =
                            parts.iter().skip(1).map(|s| Arc::from(*s)).collect();
                        if !labels.is_empty() {
                            output_labels = Some(labels);
                        }
                    }
                    Some(".e") => break,
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
                if line_no_spaces.len() < ni + no {
                    // Line too short, might be continuation or malformed - try multi-line format
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.is_empty() {
                        continue;
                    }

                    // Multi-line format: accumulate input lines, then get output line
                    let mut input_accumulator = parts[0].to_string();
                    let mut output_line = String::new();

                    // Look ahead to accumulate more input lines and find output
                    while i < lines.len() {
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

                        let next_parts: Vec<&str> = next_line.split_whitespace().collect();
                        if next_parts.is_empty() {
                            i += 1;
                            continue;
                        }

                        let part = next_parts[0];

                        // Check if this looks like an output line
                        // Output lines have exact length matching num_outputs and mostly 0/1/~
                        let is_output = part.len() == no;

                        if is_output {
                            output_line = part.to_string();
                            i += 1; // Consume this line
                            break;
                        } else {
                            // Accumulate more input
                            input_accumulator.push_str(part);
                            i += 1; // Consume this line
                        }
                    }

                    if output_line.is_empty() {
                        continue; // Skip malformed cubes
                    }

                    (input_accumulator, output_line)
                } else {
                    // Line has enough characters - split at boundary
                    let (inp, out) = line_no_spaces.split_at(ni);
                    (inp.to_string(), out.to_string())
                }
            } else {
                // Dimensions not yet known - use whitespace splitting as before
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 2 {
                    continue; // Need at least inputs and outputs
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

            // Verify dimensions are consistent
            if input_str.len() != ni || output_str.len() != no {
                // Skip cubes with wrong dimensions (might be intermediate lines)
                continue;
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
                cubes.push(Cube::new(&inputs, &f_outputs, CubeType::F));
            }
            if has_d {
                cubes.push(Cube::new(&inputs, &d_outputs, CubeType::D));
            }
            if has_r {
                cubes.push(Cube::new(&inputs, &r_outputs, CubeType::R));
            }
        }

        // Verify we got dimensions
        let num_inputs = num_inputs.ok_or(PLAError::MissingInputDirective)?;
        let num_outputs = num_outputs.ok_or(PLAError::MissingOutputDirective)?;

        // Validate label counts if present
        if let Some(ref labels) = input_labels {
            if labels.len() != num_inputs {
                return Err(PLAError::LabelCountMismatch {
                    label_type: "input".to_string(),
                    expected: num_inputs,
                    actual: labels.len(),
                }
                .into());
            }
        }
        if let Some(ref labels) = output_labels {
            if labels.len() != num_outputs {
                return Err(PLAError::LabelCountMismatch {
                    label_type: "output".to_string(),
                    expected: num_outputs,
                    actual: labels.len(),
                }
                .into());
            }
        }

        // Generate default labels if not provided
        let input_labels = input_labels.unwrap_or_else(|| {
            (0..num_inputs)
                .map(|i| Arc::from(format!("x{}", i).as_str()))
                .collect()
        });
        let output_labels = output_labels.unwrap_or_else(|| {
            (0..num_outputs)
                .map(|i| Arc::from(format!("y{}", i).as_str()))
                .collect()
        });

        // Construct the type using the trait method
        Ok(T::create_from_pla_parts(
            num_inputs,
            num_outputs,
            input_labels,
            output_labels,
            cubes,
            cover_type,
        ))
    }
}
