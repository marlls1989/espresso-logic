//! # Espresso Logic Minimizer
//!
//! This crate provides Rust bindings to the Espresso heuristic logic minimizer
//! (Version 2.3), a classic tool from UC Berkeley for minimizing Boolean functions.
//!
//! ## Overview
//!
//! Espresso takes a Boolean function represented as a sum-of-products (cover) and
//! produces a minimal or near-minimal equivalent representation. It's particularly
//! useful for:
//!
//! - Digital logic synthesis
//! - PLA (Programmable Logic Array) minimization
//! - Boolean function simplification
//! - Logic optimization in CAD tools
//!
//! ## Example
//!
//! ```
//! use espresso_logic::{Cover, CoverBuilder};
//!
//! # fn main() -> std::io::Result<()> {
//! // Create a cover for a 2-input, 1-output function
//! let mut cover = CoverBuilder::<2, 1>::new();
//!
//! // Build the ON-set (truth table)
//! cover.add_cube(&[Some(false), Some(true)], &[Some(true)]); // 01 -> 1 (XOR)
//! cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // 10 -> 1 (XOR)
//!
//! // Minimize - runs in isolated process
//! cover.minimize()?;
//!
//! // Use the result
//! println!("Minimized to {} cubes", cover.num_cubes());
//! # Ok(())
//! # }
//! ```
//!
//! ## PLA File Format
//!
//! Covers can also read and write PLA files, a standard format for representing
//! Boolean functions:
//!
//! ```
//! use espresso_logic::{Cover, PLACover, PLAType};
//! # use std::io::Write;
//!
//! # fn main() -> std::io::Result<()> {
//! # let mut temp = tempfile::NamedTempFile::new()?;
//! # temp.write_all(b".i 2\n.o 1\n.p 1\n01 1\n.e\n")?;
//! # temp.flush()?;
//! # let input_path = temp.path();
//! // Read from PLA file
//! let mut cover = PLACover::from_pla_file(input_path)?;
//!
//! // Minimize
//! cover.minimize()?;
//!
//! # let output_file = tempfile::NamedTempFile::new()?;
//! # let output_path = output_file.path();
//! // Write to PLA file
//! cover.to_pla_file(output_path, PLAType::F)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Thread Safety and Concurrency
//!
//! **This library IS thread-safe!** The API uses **transparent process isolation** where
//! the underlying C library runs in isolated forked processes. The parent process never
//! touches global state, making concurrent use completely safe.
//!
//! ### Multi-threaded Applications
//!
//! Just use `CoverBuilder` directly - each thread creates its own cover:
//!
//! ```
//! use espresso_logic::{Cover, CoverBuilder};
//! use std::thread;
//!
//! # fn main() -> std::io::Result<()> {
//! // Spawn threads - no synchronization needed!
//! let handles: Vec<_> = (0..4).map(|_| {
//!     thread::spawn(move || {
//!         // Each thread creates its own cover
//!         let mut cover = CoverBuilder::<2, 1>::new();
//!         cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
//!         cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
//!         
//!         // Each operation runs in an isolated process
//!         cover.minimize()?;
//!         Ok(cover.num_cubes())
//!     })
//! }).collect();
//!
//! for handle in handles {
//!     let result: std::io::Result<usize> = handle.join().unwrap();
//!     println!("Result: {} cubes", result?);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! **How it works:**
//! - **No global state** in parent process
//! - **Process isolation**: Each operation runs in a forked worker process
//! - **Automatic cleanup**: Workers terminate after each operation
//! - **Efficient IPC**: Uses shared memory for fast communication

pub mod sys;

// Process isolation modules (internal)
mod conversion;
mod ipc;
mod unsafe_espresso;
mod worker;

// Re-export commonly used constants for CLI
pub use sys::{ESSEN, EXPAND, GASP, IRRED, MINCOV, REDUCE, SHARP, SPARSE};

/// Worker mode detection - steals execution before main() if running as worker
#[ctor::ctor]
fn check_worker_mode() {
    if std::env::args().any(|arg| arg == "__ESPRESSO_WORKER__") {
        // We're running as a worker process - handle requests and exit
        worker::run_worker_loop();
        std::process::exit(0);
    }
}

