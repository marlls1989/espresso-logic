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
//!
//! ## Thread Safety and Concurrency
//!
//! **IMPORTANT**: This library is **NOT thread-safe**. The underlying C library uses
//! extensive global state (cube structure, configuration variables), which means:
//!
//! - Only one `Espresso` or `PLA` operation should be active at a time
//! - Concurrent operations from multiple threads **will cause undefined behavior**
//! - Tests must run sequentially (use `cargo test -- --test-threads=1`)
//!
//! ### Multi-threaded Applications
//!
//! If you need to use Espresso in a multi-threaded application, you must:
//!
//! ```no_run
//! use espresso_logic::Espresso;
//! use std::sync::Mutex;
//! use std::sync::Arc;
//!
//! // Wrap all Espresso operations in a mutex
//! let espresso_lock = Arc::new(Mutex::new(()));
//!
//! // In each thread:
//! let _guard = espresso_lock.lock().unwrap();
//! let mut esp = Espresso::new(2, 1);
//! // ... perform operations ...
//! // Lock is released when _guard goes out of scope
//! ```
//!
//! For applications requiring high concurrency, consider process-based isolation
//! where the C library runs in a separate process with IPC for communication.

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
///
/// # Thread Safety
///
/// **This struct is NOT thread-safe**. The underlying C library uses global state.
/// Only one `Espresso` instance should be active at a time. In multi-threaded applications,
/// wrap all Espresso operations in a `Mutex`.
///
/// # Examples
///
/// ```no_run
/// use espresso_logic::{Espresso, CoverBuilder};
///
/// let mut esp = Espresso::new(2, 1);
/// let mut builder = CoverBuilder::new(2, 1);
/// builder.add_cube(&[0, 1], &[1]);
/// let minimized = esp.minimize(builder.build(), None, None);
/// ```
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
            // Always tear down existing cube state to avoid interference
            if !sys::cube.fullset.is_null() {
                sys::setdown_cube();
                // Note: setdown_cube() does NOT free part_size, so we need to do it
                if !sys::cube.part_size.is_null() {
                    libc::free(sys::cube.part_size as *mut libc::c_void);
                    sys::cube.part_size = ptr::null_mut();
                }
            }

            // Initialize the cube structure before calling cube_setup()
            // This mimics what parse_pla() does when reading .i and .o directives
            sys::cube.num_binary_vars = num_inputs as c_int;
            sys::cube.num_vars = (num_inputs + 1) as c_int;

            // Allocate part_size array
            let part_size_ptr =
                libc::malloc((sys::cube.num_vars as usize) * std::mem::size_of::<c_int>())
                    as *mut c_int;
            if part_size_ptr.is_null() {
                panic!("Failed to allocate part_size array");
            }
            sys::cube.part_size = part_size_ptr;

            // Set the output size (cube_setup will set binary var sizes to 2)
            *sys::cube.part_size.add(num_inputs) = num_outputs as c_int;

            // Now it's safe to call cube_setup()
            sys::cube_setup();

            // Apply default configuration (like the CLI does)
            let config = EspressoConfig::default();
            config.apply();
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
        // espresso() makes its own copies (via sf_save), so we need to keep our covers alive
        // and clone them to pass in
        let f_ptr = f.clone().into_raw();

        // D and R cannot be NULL - they must be empty covers if not provided
        // (see cvrin.c line 454-455: PLA->D = new_cover(10); PLA->R = new_cover(10);)
        let d_ptr = d
            .as_ref()
            .map(|c| c.clone().into_raw())
            .unwrap_or_else(|| unsafe { sys::sf_new(0, sys::cube.size as c_int) });

        // CRITICAL: If R (OFF-set) is not provided, compute it as complement(F, D)
        // This is what read_pla does at cvrin.c line 558:
        //   PLA->R = complement(cube2list(PLA->F, PLA->D));
        let r_ptr = r
            .as_ref()
            .map(|c| c.clone().into_raw())
            .unwrap_or_else(|| unsafe {
                // Compute R = complement(F, D)
                let cube_list = sys::cube2list(f_ptr, d_ptr);
                sys::complement(cube_list)
            });

        let result = unsafe { sys::espresso(f_ptr, d_ptr, r_ptr) };

        unsafe { Cover::from_raw(result) }
    }

    /// Perform exact minimization (slower but guarantees minimal result)
    pub fn minimize_exact(&mut self, f: Cover, d: Option<Cover>, r: Option<Cover>) -> Cover {
        // Copy covers before passing to minimize_exact (like PLA::minimize does)
        let f_copy = unsafe { sys::sf_save(f.into_raw()) };

        // D and R cannot be NULL - they must be empty covers if not provided
        let d_copy = d
            .map(|c| unsafe { sys::sf_save(c.into_raw()) })
            .unwrap_or_else(|| unsafe { sys::sf_new(0, sys::cube.size as c_int) });
        let r_copy = r
            .map(|c| unsafe { sys::sf_save(c.into_raw()) })
            .unwrap_or_else(|| unsafe { sys::sf_new(0, sys::cube.size as c_int) });

        let result = unsafe { sys::minimize_exact(f_copy, d_copy, r_copy, 1) };

        unsafe { Cover::from_raw(result) }
    }
}

