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
//! ## Three Ways to Use Espresso
//!
//! ### 1. Boolean Expressions (Recommended for most use cases)
//!
//! Build expressions programmatically and minimize them:
//!
//! ```
//! use espresso_logic::{BoolExpr, expr};
//!
//! # fn main() -> std::io::Result<()> {
//! let a = BoolExpr::variable("a");
//! let b = BoolExpr::variable("b");
//! let c = BoolExpr::variable("c");
//!
//! // Build a redundant expression: a*b + a*b*c
//! let redundant = expr!(a * b + a * b * c);
//!
//! // Minimize it (returns a new minimized expression)
//! let minimized = redundant.minimize()?;
//!
//! println!("Minimized: {}", minimized);  // Output: a*b
//! # Ok(())
//! # }
//! ```
//!
//! Parse expressions from strings:
//!
//! ```
//! use espresso_logic::BoolExpr;
//!
//! # fn main() -> Result<(), String> {
//! // Parse using standard operators: +, *, ~, !
//! let expr = BoolExpr::parse("a * b + ~a * ~b")?;
//!
//! // Minimize
//! let minimized = expr.minimize().map_err(|e| e.to_string())?;
//! # Ok(())
//! # }
//! ```
//!
//! #### Using ExprCover for More Control
//!
//! For advanced use cases, [`ExprCover`] provides direct access to the cover
//! representation and implements the [`Cover`] trait:
//!
//! ```
//! use espresso_logic::{BoolExpr, ExprCover, Cover};
//!
//! # fn main() -> std::io::Result<()> {
//! let a = BoolExpr::variable("a");
//! let b = BoolExpr::variable("b");
//! let expr = a.and(&b).or(&a.and(&b.not()));
//!
//! // Convert to cover representation
//! let mut cover = ExprCover::from_expr(expr);
//!
//! // Access cover properties
//! println!("Variables: {:?}", cover.variables());
//! println!("Number of cubes: {}", cover.num_cubes());
//!
//! // Minimize the cover
//! cover.minimize()?;
//!
//! // Convert back to expression
//! let minimized = cover.to_expr();
//! println!("Minimized: {}", minimized);
//! # Ok(())
//! # }
//! ```
//!
//! ### 2. Cover Builder (Static dimensions with compile-time checking)
//!
//! Build covers with fixed dimensions known at compile time:
//!
//! ```
//! use espresso_logic::{Cover, CoverBuilder};
//!
//! # fn main() -> std::io::Result<()> {
//! // Create a cover for a 2-input, 1-output function
//! let mut cover = CoverBuilder::<2, 1>::new();
//!
//! // Build the ON-set (truth table)
//! cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);  // 01 -> 1
//! cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);  // 10 -> 1
//!
//! // Minimize in-place
//! cover.minimize()?;
//!
//! // Iterate over minimized cubes
//! for (inputs, outputs) in cover.cubes_iter() {
//!     println!("Cube: {:?} -> {:?}", inputs, outputs);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### 3. PLA Files (Dynamic dimensions from files)
//!
//! Load and minimize PLA files with dynamic dimensions:
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
//! // Write to PLA file (uses efficient writer-based implementation)
//! cover.to_pla_file(output_path, PLAType::F)?;
//!
//! // Or write directly to any Write implementation
//! use std::io::{Write, BufReader};
//! let mut buffer = Vec::new();
//! cover.write_pla(&mut buffer, PLAType::F)?;
//!
//! // Similarly, you can read from any BufRead implementation
//! let reader = BufReader::new(buffer.as_slice());
//! let cover2 = PLACover::from_pla_reader(reader)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Cover Types
//!
//! The library supports different cover types for representing Boolean functions:
//!
//! - **F Type** - ON-set only (specifies where output is 1)
//! - **FD Type** - ON-set + Don't-cares (default, most flexible)
//! - **FR Type** - ON-set + OFF-set (specifies both 1s and 0s)
//! - **FDR Type** - ON-set + Don't-cares + OFF-set (complete specification)
//!
//! ```
//! use espresso_logic::{CoverBuilder, FType, FDType, Cover};
//!
//! # fn main() -> std::io::Result<()> {
//! // F type (ON-set only)
//! let mut f_cover = CoverBuilder::<2, 1, FType>::new();
//! f_cover.add_cube(&[Some(true), Some(true)], &[Some(true)]);
//!
//! // FD type (ON-set + Don't-cares) - default
//! let mut fd_cover = CoverBuilder::<2, 1, FDType>::new();  // or just CoverBuilder::<2, 1>::new()
//! fd_cover.add_cube(&[Some(true), Some(true)], &[Some(true)]);  // ON
//! fd_cover.add_cube(&[Some(false), Some(false)], &[None]);      // Don't-care
//! # Ok(())
//! # }
//! ```
//!
//! ## Thread Safety and Concurrency
//!
//! **This library IS thread-safe!** The underlying C library uses **C11 thread-local storage**
//! (`_Thread_local`) for all global state. Each thread gets its own independent copy of all
//! global variables, making concurrent use completely safe without any synchronization.
//!
//! ### Multi-threaded Applications
//!
//! Just use `CoverBuilder` directly - each thread executes Espresso independently:
//!
//! ```
//! use espresso_logic::{Cover, CoverBuilder};
//! use std::thread;
//!
//! # fn main() -> std::io::Result<()> {
//! // Spawn threads - no synchronization needed!
//! let handles: Vec<_> = (0..4).map(|_| {
//!     thread::spawn(move || {
//!         let mut cover = CoverBuilder::<2, 1>::new();
//!         cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
//!         cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
//!         
//!         // Thread-safe - each thread executes with independent global state
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
//! - **Thread-local storage**: All C global variables use `_Thread_local`
//! - **Independent state**: Each thread has its own copy of all globals
//! - **Native safety**: Uses standard C11 thread safety features

// Public modules
pub mod cover;
pub mod espresso;
pub mod expression;
pub mod pla;
pub mod sys;

// Re-export high-level public API
pub use cover::{
    Cover, CoverBuilder, CoverTypeMarker, Cube, CubeType, FDRType, FDType, FRType, FType, PLAType,
};
pub use expression::{BoolExpr, ExprCover};
pub use pla::PLACover;

/// Configuration for the Espresso algorithm
#[derive(Debug, Clone, PartialEq, Eq)]
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