use std::fmt;
use std::io;
use std::os::raw::c_int;
use std::path::Path;
use std::sync::Arc;

/// Type alias for complex cube iterator return type
type CubeIterator<'a> = Box<dyn Iterator<Item = (Vec<Option<bool>>, Vec<Option<bool>>)> + 'a>;

/// Represents the type of PLA output format (also used as cover type)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PLAType {
    /// On-set only (F)
    F = 1,
    /// On-set and don't-care set (FD)
    FD = 3,
    /// On-set and off-set (FR)
    FR = 5,
    /// On-set, don't-care set, and off-set (FDR)
    FDR = 7,
}

impl PLAType {
    /// Check if this type includes F (ON-set)
    pub fn has_f(&self) -> bool {
        matches!(self, PLAType::F | PLAType::FD | PLAType::FR | PLAType::FDR)
    }

    /// Check if this type includes D (don't-care set)
    pub fn has_d(&self) -> bool {
        matches!(self, PLAType::FD | PLAType::FDR)
    }

    /// Check if this type includes R (OFF-set)
    pub fn has_r(&self) -> bool {
        matches!(self, PLAType::FR | PLAType::FDR)
    }
}

/// Configuration for the Espresso algorithm
#[derive(Debug, Clone)]
pub struct EspressoConfig {
    /// Enable debugging output
    pub debug: bool,
    /// Verbose debugging
    pub verbose_debug: bool,
    /// Print trace information
    pub trace: bool,
    /// Print summary information
    pub summary: bool,
    /// Remove essential primes
    pub remove_essential: bool,
    /// Force irredundant
    pub force_irredundant: bool,
    /// Unwrap onset
    pub unwrap_onset: bool,
    /// Single expand mode (fast)
    pub single_expand: bool,
    /// Use super gasp
    pub use_super_gasp: bool,
    /// Use random order
    pub use_random_order: bool,
}

impl Default for EspressoConfig {
    fn default() -> Self {
        // Match C defaults from main.c lines 51-72
        EspressoConfig {
            debug: false,
            verbose_debug: false,
            trace: false,
            summary: false,
            remove_essential: true,
            force_irredundant: true,
            unwrap_onset: true,
            single_expand: false,
            use_super_gasp: false,
            use_random_order: false,
        }
    }
}

impl EspressoConfig {
    /// Create a new configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }
}

/// Unified function to decode all three covers from worker into typed Cubes
/// Takes F, D, R serialized covers and produces a Vec<Cube> with correct types
fn decode_worker_result(
    f_serialized: &ipc::SerializedCover,
    d_serialized: Option<&ipc::SerializedCover>,
    r_serialized: Option<&ipc::SerializedCover>,
    num_inputs: usize,
    num_outputs: usize,
) -> Vec<Cube> {
    let mut result = Vec::new();

    // Decode F cubes
    for cube_data in &f_serialized.cubes {
        if let Some((inputs, outputs)) = decode_cube(cube_data, num_inputs, num_outputs) {
            result.push(Cube::new(inputs, outputs, CubeType::F));
        }
    }

    // Decode D cubes if available
    if let Some(d_ser) = d_serialized {
        for cube_data in &d_ser.cubes {
            if let Some((inputs, outputs)) = decode_cube(cube_data, num_inputs, num_outputs) {
                result.push(Cube::new(inputs, outputs, CubeType::D));
            }
        }
    }

    // Decode R cubes if available
    if let Some(r_ser) = r_serialized {
        for cube_data in &r_ser.cubes {
            if let Some((inputs, outputs)) = decode_cube(cube_data, num_inputs, num_outputs) {
                result.push(Cube::new(inputs, outputs, CubeType::R));
            }
        }
    }

    result
}

