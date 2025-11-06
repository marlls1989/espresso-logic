//! PLA (Programmable Logic Array) format support
//!
//! This module handles PLA file I/O and provides `PLACover`, a dynamic cover type
//! for working with PLA files where dimensions are not known at compile time.

use std::io;
use std::path::Path;

use crate::cover::{Cube, CubeType, Minimizable, PLAType};

/// Trait for types that support PLA serialization
pub(crate) trait PLASerializable: Minimizable {
    /// Write this cover to PLA format string
    fn to_pla_string(&self, pla_type: PLAType) -> io::Result<String> {
        let mut output = String::new();

        // Write .type directive first for FD, FR, FDR (matching C output order)
        match pla_type {
            PLAType::FD => output.push_str(".type fd\n"),
            PLAType::FR => output.push_str(".type fr\n"),
            PLAType::FDR => output.push_str(".type fdr\n"),
            PLAType::F => {} // F is default, no .type needed
        }

        // Write PLA header
        output.push_str(&format!(".i {}\n", self.num_inputs()));
        output.push_str(&format!(".o {}\n", self.num_outputs()));

        // Filter cubes based on output type using cube_type field
        let filtered_cubes: Vec<_> = self
            .internal_cubes_iter()
            .filter(|cube| {
                match pla_type {
                    PLAType::F => cube.cube_type == CubeType::F,
                    PLAType::FD => cube.cube_type == CubeType::F || cube.cube_type == CubeType::D,
                    PLAType::FR => cube.cube_type == CubeType::F || cube.cube_type == CubeType::R,
                    PLAType::FDR => true, // All cubes
                }
            })
            .collect();

        // Add .p directive with filtered cube count
        output.push_str(&format!(".p {}\n", filtered_cubes.len()));

        // Write filtered cubes
        for cube in filtered_cubes {
            // Write inputs
            for inp in cube.inputs.iter() {
                output.push(match inp {
                    Some(false) => '0',
                    Some(true) => '1',
                    None => '-',
                });
            }

            output.push(' ');

            // Encode outputs based on cube type and output format
            // With bool outputs: true = bit set in this cube, false = bit not set
            match pla_type {
                PLAType::F => {
                    // F-type: '1' for bit set, '0' for bit not set
                    for &out in cube.outputs.iter() {
                        output.push(if out { '1' } else { '0' });
                    }
                }
                PLAType::FD | PLAType::FDR | PLAType::FR => {
                    // Use cube_type to determine character for set/unset bits
                    let (set_char, unset_char) = match cube.cube_type {
                        CubeType::F => ('1', '~'), // F cube: 1=ON, ~=not in cube
                        CubeType::D => ('2', '~'), // D cube: 2=DC, ~=not in cube
                        CubeType::R => ('0', '~'), // R cube: 0=OFF, ~=not in cube
                    };

                    for &out in cube.outputs.iter() {
                        output.push(if out { set_char } else { unset_char });
                    }
                }
            }

            output.push('\n');
        }

        // C version uses ".e" for F-type, ".end" for FD/FR/FDR types
        match pla_type {
            PLAType::F => output.push_str(".e\n"),
            _ => output.push_str(".end\n"),
        }
        Ok(output)
    }

    /// Write this cover to a PLA file
    fn to_pla_file<P: AsRef<Path>>(&self, path: P, pla_type: PLAType) -> io::Result<()> {
        let content = self.to_pla_string(pla_type)?;
        std::fs::write(path, content)
    }
}

