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
//! ## API Levels
//!
//! This crate provides **two API levels** to suit different needs:
//!
//! ### High-Level API (Recommended)
//!
//! The high-level API provides easy-to-use abstractions with automatic resource management:
//!
//! - **[`BoolExpr`]** - Boolean expressions with parsing, operators, and the `expr!` macro
//! - **[`Cover`]** - Dynamic covers with automatic dimension management
//! - **[`PLAReader`]** and **[`PLAWriter`]** traits - File I/O for PLA format
//!
//! **Benefits:**
//! - ✅ Automatic memory management
//! - ✅ No manual dimension tracking
//! - ✅ Thread-safe by design
//! - ✅ Clean, idiomatic Rust API
//!
//! ### Low-Level API (Advanced)
//!
//! The low-level [`espresso`] module provides direct access to the C library:
//!
//! - **[`espresso::Espresso`]** - Direct Espresso instance management
//! - **[`espresso::EspressoCover`]** - Raw cover with C memory control
//!
//! **When to use:**
//! - **Access to intermediate covers** - Get ON-set (F), don't-care (D), and OFF-set (R) separately
//! - **Custom don't-care/off-sets** - Provide your own D and R covers to `minimize()`
//! - **Maximum performance** - Minimal overhead, direct C calls (~5-10% faster than high-level)
//! - **Explicit instance control** - Manually manage when Espresso instances are created/destroyed
//!
//! **Note:** Algorithm configuration via [`EspressoConfig`] works with **both** APIs -
//! it's not a reason to use the low-level API.
//!
//! **Important constraints:**
//! - ⚠️ **All covers on a thread must use the same dimensions** until dropped
//! - ⚠️ Requires manual dimension management
//! - ⚠️ More complex error handling
//!
//! See the [`espresso`] module documentation for detailed usage and safety guidelines.
//!
//! ## Three Ways to Use the High-Level API
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
//! # fn main() -> std::io::Result<()> {
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
//! # fn main() -> std::io::Result<()> {
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
//!
//! ## Using the Low-Level API (Advanced)
//!
//! For maximum performance and fine-grained control, use the [`espresso`] module directly:
//!
//! ```
//! use espresso_logic::espresso::{Espresso, EspressoCover, CubeType};
//! use espresso_logic::EspressoConfig;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Explicit instance creation with custom config
//! let mut config = EspressoConfig::default();
//! config.single_expand = true;  // Faster mode
//! let _esp = Espresso::new(2, 1, &config);
//!
//! // Create cover with raw cube data
//! let cubes = vec![
//!     (vec![0, 1], vec![1]),  // 01 -> 1
//!     (vec![1, 0], vec![1]),  // 10 -> 1
//! ];
//! let cover = EspressoCover::from_cubes(cubes, 2, 1)?;
//!
//! // Minimize and get all three covers (F, D, R)
//! let (f_result, d_result, r_result) = cover.minimize(None, None);
//!
//! println!("ON-set: {} cubes", f_result.to_cubes(2, 1, CubeType::F).len());
//! println!("Don't-care: {} cubes", d_result.to_cubes(2, 1, CubeType::F).len());
//! println!("OFF-set: {} cubes", r_result.to_cubes(2, 1, CubeType::F).len());
//! # Ok(())
//! # }
//! ```
//!
//! **⚠️ Important Constraint:** All covers on a thread must use the same dimensions until dropped:
//!
//! ```
//! use espresso_logic::espresso::EspressoCover;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Works: same dimensions (2 inputs, 1 output)
//! let cover1 = EspressoCover::from_cubes(vec![(vec![0, 1], vec![1])], 2, 1)?;
//! let cover2 = EspressoCover::from_cubes(vec![(vec![1, 0], vec![1])], 2, 1)?;
//!
//! // Must drop before using different dimensions
//! drop(cover1);
//! drop(cover2);
//!
//! // Now 3 inputs works
//! let cover3 = EspressoCover::from_cubes(vec![(vec![0, 1, 0], vec![1])], 3, 1)?;
//! # Ok(())
//! # }
//! ```
//!
//! See the [`espresso`] module documentation for detailed safety guidelines and usage patterns.
//!
//! # 📚 Comprehensive Guides
//!
//! See the [`doc`] module for embedded guides:
//!
//! - [`doc::examples`] - Complete usage examples for all features
//! - [`doc::boolean_expressions`] - Boolean expression API deep dive
//! - [`doc::pla_format`] - PLA file format specification
//! - [`doc::cli`] - Command-line tool documentation

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