/// Helper function to decode a single cube from serialized data
/// Returns None if cube has no set bits (shouldn't happen)
fn decode_cube(
    cube_data: &ipc::SerializedCube,
    num_inputs: usize,
    num_outputs: usize,
) -> Option<(Vec<Option<bool>>, Vec<bool>)> {
    let mut inputs = Vec::with_capacity(num_inputs);
    let mut outputs = Vec::with_capacity(num_outputs);

    // Decode inputs (binary variables - 2 bits each)
    for var in 0..num_inputs {
        let bit0 = var * 2;
        let bit1 = var * 2 + 1;

        let word0 = (bit0 >> 5) + 1;
        let b0 = bit0 & 31;
        let word1 = (bit1 >> 5) + 1;
        let b1 = bit1 & 31;

        let has_bit0 = (cube_data.data.get(word0).copied().unwrap_or(0) & (1 << b0)) != 0;
        let has_bit1 = (cube_data.data.get(word1).copied().unwrap_or(0) & (1 << b1)) != 0;

        inputs.push(match (has_bit0, has_bit1) {
            (false, false) => None,
            (true, false) => Some(false),
            (false, true) => Some(true),
            (true, true) => None, // don't care
        });
    }

    // Decode outputs (multi-valued variable - 1 bit per value)
    // Simplified: bit set → true, bit not set → false
    let output_start = num_inputs * 2;
    for out in 0..num_outputs {
        let bit = output_start + out;
        let word = (bit >> 5) + 1;
        let b = bit & 31;
        let val = (cube_data.data.get(word).copied().unwrap_or(0) & (1 << b)) != 0;

        outputs.push(val);
    }

    Some((inputs, outputs))
}

/// Internal trait for types that can be minimized
/// Contains implementation details needed by the minimization algorithm
trait Minimizable: Send + Sync {
    /// Get the number of inputs (required for minimization)
    fn num_inputs(&self) -> usize;

    /// Get the number of outputs (required for minimization)
    fn num_outputs(&self) -> usize;

    /// Get the cover type (required for minimization)
    fn cover_type(&self) -> PLAType;

    /// Iterate over typed cubes (internal use only)
    fn internal_cubes_iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cube> + 'a>;

    /// Set cubes after minimization (internal use)
    fn set_cubes(&mut self, cubes: Vec<Cube>);
}

/// Public trait for all cover types (static and dynamic dimensions)
pub trait Cover: Send + Sync {
    /// Get the number of inputs
    fn num_inputs(&self) -> usize;

    /// Get the number of outputs  
    fn num_outputs(&self) -> usize;

    /// Get the number of cubes (for F/FD types, only counts F cubes; for FR/FDR, counts all)
    fn num_cubes(&self) -> usize;

    /// Get the cover type (F, FD, FR, or FDR)
    fn cover_type(&self) -> PLAType;

    /// Iterate over cubes (inputs, outputs)
    /// Returns cubes in same format as add_cube takes (owned vecs for easy use)
    fn cubes_iter<'a>(&'a self) -> CubeIterator<'a>;

    /// Minimize this cover in-place using default configuration
    fn minimize(&mut self) -> io::Result<()>;

    /// Minimize this cover in-place with custom configuration
    fn minimize_with_config(&mut self, config: &EspressoConfig) -> io::Result<()>;

    /// Write this cover to PLA format string
    fn to_pla_string(&self, pla_type: PLAType) -> io::Result<String>;

    /// Write this cover to a PLA file
    fn to_pla_file<P: AsRef<Path>>(&self, path: P, pla_type: PLAType) -> io::Result<()>;
}

/// Blanket implementation: Cover for all Minimizable types
impl<T: Minimizable> Cover for T {
    fn num_inputs(&self) -> usize {
        Minimizable::num_inputs(self)
    }

    fn num_outputs(&self) -> usize {
        Minimizable::num_outputs(self)
    }

    fn num_cubes(&self) -> usize {
        let cover_type = self.cover_type();
        if cover_type.has_r() {
            self.internal_cubes_iter().count()
        } else {
            // F/FD: only count F cubes
            self.internal_cubes_iter()
                .filter(|cube| cube.cube_type == CubeType::F)
                .count()
        }
    }