/// Parse a PLA format string into cube data
///
/// Returns (num_inputs, num_outputs, cubes, cover_type)
pub(crate) fn parse_pla_content(content: &str) -> io::Result<(usize, usize, Vec<Cube>, PLAType)> {
    let mut num_inputs: Option<usize> = None;
    let mut num_outputs: Option<usize> = None;
    let mut cubes = Vec::new();
    // Default to FD_type to match C espresso behavior (main.c line 21)
    // This causes '-' in outputs to be parsed as D cubes, not just don't-care bits
    let mut pla_type = PLAType::FD;

    let lines: Vec<&str> = content.lines().collect();
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
                            io::Error::new(io::ErrorKind::InvalidData, "Invalid .i directive")
                        })?;
                    num_inputs = Some(val);
                }
                Some(".o") => {
                    let val: usize =
                        parts.get(1).and_then(|s| s.parse().ok()).ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "Invalid .o directive")
                        })?;
                    num_outputs = Some(val);
                }
                Some(".type") => {
                    if let Some(type_str) = parts.get(1) {
                        pla_type = match *type_str {
                            "f" => PLAType::F,
                            "fd" => PLAType::FD,
                            "fr" => PLAType::FR,
                            "fdr" => PLAType::FDR,
                            _ => PLAType::F,
                        };
                    }
                }
                Some(".e") => break,
                Some(".ilb") | Some(".ob") | Some(".p") => {}
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
        for ch in input_str.chars() {
            inputs.push(match ch {
                '0' => Some(false),
                '1' => Some(true),
                '-' | '~' | 'x' | 'X' => None,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid input character: '{}'", ch),
                    ))
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

        for ch in output_str.chars() {
            match ch {
                '1' | '4' if pla_type.has_f() => {
                    f_outputs.push(true); // Bit set in F cube
                    d_outputs.push(false); // Not in D cube
                    r_outputs.push(false); // Not in R cube
                    has_f = true;
                }
                '0' | '3' if pla_type.has_r() => {
                    f_outputs.push(false); // Not in F cube
                    d_outputs.push(false); // Not in D cube
                    r_outputs.push(true); // Bit set in R cube
                    has_r = true;
                }
                '-' | '2' if pla_type.has_d() => {
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
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid output character: '{}'", ch),
                    ))
                }
            }
        }

        // Add cubes only if they have meaningful outputs
        if has_f {
            cubes.push(Cube::new(inputs.clone(), f_outputs, CubeType::F));
        }
        if has_d {
            cubes.push(Cube::new(inputs.clone(), d_outputs, CubeType::D));
        }
        if has_r {
            cubes.push(Cube::new(inputs, r_outputs, CubeType::R));
        }
    }

    // Verify we got dimensions
    let ni = num_inputs.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "PLA file missing .i directive and no cubes to infer from",
        )
    })?;
    let no = num_outputs.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "PLA file missing .o directive and no cubes to infer from",
        )
    })?;

    Ok((ni, no, cubes, pla_type))
}

// ============================================================================
// PLACover - Dynamic Cover for PLA Files
// ============================================================================

/// A cover with dynamic dimensions (from PLA files)
///
/// Use this when loading PLA files where dimensions are not known at compile time.
/// Outputs are `Option<bool>`: Some(true)=1, Some(false)=0, None=don't-care
#[derive(Clone)]
pub struct PLACover {
    num_inputs: usize,
    num_outputs: usize,
    /// Cubes with their type (F/D/R) and data
    cubes: Vec<Cube>,
    /// Cover type (F, FD, FR, or FDR)
    cover_type: PLAType,
}

impl PLACover {
    /// Create a new empty cover with specified dimensions
    pub fn new(num_inputs: usize, num_outputs: usize) -> Self {
        PLACover {
            num_inputs,
            num_outputs,
            cubes: Vec::new(),
            cover_type: PLAType::F,
        }
    }

    /// Load a cover from a PLA format file
    ///
    /// The dimensions are determined from the PLA file.
    pub fn from_pla_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_pla_content(&content)
    }

    /// Load a cover from PLA format string
    ///
    /// The dimensions are determined from the PLA content.
    pub fn from_pla_content(content: &str) -> io::Result<Self> {
        let (num_inputs, num_outputs, cubes, cover_type) = parse_pla_content(content)?;

        Ok(PLACover {
            num_inputs,
            num_outputs,
            cubes,
            cover_type,
        })
    }
}

// Implement Minimizable for PLACover (Cover trait is auto-implemented via blanket impl)
impl Minimizable for PLACover {
    fn num_inputs(&self) -> usize {
        self.num_inputs
    }

    fn num_outputs(&self) -> usize {
        self.num_outputs
    }

    fn cover_type(&self) -> PLAType {
        self.cover_type
    }

    fn internal_cubes_iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cube> + 'a> {
        Box::new(self.cubes.iter())
    }

    fn set_cubes(&mut self, cubes: Vec<Cube>) {
        self.cubes = cubes;
    }
}

// Implement PLASerializable for PLACover
impl PLASerializable for PLACover {}
