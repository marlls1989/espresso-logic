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
//! ```no_run
//! use espresso_logic::{Espresso, CoverBuilder};
//!
//! // Create an Espresso instance
//! let mut esp = Espresso::new(2, 1); // 2 inputs, 1 output
//!
//! // Build the ON-set (truth table)
//! let mut builder = CoverBuilder::new(2, 1);
//! builder.add_cube(&[0, 1], &[1]); // When input 0 is 0 and input 1 is 1, output is 1
//! builder.add_cube(&[1, 0], &[1]); // When input 0 is 1 and input 1 is 0, output is 1
//! let f = builder.build();
//!
//! // Minimize
//! let minimized = esp.minimize(f, None, None);
//!
//! // Use the result
//! println!("Minimized cover: {:?}", minimized);
//! ```
//!
//! ## PLA File Format
//!
//! Espresso can also read and write PLA files, a standard format for representing
//! Boolean functions:
//!
//! ```no_run
//! use espresso_logic::PLA;
//!
//! // Read from file
//! let pla = PLA::from_file("input.pla").expect("Failed to read PLA file");
//!
//! // Minimize
//! let minimized = pla.minimize();
//!
//! // Write to file
//! minimized.to_file("output.pla", espresso_logic::PLAType::F)
//!     .expect("Failed to write PLA file");
//! ```

pub mod sys;

// Re-export commonly used constants for CLI
pub use sys::{ESSEN, EXPAND, GASP, IRRED, MINCOV, REDUCE, SHARP, SPARSE};

use std::ffi::CString;
use std::fmt;
use std::io;
use std::os::raw::c_int;
use std::path::Path;
use std::ptr;

/// Represents the type of PLA output format
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

    /// Apply this configuration to the global Espresso state
    pub fn apply(&self) {
        unsafe {
            sys::debug = if self.debug {
                sys::EXPAND
                    | sys::ESSEN
                    | sys::IRRED
                    | sys::REDUCE
                    | sys::SPARSE
                    | sys::GASP
                    | sys::SHARP
                    | sys::MINCOV
            } else {
                0
            };
            sys::verbose_debug = if self.verbose_debug { 1 } else { 0 };
            sys::trace = if self.trace { 1 } else { 0 };
            sys::summary = if self.summary { 1 } else { 0 };
            sys::remove_essential = if self.remove_essential { 1 } else { 0 };
            sys::force_irredundant = if self.force_irredundant { 1 } else { 0 };
            sys::unwrap_onset = if self.unwrap_onset { 1 } else { 0 };
            sys::single_expand = if self.single_expand { 1 } else { 0 };
            sys::use_super_gasp = if self.use_super_gasp { 1 } else { 0 };
            sys::use_random_order = if self.use_random_order { 1 } else { 0 };
            sys::print_solution = 1;
            sys::pos = 0;
            sys::recompute_onset = 0;
            sys::kiss = 0;
            sys::echo_comments = 1;
            sys::echo_unknown_commands = 1;
            sys::skip_make_sparse = 0;
        }
    }
}

/// A safe wrapper around the Espresso logic minimizer
pub struct Espresso {
    initialized: bool,
    #[allow(dead_code)]
    num_inputs: usize,
    #[allow(dead_code)]
    num_outputs: usize,
}

impl Espresso {
    /// Create a new Espresso instance for functions with the given number of inputs and outputs
    pub fn new(num_inputs: usize, num_outputs: usize) -> Self {
        unsafe {
            sys::cube_setup();
        }

        Espresso {
            initialized: true,
            num_inputs,
            num_outputs,
        }
    }

    /// Minimize a Boolean function
    ///
    /// # Arguments
    ///
    /// * `f` - The ON-set (minterms where the function is true)
    /// * `d` - The don't-care set (optional, minterms where the function value doesn't matter)
    /// * `r` - The OFF-set (optional, minterms where the function is false)
    ///
    /// # Returns
    ///
    /// A minimized cover representing the Boolean function
    pub fn minimize(&mut self, f: Cover, d: Option<Cover>, r: Option<Cover>) -> Cover {
        let f_ptr = f.into_raw();
        let d_ptr = d.map(|c| c.into_raw()).unwrap_or(ptr::null_mut());
        let r_ptr = r.map(|c| c.into_raw()).unwrap_or(ptr::null_mut());

        let result = unsafe { sys::espresso(f_ptr, d_ptr, r_ptr) };

        unsafe { Cover::from_raw(result) }
    }