    fn cover_type(&self) -> PLAType {
        Minimizable::cover_type(self)
    }

    fn cubes_iter<'a>(&'a self) -> CubeIterator<'a> {
        // Convert internal Cube structs to public API format
        // Only return F cubes for F-type covers, all cubes for FD/FR/FDR
        let cover_type = self.cover_type();
        Box::new(
            self.internal_cubes_iter()
                .filter(move |cube| {
                    // For F-type, only return F cubes; for FD/FR/FDR, return all
                    cover_type != PLAType::F || cube.cube_type == CubeType::F
                })
                .map(|cube| {
                    // Convert bool outputs back to Option<bool> for public API
                    let inputs = cube.inputs.to_vec();
                    let outputs: Vec<Option<bool>> =
                        cube.outputs.iter().map(|&b| Some(b)).collect();
                    (inputs, outputs)
                }),
        )
    }

    fn minimize(&mut self) -> io::Result<()> {
        let config = EspressoConfig::default();
        self.minimize_with_config(&config)
    }

    fn minimize_with_config(&mut self, config: &EspressoConfig) -> io::Result<()> {
        use worker::Worker;

        // Convert config
        let ipc_config = ipc::IpcConfig {
            debug: config.debug,
            verbose_debug: config.verbose_debug,
            trace: config.trace,
            summary: config.summary,
            remove_essential: config.remove_essential,
            force_irredundant: config.force_irredundant,
            unwrap_onset: config.unwrap_onset,
            single_expand: config.single_expand,
            use_super_gasp: config.use_super_gasp,
            use_random_order: config.use_random_order,
        };

        // Split cubes into F, D, R sets for worker based on cube type
        // With typed cubes, this is now simple - just group by type
        let mut f_cubes = Vec::new();
        let mut d_cubes = Vec::new();
        let mut r_cubes = Vec::new();

        for cube in self.internal_cubes_iter() {
            let input_vec: Vec<u8> = cube
                .inputs
                .iter()
                .map(|&opt| match opt {
                    Some(false) => 0,
                    Some(true) => 1,
                    None => 2,
                })
                .collect();

            // Convert outputs: true → 1, false → 0
            let output_vec: Vec<u8> = cube
                .outputs
                .iter()
                .map(|&b| if b { 1 } else { 0 })
                .collect();

            // Send to appropriate set based on cube type
            match cube.cube_type {
                CubeType::F => f_cubes.push((input_vec, output_vec)),
                CubeType::D => d_cubes.push((input_vec, output_vec)),
                CubeType::R => r_cubes.push((input_vec, output_vec)),
            }
        }

        // Call worker with appropriate sets based on cover type
        let (f_serialized, d_serialized, r_serialized) = Worker::execute_minimize(
            self.num_inputs(),
            self.num_outputs(),
            ipc_config,
            f_cubes,
            if d_cubes.is_empty() {
                None
            } else {
                Some(d_cubes)
            },
            if r_cubes.is_empty() {
                None
            } else {
                Some(r_cubes)
            },
        )?;

        // Decode worker results into typed Cubes
        let all_cubes = decode_worker_result(
            &f_serialized,
            d_serialized.as_ref(),
            r_serialized.as_ref(),
            self.num_inputs(),
            self.num_outputs(),
        );

        // Update cubes with type information preserved
        self.set_cubes(all_cubes);
        Ok(())
    }

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

    fn to_pla_file<P: AsRef<Path>>(&self, path: P, pla_type: PLAType) -> io::Result<()> {
        let content = self.to_pla_string(pla_type)?;
        std::fs::write(path, content)
    }
}