impl Drop for Espresso {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                sys::setdown_cube();
                // setdown_cube() does not free part_size, so we must do it
                if !sys::cube.part_size.is_null() {
                    libc::free(sys::cube.part_size as *mut libc::c_void);
                    sys::cube.part_size = ptr::null_mut();
                }
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

    /// Debug: dump cube contents as hex (for debugging/testing)
    pub fn debug_dump(&self) {
        unsafe {
            println!(
                "Cover: count={}, size={}, wsize={}",
                (*self.ptr).count,
                (*self.ptr).sf_size,
                (*self.ptr).wsize
            );
            let data = (*self.ptr).data;
            let wsize = (*self.ptr).wsize as usize;
            let count = (*self.ptr).count as usize;
            for i in 0..count {
                let cube_ptr = data.add(i * wsize);
                print!("  Cube {}: ", i);
                for w in 0..wsize {
                    print!("{:08x} ", *cube_ptr.add(w));
                }
                println!();
            }
        }
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
///
/// # Important
///
/// `CoverBuilder` requires the global cube structure to be initialized via
/// [`Espresso::new`] before calling [`build`](CoverBuilder::build).
///
/// # Examples
///
/// ```no_run
/// use espresso_logic::{Espresso, CoverBuilder};
///
/// // MUST call Espresso::new first to initialize global state
/// let _esp = Espresso::new(2, 1);
///
/// let mut builder = CoverBuilder::new(2, 1);
/// builder.add_cube(&[0, 1], &[1]); // 01 -> 1
/// builder.add_cube(&[1, 0], &[1]); // 10 -> 1
/// let cover = builder.build();
/// ```
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
        // Get the cube size from the global cube structure
        // (which was initialized by Espresso::new())
        let cube_size = unsafe { sys::cube.size as usize };

        // Create empty cover with capacity
        let mut cover = Cover::new(self.cubes.len(), cube_size);

        // Add each cube to the cover (following read_cube pattern from cvrin.c)
        for (inputs, outputs) in self.cubes {
            unsafe {
                // Use cube.temp[0] as temporary working cube (like read_cube does)
                let cf = *sys::cube.temp.add(0);

                // Clear the cube (like read_cube does: set_clear(cf, cube.size))
                sys::set_clear(cf, cube_size as c_int);

                // Set input values for binary variables (following read_cube from cvrin.c)
                // In read_cube: case '-': set_insert(cf, var*2+1); case '0': set_insert(cf, var*2);
                //               case '1': set_insert(cf, var*2+1);
                for (var, &val) in inputs.iter().enumerate() {
                    match val {
                        0 => {
                            // PLA '0': set_insert(cf, var*2)
                            let bit_pos = var * 2;
                            let word = (bit_pos >> 5) + 1;
                            let bit = bit_pos & 31;
                            *cf.add(word) |= 1 << bit;
                        }
                        1 => {
                            // PLA '1': set_insert(cf, var*2+1)
                            let bit_pos = var * 2 + 1;
                            let word = (bit_pos >> 5) + 1;
                            let bit = bit_pos & 31;
                            *cf.add(word) |= 1 << bit;
                        }
                        2 => {
                            // PLA '-' (don't care): set both bits
                            // set_insert(cf, var*2+1) and set_insert(cf, var*2)
                            let bit0 = var * 2;
                            let word0 = (bit0 >> 5) + 1;
                            let b0 = bit0 & 31;
                            *cf.add(word0) |= 1 << b0;

                            let bit1 = var * 2 + 1;
                            let word1 = (bit1 >> 5) + 1;
                            let b1 = bit1 & 31;
                            *cf.add(word1) |= 1 << b1;
                        }
                        _ => panic!("Invalid input value: {} (must be 0, 1, or 2)", val),
                    }
                }

                // Set output values (last variable, following read_cube pattern)
                // In read_cube: case '1': set_insert(cf, i) where i is bit in output range
                let output_var = sys::cube.num_vars - 1;
                let output_first = *sys::cube.first_part.add(output_var as usize) as usize;

                for (i, &val) in outputs.iter().enumerate() {
                    if val == 1 {
                        // case '1': set_insert(cf, i)
                        let bit_pos = output_first + i;
                        let word = (bit_pos >> 5) + 1;
                        let bit = bit_pos & 31;
                        *cf.add(word) |= 1 << bit;
                    }
                    // case '0': do nothing
                }

                // Add cube to cover (like read_cube: PLA->F = sf_addset(PLA->F, cf))
                cover.ptr = sys::sf_addset(cover.ptr, cf);
            }
        }

        // NOTE: Do NOT call sf_active() here - cubes should not have ACTIVE flag set
        // before being passed to espresso(). The ACTIVE flag is managed internally by espresso.

        cover
    }
}

