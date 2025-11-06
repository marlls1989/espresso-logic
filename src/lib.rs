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

// Public modules
pub mod expression;
pub mod sys;

// Private modules
mod cover;
mod pla;
mod worker;

// Internal unsafe bindings (not exposed)
#[path = "unsafe.rs"]
mod r#unsafe;

// Re-export high-level public API
pub use cover::{Cover, CoverBuilder, CoverTypeMarker, FDRType, FDType, FRType, FType, PLAType};
pub use expression::{BoolExpr, ExprCover};
pub use pla::PLACover;

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

/// Worker mode detection - steals execution before main() if running as worker
#[ctor::ctor]
fn check_worker_mode() {
    if std::env::args().any(|arg| arg == "__ESPRESSO_WORKER__") {
        // We're running as a worker process - handle requests and exit
        worker::run_worker_loop();
        std::process::exit(0);
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