/// A cover builder with compile-time dimension checking
///
/// Uses const generics for ergonomic hand-construction of covers.
/// For loading from PLA files with dynamic dimensions, use `PLACover::from_pla_*`.
///
/// The `T` type parameter specifies the cover type and defaults to `FDType` (ON-set + Don't-care).
///
/// # Examples
///
/// ```
/// use espresso_logic::{CoverBuilder, Cover, FType, FDType};
///
/// # fn main() -> std::io::Result<()> {
/// // Create a cover for a 2-input, 1-output function (FD type by default)
/// let mut cover = CoverBuilder::<2, 1>::new();
///
/// // Build the function (XOR)
/// cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);  // 01 -> 1
/// cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);  // 10 -> 1
///
/// // Or create an F-type cover explicitly
/// let mut f_cover = CoverBuilder::<2, 1, FType>::new();
/// f_cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
///
/// // Minimize it
/// cover.minimize()?;
///
/// // Read the result
/// println!("Minimized to {} cubes", cover.num_cubes());
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CoverBuilder<const INPUTS: usize, const OUTPUTS: usize, T: CoverTypeMarker = FDType> {
    /// Cube data stored internally as typed cubes
    cubes: Vec<Cube>,
    _marker: std::marker::PhantomData<T>,
}

/// Marker trait for cover type specification
pub trait CoverTypeMarker: Send + Sync + Clone {
    const PLA_TYPE: PLAType;
}

/// Marker type for F (ON-set only) covers
#[derive(Clone, Copy, Debug)]
pub struct FType;
impl CoverTypeMarker for FType {
    const PLA_TYPE: PLAType = PLAType::F;
}

/// Marker type for FD (ON-set + Don't-care) covers (default)
#[derive(Clone, Copy, Debug)]
pub struct FDType;
impl CoverTypeMarker for FDType {
    const PLA_TYPE: PLAType = PLAType::FD;
}

/// Marker type for FR (ON-set + OFF-set) covers
#[derive(Clone, Copy, Debug)]
pub struct FRType;
impl CoverTypeMarker for FRType {
    const PLA_TYPE: PLAType = PLAType::FR;
}

/// Marker type for FDR (ON-set + Don't-care + OFF-set) covers
#[derive(Clone, Copy, Debug)]
pub struct FDRType;
impl CoverTypeMarker for FDRType {
    const PLA_TYPE: PLAType = PLAType::FDR;
}

/// Type of a cube (ON-set, DC-set, or OFF-set)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CubeType {
    F, // ON-set cube
    D, // Don't-care set cube
    R, // OFF-set cube
}

/// A cube in a PLA cover
#[derive(Clone, Debug)]
pub struct Cube {
    pub(crate) inputs: Arc<[Option<bool>]>,
    pub(crate) outputs: Arc<[bool]>, // Simplified: true = bit set, false = bit not set
    pub(crate) cube_type: CubeType,
}

impl Cube {
    fn new(inputs: Vec<Option<bool>>, outputs: Vec<bool>, cube_type: CubeType) -> Self {
        Cube {
            inputs: inputs.into(),
            outputs: outputs.into(),
            cube_type,
        }
    }
}

/// A cover with dynamic dimensions (from PLA files)
///
/// Use this when loading PLA files where dimensions are not known at compile time.
/// Outputs are Option<bool>: Some(true)=1, Some(false)=0, None=don't-care
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

        // Don't merge cubes - the C code keeps F, D, R in separate cover structures
        // (PLA->F, PLA->D, PLA->R), and espresso() handles any necessary merging internally.
        // If we merge here, we lose the separation between F, D, and R cubes.
        Ok(PLACover {
            num_inputs: ni,
            num_outputs: no,
            cubes,
            cover_type: pla_type,
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

/// Internal cover with raw C pointer (only used in workers)
struct UnsafeCover {
    ptr: sys::pset_family,
}

impl UnsafeCover {
    /// Create a new empty cover
    fn new(capacity: usize, cube_size: usize) -> Self {
        let ptr = unsafe { sys::sf_new(capacity as c_int, cube_size as c_int) };
        UnsafeCover { ptr }
    }

    /// Create from raw pointer
    unsafe fn from_raw(ptr: sys::pset_family) -> Self {
        UnsafeCover { ptr }
    }

    /// Convert to raw pointer
    fn into_raw(self) -> sys::pset_family {
        let ptr = self.ptr;
        std::mem::forget(self);
        ptr
    }
}

impl Drop for UnsafeCover {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::sf_free(self.ptr);
            }
        }
    }
}