    /// Perform exact minimization (slower but guarantees minimal result)
    pub fn minimize_exact(&mut self, f: Cover, d: Option<Cover>, r: Option<Cover>) -> Cover {
        let f_ptr = f.into_raw();
        let d_ptr = d.map(|c| c.into_raw()).unwrap_or(ptr::null_mut());
        let r_ptr = r.map(|c| c.into_raw()).unwrap_or(ptr::null_mut());

        let result = unsafe { sys::minimize_exact(f_ptr, d_ptr, r_ptr, 1) };

        unsafe { Cover::from_raw(result) }
    }
}

impl Drop for Espresso {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                sys::setdown_cube();
            }
        }
    }
}

/// Represents a cover (set of cubes) in the Boolean function
pub struct Cover {
    ptr: sys::pset_family,
}

impl Cover {
    /// Create a new empty cover with the given capacity and cube size
    pub fn new(capacity: usize, cube_size: usize) -> Self {
        let ptr = unsafe { sys::sf_new(capacity as c_int, cube_size as c_int) };

        Cover { ptr }
    }

    /// Create from a raw pointer (takes ownership)
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `ptr` is a valid pointer to a set_family allocated by Espresso
    /// - `ptr` is not used after this call (ownership is transferred)
    /// - `ptr` will be freed when the Cover is dropped
    pub unsafe fn from_raw(ptr: sys::pset_family) -> Self {
        Cover { ptr }
    }

    /// Convert to a raw pointer (releases ownership)
    pub fn into_raw(self) -> sys::pset_family {
        let ptr = self.ptr;
        std::mem::forget(self);
        ptr
    }

    /// Get the number of cubes in this cover
    pub fn count(&self) -> usize {
        unsafe { (*self.ptr).count as usize }
    }

    /// Get the size of each cube
    pub fn cube_size(&self) -> usize {
        unsafe { (*self.ptr).sf_size as usize }
    }
}

impl Drop for Cover {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::sf_free(self.ptr);
            }
        }
    }
}

impl Clone for Cover {
    fn clone(&self) -> Self {
        let ptr = unsafe { sys::sf_save(self.ptr) };
        Cover { ptr }
    }
}

impl fmt::Debug for Cover {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cover")
            .field("count", &self.count())
            .field("cube_size", &self.cube_size())
            .finish()
    }
}

/// Builder for creating covers programmatically
pub struct CoverBuilder {
    num_inputs: usize,
    num_outputs: usize,
    cubes: Vec<(Vec<u8>, Vec<u8>)>,
}

impl CoverBuilder {
    /// Create a new cover builder
    pub fn new(num_inputs: usize, num_outputs: usize) -> Self {
        CoverBuilder {
            num_inputs,
            num_outputs,
            cubes: Vec::new(),
        }
    }

