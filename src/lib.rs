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
//! The `expr!` macro provides three convenient styles:
//!
//! ```
//! use espresso_logic::{BoolExpr, expr};
//!
//! # fn main() -> std::io::Result<()> {
//! // Style 1: String literals (most concise - no declarations!)
//! let xor = expr!("a" * "b" + !"a" * !"b");
//! println!("{}", xor);  // Output: a * b + ~a * ~b (minimal parentheses!)
//!
//! // Style 2: Existing BoolExpr variables
//! let a = BoolExpr::variable("a");
//! let b = BoolExpr::variable("b");
//! let c = BoolExpr::variable("c");
//! let redundant = expr!(a * b + a * b * c);
//!
//! // Minimize it (returns a new minimized expression)
//! let minimized = redundant.minimize()?;
//! println!("Minimized: {}", minimized);  // Output: a * b
//!
//! // Check logical equivalence (create new instance for comparison)
//! let redundant2 = expr!(a * b + a * b * c);
//! assert!(redundant2.equivalent_to(&minimized));
//! # Ok(())
//! # }
//! ```
//!
//! Parse expressions from strings:
//!
//! ```
//! use espresso_logic::BoolExpr;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Parse using standard operators: +, *, ~, !
//! let expr = BoolExpr::parse("a * b + ~a * ~b")?;
//!
//! // Minimize
//! let minimized = expr.minimize()?;
//! # Ok(())
//! # }
//! ```
//!
//! #### Using Cover with Expressions
//!
//! For advanced use cases, the `Cover` type provides direct access to the cover
//! representation and supports adding expressions:
//!
//! ```
//! use espresso_logic::{BoolExpr, Cover, CoverType};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let a = BoolExpr::variable("a");
//! let b = BoolExpr::variable("b");
//! let expr = a.and(&b).or(&a.and(&b.not()));
//!
//! // Create cover and add expression
//! let mut cover = Cover::new(CoverType::F);
//! cover.add_expr(expr, "output")?;
//!
//! // Access cover properties
//! println!("Input variables: {:?}", cover.input_labels());
//! println!("Number of cubes: {}", cover.num_cubes());
//!
//! // Minimize the cover
//! cover.minimize()?;
//!
//! // Convert back to expression
//! let minimized = cover.to_expr("output")?;
//! println!("Minimized: {}", minimized);
//! # Ok(())
//! # }
//! ```
//!
//! ### 2. Manual Cube Construction
//!
//! Build covers by manually adding cubes (dimensions grow automatically):
//!
//! ```
//! use espresso_logic::{Cover, CoverType};
//!
//! # fn main() -> std::io::Result<()> {
//! // Create a cover (dimensions grow automatically)
//! let mut cover = Cover::new(CoverType::F);
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
//! Load and minimize PLA files:
//!
//! ```
//! use espresso_logic::{Cover, CoverType, PLAReader, PLAWriter};
//! # use std::io::Write;
//!
//! # fn main() -> std::io::Result<()> {
//! # let mut temp = tempfile::NamedTempFile::new()?;
//! # temp.write_all(b".i 2\n.o 1\n.p 1\n01 1\n.e\n")?;
//! # temp.flush()?;
//! # let input_path = temp.path();
//! // Read from PLA file (PLAReader trait)
//! let mut cover = Cover::from_pla_file(input_path)?;
//!
//! // Minimize
//! cover.minimize()?;
//!
//! # let output_file = tempfile::NamedTempFile::new()?;
//! # let output_path = output_file.path();
//! // Write to PLA file (PLAWriter trait)
//! cover.to_pla_file(output_path, CoverType::F)?;
//!
//! // Or write directly to any Write implementation
//! use std::io::{Write, BufReader};
//! let mut buffer = Vec::new();
//! cover.write_pla(&mut buffer, CoverType::F)?;
//!
//! // Similarly, you can read from any BufRead implementation
//! let reader = BufReader::new(buffer.as_slice());
//! let cover2 = Cover::from_pla_reader(reader)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Cover Types
//!
//! The library supports different cover types for representing Boolean functions:
//!
//! - **F Type** - ON-set only (specifies where output is 1)
//! - **FD Type** - ON-set + Don't-cares (most flexible)
//! - **FR Type** - ON-set + OFF-set (specifies both 1s and 0s)
//! - **FDR Type** - ON-set + Don't-cares + OFF-set (complete specification)
//!
//! ```
//! use espresso_logic::{Cover, CoverType};
//!
//! # fn main() -> std::io::Result<()> {
//! // F type (ON-set only)
//! let mut f_cover = Cover::new(CoverType::F);
//! f_cover.add_cube(&[Some(true), Some(true)], &[Some(true)]);
//!
//! // FD type (ON-set + Don't-cares)
//! let mut fd_cover = Cover::new(CoverType::FD);
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
//! Just use `Cover` directly - each thread executes Espresso independently:
//!
//! ```
//! use espresso_logic::{Cover, CoverType};
//! use std::thread;
//!
//! # fn main() -> std::io::Result<()> {
//! // Spawn threads - no synchronization needed!
//! let handles: Vec<_> = (0..4).map(|_| {
//!     thread::spawn(move || {
//!         let mut cover = Cover::new(CoverType::F);
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
pub mod error;
pub mod espresso;
pub mod expression;
pub mod pla;
pub mod sys;

// Re-export high-level public API
pub use cover::{Cover, CoverType, Cube, CubeType};
pub use error::{
    AddExprError, CoverError, CubeError, ExpressionParseError, InstanceError, MinimizationError,
    PLAError, PLAReadError, PLAWriteError, ParseBoolExprError, ToExprError,
};
pub use expression::BoolExpr;
pub use pla::{PLAReader, PLAWriter};

// Re-export procedural macro
pub use espresso_logic_macros::expr;

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