impl Clone for UnsafeCover {
    fn clone(&self) -> Self {
        let ptr = unsafe { sys::sf_save(self.ptr) };
        UnsafeCover { ptr }
    }
}

impl UnsafeCover {
    /// Build cover from cube data (INTERNAL: only in worker processes)
    fn build_from_cubes(
        cubes: Vec<(Vec<u8>, Vec<u8>)>,
        _num_inputs: usize,
        _num_outputs: usize,
    ) -> Self {
        // This assumes UnsafeEspresso has already initialized the cube structure
        let cube_size = unsafe { sys::cube.size as usize };

        // Create empty cover with capacity
        let mut cover = UnsafeCover::new(cubes.len(), cube_size);

        // Add each cube to the cover
        for (inputs, outputs) in cubes {
            unsafe {
                let cf = *sys::cube.temp.add(0);
                sys::set_clear(cf, cube_size as c_int);

                // Set input values
                for (var, &val) in inputs.iter().enumerate() {
                    match val {
                        0 => {
                            let bit_pos = var * 2;
                            let word = (bit_pos >> 5) + 1;
                            let bit = bit_pos & 31;
                            *cf.add(word) |= 1 << bit;
                        }
                        1 => {
                            let bit_pos = var * 2 + 1;
                            let word = (bit_pos >> 5) + 1;
                            let bit = bit_pos & 31;
                            *cf.add(word) |= 1 << bit;
                        }
                        2 => {
                            // Don't care: set both bits
                            let bit0 = var * 2;
                            let word0 = (bit0 >> 5) + 1;
                            let b0 = bit0 & 31;
                            *cf.add(word0) |= 1 << b0;

                            let bit1 = var * 2 + 1;
                            let word1 = (bit1 >> 5) + 1;
                            let b1 = bit1 & 31;
                            *cf.add(word1) |= 1 << b1;
                        }
                        _ => panic!("Invalid input value"),
                    }
                }

                // Set output values
                let output_var = sys::cube.num_vars - 1;
                let output_first = *sys::cube.first_part.add(output_var as usize) as usize;

                for (i, &val) in outputs.iter().enumerate() {
                    if val == 1 {
                        let bit_pos = output_first + i;
                        let word = (bit_pos >> 5) + 1;
                        let bit = bit_pos & 31;
                        *cf.add(word) |= 1 << bit;
                    }
                }

                cover.ptr = sys::sf_addset(cover.ptr, cf);
            }
        }

        cover
    }
}