    /// Add a cube to the cover
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input values (0 = must be 0, 1 = must be 1, 2 = don't care)
    /// * `outputs` - Output values (0 or 1)
    pub fn add_cube(&mut self, inputs: &[u8], outputs: &[u8]) -> &mut Self {
        assert_eq!(inputs.len(), self.num_inputs, "Input length mismatch");
        assert_eq!(outputs.len(), self.num_outputs, "Output length mismatch");

        self.cubes.push((inputs.to_vec(), outputs.to_vec()));
        self
    }

    /// Build the cover
    pub fn build(self) -> Cover {
        // Calculate cube size: 2 bits per input + 1 bit per output
        let cube_size = self.num_inputs * 2 + self.num_outputs;

        // Create empty cover with capacity
        let mut cover = Cover::new(self.cubes.len(), cube_size);

        // Add each cube to the cover
        for (inputs, outputs) in self.cubes {
            unsafe {
                // Calculate set size needed (from SET_SIZE macro in espresso.h)
                let set_size = if cube_size <= 32 {
                    2
                } else {
                    ((cube_size - 1) >> 5) + 2
                };

                // Allocate and clear a new cube (set)
                let cube_words = set_size;
                let cube = libc::malloc(cube_words * std::mem::size_of::<u32>()) as *mut u32;
                if cube.is_null() {
                    panic!("Failed to allocate cube");
                }

                // Clear the cube (set_clear implementation)
                let loop_init = if cube_size <= 32 {
                    1
                } else {
                    (cube_size - 1) >> 5
                };
                *cube = loop_init as u32;
                for i in 1..=loop_init {
                    *cube.add(i) = 0;
                }

                // Set input values (2 bits per input: 00=DC, 01=0, 10=1, 11=DC)
                for (i, &val) in inputs.iter().enumerate() {
                    let bit_pos = i * 2;
                    match val {
                        0 => {
                            // Variable must be 0 (bit pattern: 01)
                            let word = (bit_pos >> 5) + 1;
                            let bit = bit_pos & 31;
                            *cube.add(word) |= 1 << bit;
                        }
                        1 => {
                            // Variable must be 1 (bit pattern: 10)
                            let word = ((bit_pos + 1) >> 5) + 1;
                            let bit = (bit_pos + 1) & 31;
                            *cube.add(word) |= 1 << bit;
                        }
                        2 => {
                            // Don't care (bit pattern: 11)
                            let word0 = (bit_pos >> 5) + 1;
                            let bit0 = bit_pos & 31;
                            let word1 = ((bit_pos + 1) >> 5) + 1;
                            let bit1 = (bit_pos + 1) & 31;
                            *cube.add(word0) |= 1 << bit0;
                            *cube.add(word1) |= 1 << bit1;
                        }
                        _ => panic!("Invalid input value: {} (must be 0, 1, or 2)", val),
                    }
                }

                // Set output values (1 bit per output)
                let output_start = self.num_inputs * 2;
                for (i, &val) in outputs.iter().enumerate() {
                    if val == 1 {
                        let bit_pos = output_start + i;
                        let word = (bit_pos >> 5) + 1;
                        let bit = bit_pos & 31;
                        *cube.add(word) |= 1 << bit;
                    }
                }

                // Add cube to the cover
                cover.ptr = sys::sf_addset(cover.ptr, cube);

                // Free the cube (sf_addset makes a copy)
                libc::free(cube as *mut libc::c_void);
            }
        }

        cover
    }
}

/// Represents a PLA (Programmable Logic Array) structure
pub struct PLA {
    ptr: sys::pPLA,
}

impl PLA {
    /// Read a PLA from a file
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path_str = path
            .as_ref()
            .to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid path"))?;

        let c_path = CString::new(path_str)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Path contains null byte"))?;

        let file_mode = CString::new("r").unwrap();
        let file = unsafe { libc::fopen(c_path.as_ptr(), file_mode.as_ptr()) };

        if file.is_null() {
            return Err(io::Error::last_os_error());
        }

        let mut pla_ptr: sys::pPLA = ptr::null_mut();

        let result = unsafe {
            // Cast libc::FILE to the FILE type expected by espresso
            sys::read_pla(file as *mut _, 1, 1, sys::FD_type as c_int, &mut pla_ptr)
        };

        unsafe { libc::fclose(file) };