/// Represents a PLA (Programmable Logic Array) structure
pub struct PLA {
    pub(crate) ptr: sys::pPLA,
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

        // CRITICAL: read_pla will IGNORE .i and .o directives if cube is already initialized!
        // (see cvrin.c lines 231 and 245: "if (cube.fullset != NULL) { fprintf(stderr, "extra .i ignored"); ... }")
        // We must tear down existing cube state to allow read_pla to parse dimensions correctly.
        unsafe {
            if !sys::cube.fullset.is_null() {
                sys::setdown_cube();
                if !sys::cube.part_size.is_null() {
                    libc::free(sys::cube.part_size as *mut libc::c_void);
                    sys::cube.part_size = ptr::null_mut();
                }
            }
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

            // D and R cannot be NULL - they must be empty covers if not provided
            // (see cvrin.c line 454-455 and espresso.c line 49 which calls sf_save(D1))
            let d_copy = if !d.is_null() {
                sys::sf_save(d)
            } else {
                sys::sf_new(0, sys::cube.size as c_int)
            };
            let r_copy = if !r.is_null() {
                sys::sf_save(r)
            } else {
                sys::sf_new(0, sys::cube.size as c_int)
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

    /// Debug: dump F cover contents
    pub fn debug_dump_f(&self) {
        unsafe {
            let f = (*self.ptr).F;
            if f.is_null() {
                println!("F is null");
                return;
            }

            println!(
                "F Cover: count={}, size={}, wsize={}",
                (*f).count,
                (*f).sf_size,
                (*f).wsize
            );
            let data = (*f).data;
            let wsize = (*f).wsize as usize;
            let count = (*f).count as usize;
            for i in 0..count {
                let cube_ptr = data.add(i * wsize);
                print!("  Cube {}: ", i);
                for w in 0..wsize {
                    print!("{:08x} ", *cube_ptr.add(w));
                }
                println!();
            }
        }
    }

    /// Get F cover as a Cover (for testing)
    pub fn get_f(&self) -> Cover {
        unsafe {
            let f = (*self.ptr).F;
            // Make a copy so we don't invalidate the PLA
            Cover::from_raw(sys::sf_save(f))
        }
    }

    /// Debug: check D and R status
    pub fn debug_check_d_r(&self) {
        unsafe {
            let d = (*self.ptr).D;
            let r = (*self.ptr).R;

            println!("D: {:?} (null: {})", d, d.is_null());
            if !d.is_null() {
                println!("  D count: {}, size: {}", (*d).count, (*d).sf_size);
            }

            println!("R: {:?} (null: {})", r, r.is_null());
            if !r.is_null() {
                println!("  R count: {}, size: {}", (*r).count, (*r).sf_size);
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