impl<const INPUTS: usize, const OUTPUTS: usize, T: CoverTypeMarker>
    CoverBuilder<INPUTS, OUTPUTS, T>
{
    /// Create a new empty cover
    pub fn new() -> Self {
        CoverBuilder {
            cubes: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Add a cube to the cover
    ///
    /// Outputs can use PLA-style notation:
    /// - `Some(true)` or `'1'` → bit set in F cube (ON-set)
    /// - `Some(false)` or `'0'` → bit set in R cube (OFF-set, only if cover type includes R)
    /// - `None` or `'-'` → bit set in D cube (Don't-care, only if cover type includes D)
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input values: `Some(false)` = 0, `Some(true)` = 1, `None` = don't care
    /// * `outputs` - Output values following PLA conventions based on cover type
    pub fn add_cube(
        &mut self,
        inputs: &[Option<bool>; INPUTS],
        outputs: &[Option<bool>; OUTPUTS],
    ) -> &mut Self {
        // Parse outputs following Espresso C convention (cvrin.c lines 176-199)
        // Create separate F, D, R cubes from a single line based on output values
        let mut f_outputs = Vec::with_capacity(OUTPUTS);
        let mut d_outputs = Vec::with_capacity(OUTPUTS);
        let mut r_outputs = Vec::with_capacity(OUTPUTS);
        let mut has_f = false;
        let mut has_d = false;
        let mut has_r = false;

        let pla_type = T::PLA_TYPE;
        for &out in outputs.iter() {
            match out {
                Some(true) if pla_type.has_f() => {
                    // '1' → bit set in F cube
                    f_outputs.push(true);
                    d_outputs.push(false);
                    r_outputs.push(false);
                    has_f = true;
                }
                Some(false) if pla_type.has_r() => {
                    // '0' → bit set in R cube
                    f_outputs.push(false);
                    d_outputs.push(false);
                    r_outputs.push(true);
                    has_r = true;
                }
                None if pla_type.has_d() => {
                    // None/'-' → bit set in D cube
                    f_outputs.push(false);
                    d_outputs.push(true);
                    r_outputs.push(false);
                    has_d = true;
                }
                _ => {
                    // Type not supported or unset bit
                    f_outputs.push(false);
                    d_outputs.push(false);
                    r_outputs.push(false);
                }
            }
        }

        // Add cubes only if they have meaningful outputs
        let inputs_vec = inputs.to_vec();
        if has_f {
            self.cubes
                .push(Cube::new(inputs_vec.clone(), f_outputs, CubeType::F));
        }
        if has_d {
            self.cubes
                .push(Cube::new(inputs_vec.clone(), d_outputs, CubeType::D));
        }
        if has_r {
            self.cubes
                .push(Cube::new(inputs_vec, r_outputs, CubeType::R));
        }

        self
    }

    /// Get the number of cubes
    pub fn num_cubes(&self) -> usize {
        self.cubes.len()
    }

    /// Get a reference to the cubes
    ///
    /// Returns the cubes as references to internal Cube structures.
    pub fn cubes(&self) -> &[Cube] {
        &self.cubes
    }

    /// Iterate over cubes
    ///
    /// Returns an iterator over `(&[Option<bool>], Vec<Option<bool>>)` tuples.
    pub fn iter_cubes(&self) -> impl Iterator<Item = (&[Option<bool>], Vec<Option<bool>>)> + '_ {
        self.cubes.iter().map(|cube| {
            let outputs: Vec<Option<bool>> = cube.outputs.iter().map(|&b| Some(b)).collect();
            (cube.inputs.as_ref(), outputs)
        })
    }
}

// Implement Minimizable for CoverBuilder (Cover trait is auto-implemented via blanket impl)
impl<const INPUTS: usize, const OUTPUTS: usize, T: CoverTypeMarker> Minimizable
    for CoverBuilder<INPUTS, OUTPUTS, T>
{
    fn num_inputs(&self) -> usize {
        INPUTS
    }

    fn num_outputs(&self) -> usize {
        OUTPUTS
    }

    fn cover_type(&self) -> PLAType {
        T::PLA_TYPE
    }

    fn internal_cubes_iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cube> + 'a> {
        Box::new(self.cubes.iter())
    }

    fn set_cubes(&mut self, cubes: Vec<Cube>) {
        // Filter cubes based on the cover type
        let pla_type = T::PLA_TYPE;
        self.cubes = cubes
            .into_iter()
            .filter(|cube| match cube.cube_type {
                CubeType::F => pla_type.has_f(),
                CubeType::D => pla_type.has_d(),
                CubeType::R => pla_type.has_r(),
            })
            .collect();
    }
}

impl<const INPUTS: usize, const OUTPUTS: usize, T: CoverTypeMarker> Default
    for CoverBuilder<INPUTS, OUTPUTS, T>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const INPUTS: usize, const OUTPUTS: usize, T: CoverTypeMarker> fmt::Debug
    for CoverBuilder<INPUTS, OUTPUTS, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoverBuilder")
            .field("inputs", &INPUTS)
            .field("outputs", &OUTPUTS)
            .field("cover_type", &T::PLA_TYPE)
            .field("num_cubes", &self.num_cubes())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cover_creation() {
        let cover = CoverBuilder::<2, 1>::new();
        // Just verify the cover was created successfully
        assert_eq!(cover.num_cubes(), 0);
    }

    #[test]
    fn test_cover_with_cubes() {
        let mut cover = CoverBuilder::<3, 1>::new();
        cover.add_cube(&[Some(true), Some(false), None], &[Some(true)]);
        assert_eq!(cover.num_cubes(), 1);
    }
}