        if result == libc::EOF {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Failed to read PLA",
            ));
        }

        Ok(PLA { ptr: pla_ptr })
    }

    /// Read a PLA from a string
    pub fn from_string(s: &str) -> io::Result<Self> {
        // Write string to temporary file and read from it
        // This is a simplified implementation
        // A better implementation would use fmemopen on supported platforms
        use std::io::Write;
        let mut temp = tempfile::NamedTempFile::new()?;
        temp.write_all(s.as_bytes())?;
        temp.flush()?;

        Self::from_file(temp.path())
    }

    /// Minimize this PLA using Espresso
    ///
    /// This is a safe wrapper that handles initialization internally
    pub fn minimize(&self) -> Self {
        unsafe {
            // Note: cube structure should already be initialized from PLA::from_file
            let f = (*self.ptr).F;
            let d = (*self.ptr).D;
            let r = (*self.ptr).R;

            let f_copy = sys::sf_save(f);
            let d_copy = if !d.is_null() {
                sys::sf_save(d)
            } else {
                ptr::null_mut()
            };
            let r_copy = if !r.is_null() {
                sys::sf_save(r)
            } else {
                ptr::null_mut()
            };

            let minimized_f = sys::espresso(f_copy, d_copy, r_copy);

            let new_pla = sys::new_PLA();
            (*new_pla).F = minimized_f;
            (*new_pla).D = d_copy;
            (*new_pla).R = r_copy;

            PLA { ptr: new_pla }
        }
    }

    /// Get statistics about this PLA
    pub fn stats(&self) -> PLAStats {
        unsafe {
            let f = (*self.ptr).F;
            let d = (*self.ptr).D;
            let r = (*self.ptr).R;

            PLAStats {
                num_cubes_f: if !f.is_null() { (*f).count as usize } else { 0 },
                num_cubes_d: if !d.is_null() { (*d).count as usize } else { 0 },
                num_cubes_r: if !r.is_null() { (*r).count as usize } else { 0 },
            }
        }
    }

    /// Write this PLA to a file
    pub fn to_file<P: AsRef<Path>>(&self, path: P, pla_type: PLAType) -> io::Result<()> {
        let path_str = path
            .as_ref()
            .to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid path"))?;

        let c_path = CString::new(path_str)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Path contains null byte"))?;

        let file_mode = CString::new("w").unwrap();
        let file = unsafe { libc::fopen(c_path.as_ptr(), file_mode.as_ptr()) };

        if file.is_null() {
            return Err(io::Error::last_os_error());
        }

        unsafe {
            // Cast libc::FILE to the FILE type expected by espresso
            sys::fprint_pla(file as *mut _, self.ptr, pla_type as c_int);
            libc::fclose(file);
        }

        Ok(())
    }

    /// Print a summary of this PLA to stdout
    pub fn print_summary(&self) {
        unsafe {
            sys::PLA_summary(self.ptr);
        }
    }

    /// Get the raw pointer (for advanced use)
    ///
    /// # Safety
    ///
    /// This provides direct access to the underlying C pointer.
    /// Use with caution.
    pub fn as_ptr(&self) -> sys::pPLA {
        self.ptr
    }

    /// Write this PLA to stdout
    pub fn write_to_stdout(&self, pla_type: PLAType) -> io::Result<()> {
        unsafe {
            // Duplicate stdout fd so we can safely close the FILE* without affecting the original stdout
            let dup_fd = libc::dup(1);
            if dup_fd == -1 {
                return Err(io::Error::last_os_error());
            }

            let stdout_ptr = libc::fdopen(dup_fd, c"w".as_ptr());
            if stdout_ptr.is_null() {
                libc::close(dup_fd);
                return Err(io::Error::other("Failed to open stdout"));
            }

            sys::fprint_pla(stdout_ptr as *mut _, self.ptr, pla_type as c_int);
            libc::fflush(stdout_ptr);

            // Close the FILE* (which also closes the duplicated fd)
            // This prevents the FILE structure leak
            libc::fclose(stdout_ptr);

            Ok(())
        }
    }
}

impl Drop for PLA {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::free_PLA(self.ptr);
            }
        }
    }
}

impl fmt::Debug for PLA {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stats = self.stats();
        f.debug_struct("PLA")
            .field("cubes_f", &stats.num_cubes_f)
            .field("cubes_d", &stats.num_cubes_d)
            .field("cubes_r", &stats.num_cubes_r)
            .finish()
    }
}

/// Statistics about a PLA
#[derive(Debug, Clone)]
pub struct PLAStats {
    pub num_cubes_f: usize,
    pub num_cubes_d: usize,
    pub num_cubes_r: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_espresso_creation() {
        let esp = Espresso::new(2, 1);
        assert_eq!(esp.num_inputs, 2);
        assert_eq!(esp.num_outputs, 1);
    }

    #[test]
    fn test_cover_creation() {
        let cover = Cover::new(10, 5);
        // Just verify the cover was created successfully
        let _ = cover.count();
    }
}