/// Comprehensive documentation guides
///
/// This module contains embedded guides from the `docs/` directory,
/// making all comprehensive documentation available on docs.rs.
///
/// # Available Guides
///
/// - [`examples`](doc::examples) - Complete usage examples for all features
/// - [`boolean_expressions`](doc::boolean_expressions) - Boolean expression API deep dive
/// - [`pla_format`](doc::pla_format) - PLA file format specification
/// - [`cli`](doc::cli) - Command-line tool documentation
pub mod doc {
    #[doc = include_str!("../docs/EXAMPLES.md")]
    #[cfg(doc)]
    pub mod examples {}

    #[doc = include_str!("../docs/BOOLEAN_EXPRESSIONS.md")]
    #[cfg(doc)]
    pub mod boolean_expressions {}

    #[doc = include_str!("../docs/PLA_FORMAT.md")]
    #[cfg(doc)]
    pub mod pla_format {}

    #[doc = include_str!("../docs/CLI.md")]
    #[cfg(doc)]
    pub mod cli {}
}

/// Configuration for the Espresso algorithm
///
/// Controls the behavior of the Espresso heuristic logic minimizer. This configuration
/// can be used with **both the high-level and low-level APIs** to tune the minimization
/// process for your specific needs.
///
/// # When to Use
///
/// Most users should use the **default configuration** which provides a good balance
/// between speed and result quality. Consider customizing when you need:
///
/// - **Maximum speed** with acceptable quality loss (`single_expand = true`)
/// - **Debugging** algorithm behavior (`debug = true`, `trace = true`)
/// - **Performance metrics** (`summary = true`)
/// - **Non-deterministic exploration** (`use_random_order = true`)
///
/// # Works with Both APIs
///
/// ## High-Level API (`Cover`)
///
/// Use with [`Cover::minimize_with_config()`](crate::Cover::minimize_with_config):
///
/// ```
/// use espresso_logic::{Cover, CoverType, EspressoConfig};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut cover = Cover::new(CoverType::F);
/// cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
///
/// // Use custom configuration
/// let mut config = EspressoConfig::default();
/// config.single_expand = true;  // Fast mode
/// config.summary = true;        // Show statistics
///
/// cover.minimize_with_config(&config)?;
/// # Ok(())
/// # }
/// ```
///
/// ## Low-Level API (`espresso` module)
///
/// Use when creating an [`Espresso`](crate::espresso::Espresso) instance:
///
/// ```
/// use espresso_logic::espresso::{Espresso, EspressoCover};
/// use espresso_logic::EspressoConfig;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create instance with custom config
/// let mut config = EspressoConfig::default();
/// config.single_expand = true;
/// let _esp = Espresso::new(2, 1, &config);
///
/// // All operations use this configuration
/// let cubes = vec![(vec![0, 1], vec![1])];
/// let cover = EspressoCover::from_cubes(cubes, 2, 1)?;
/// let (result, _, _) = cover.minimize(None, None);
/// # Ok(())
/// # }
/// ```
///
/// # Common Configuration Patterns
///
/// ## Fast Mode (Recommended for Large Problems)
///
/// ```
/// use espresso_logic::EspressoConfig;
///
/// let mut config = EspressoConfig::default();
/// config.single_expand = true;  // Skip iterative expand phase
/// // Results: ~30-50% faster, typically 5-10% larger covers
/// ```
///
/// ## Quality Mode (Default)
///
/// ```
/// use espresso_logic::EspressoConfig;
///
/// let config = EspressoConfig::default();
/// // remove_essential = true (remove obvious terms first)
/// // force_irredundant = true (ensure no redundant cubes)
/// // unwrap_onset = true (preprocessing optimization)
/// // single_expand = false (iterate for best results)
/// ```
///
/// ## Debug Mode
///
/// ```
/// use espresso_logic::EspressoConfig;
///
/// let mut config = EspressoConfig::default();
/// config.debug = true;      // Print detailed algorithm steps
/// config.trace = true;      // Show phase transitions
/// config.summary = true;    // Display final statistics
/// ```
///
/// ## Experimental Mode
///
/// ```
/// use espresso_logic::EspressoConfig;
///
/// let mut config = EspressoConfig::default();
/// config.use_super_gasp = true;    // Enhanced heuristics
/// config.use_random_order = true;   // Non-deterministic exploration
/// // May find better solutions but results vary between runs
/// ```
///
/// # Performance Guidelines
///
/// | Problem Size | Recommended Setting | Expected Speedup |
/// |--------------|-------------------|------------------|
/// | < 100 cubes | Default | N/A (very fast) |
/// | 100-1000 cubes | `single_expand = true` | 30-50% faster |
/// | > 1000 cubes | `single_expand = true` | 40-60% faster |
///
/// Quality trade-off with `single_expand = true`: typically 5-10% larger results.
///
/// # Algorithm Background
///
/// Espresso uses a heuristic approach with several phases:
/// 1. **Reduce** - Remove redundant literals from cubes
/// 2. **Expand** - Enlarge cubes to cover more area
/// 3. **Irredundant** - Remove covered cubes
/// 4. **Lastgasp** - Final optimization pass
///
/// The configuration controls how aggressively each phase operates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspressoConfig {
    /// Enable debugging output to stderr
    ///
    /// When `true`, prints detailed information about the minimization process.
    /// Useful for understanding algorithm behavior but verbose.
    ///
    /// **Default:** `false`
    pub debug: bool,

    /// Enable verbose debugging output
    ///
    /// Even more detailed than `debug`. Only use for deep algorithm analysis.
    ///
    /// **Default:** `false`
    pub verbose_debug: bool,

    /// Print trace information during minimization
    ///
    /// Shows progress through different minimization phases.
    ///
    /// **Default:** `false`
    pub trace: bool,

    /// Print summary statistics after minimization
    ///
    /// Shows cube counts, execution time, and optimization metrics.
    ///
    /// **Default:** `false`
    pub summary: bool,

    /// Remove essential prime implicants before minimization
    ///
    /// Essential primes are terms that must be in any minimal cover. Removing
    /// them first can speed up minimization for large problems.
    ///
    /// **Default:** `true` (recommended)
    pub remove_essential: bool,

    /// Force the cover to be irredundant
    ///
    /// Ensures no cube can be removed without changing the function. Should
    /// normally be enabled for minimal results.
    ///
    /// **Default:** `true` (recommended)
    pub force_irredundant: bool,

    /// Unwrap the onset before minimization
    ///
    /// A preprocessing step that can improve results for certain functions.
    ///
    /// **Default:** `true` (recommended)
    pub unwrap_onset: bool,

    /// Use single expand mode (faster but may be less optimal)
    ///
    /// Performs only one expand phase instead of iterating. Significantly
    /// faster for large problems, with minimal quality loss in practice.
    ///
    /// **Performance vs Quality:** Set `true` for speed, `false` for optimal results.
    ///
    /// **Default:** `false`
    pub single_expand: bool,

    /// Use super gasp heuristic
    ///
    /// An enhanced version of the GASP (Generalized Algorithm for Simplification
    /// of Products) heuristic that can find better solutions at some cost.
    ///
    /// **Default:** `false`
    pub use_super_gasp: bool,

    /// Use random order for processing
    ///
    /// Randomizes the order of cube processing. Can occasionally find better
    /// solutions but makes results non-deterministic.
    ///
    /// **Default:** `false` (deterministic results)
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
