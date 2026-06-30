//! # Espresso Logic Minimizer
//!
//! This crate provides Rust bindings to the Espresso heuristic logic minimiser
//! (Version 2.3), a classic tool from UC Berkeley for minimising Boolean functions.
//!
//! ## Overview
//!
//! Espresso takes a Boolean function represented as a sum-of-products (cover) and
//! produces a minimal or near-minimal equivalent representation. It's particularly
//! useful for:
//!
//! - Digital logic synthesis
//! - PLA (Programmable Logic Array) minimisation
//! - Boolean function simplification
//! - Logic optimisation in CAD tools
//!
//! ## API Levels
//!
//! This crate provides **two API levels** to suit different needs:
//!
//! ### High-Level API (Recommended)
//!
//! The high-level API provides easy-to-use abstractions with automatic resource management:
//!
//! - **[`BoolExpr`]** - Owned, syntactic Boolean expressions with parsing, the bitwise operators
//!   (`&`, `|`, `^`, `!`) and evaluation
//! - **[`Bdd`]** - Canonical BDD handles from a [`BddContext`] (or thread-safe [`SyncBddContext`]) for
//!   logical equivalence, cofactors and quantification
//! - **[`Cover`]** - Dynamic covers with automatic dimension management
//! - **[`Cube`]** / **[`Minterm`]** / **[`OutputSet`]** - A `Cover`'s product terms: a [`Cube`] pairs an
//!   input [`Minterm`] (a label-carrying row of tri-state values, `1`/`0`/`-`) with an [`OutputSet`]
//!   (a binary, one-bit-per-output membership bitmap). [`Cube::inputs`] returns `&Minterm`;
//!   [`Cube::outputs`] returns `&OutputSet`.
//!
//! **Benefits:**
//! - Automatic memory management
//! - No manual dimension tracking
//! - Thread-safe by design
//! - Idiomatic Rust API
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
//! - **Lower per-call overhead** - skips the high-level validation and output-[`Cover`] construction;
//!   measured ~10–14% faster on small covers but only ~1–5% (within measurement noise) on large ones,
//!   since the gap is a fixed per-call cost that minimisation soon dwarfs (machine-/input-dependent —
//!   see the `api_overhead` group in `benches/pla_benchmarks.rs`)
//! - **Explicit instance control** - Manually manage when Espresso instances are created/destroyed
//!
//! **Note:** Algorithm configuration via [`EspressoConfig`] works with **both** APIs -
//! it's not a reason to use the low-level API.
//!
//! **Important constraints:**
//! - ⚠️ **All covers on a thread must use the same dimensions** until dropped
//! - Requires manual dimension management
//! - More complex error handling
//!
//! See the [`espresso`] module documentation for detailed usage and safety guidelines.
//!
//! ## Using the High-Level API
//!
//! ### 1. Boolean Expressions (Recommended for most use cases)
//!
//! [`BoolExpr`] is an owned, syntactic value, composed with the bitwise operators `&` (AND), `|` (OR),
//! `^` (XOR), `!` (NOT), by value or by reference:
//!
//! ```
//! use espresso_logic::BoolExpr;
//!
//! let a = BoolExpr::var("a");
//! let b = BoolExpr::var("b");
//!
//! // XOR, built from the operators.
//! let xor = (&a & !&b) | (!&a & &b);
//! println!("{xor}");  // a & !b | !a & b (minimal parentheses)
//! ```
//!
//! Parse expressions from strings (the `*`/`+`/`~` and `&`/`|`/`!` spellings both parse):
//!
//! ```
//! use espresso_logic::BoolExpr;
//!
//! # fn main() -> Result<(), espresso_logic::expression::ParseBoolExprError> {
//! let expr = BoolExpr::parse("a & b | !a & !b")?;
//! println!("{expr}");
//! # Ok(())
//! # }
//! ```
//!
//! `BoolExpr` is purely syntactic: `a & b` and `b & a` are different values, and equality compares the
//! token structure, not the Boolean function. For canonical, semantic work — logical equivalence,
//! cofactors, quantification — build a [`Bdd`] handle in a [`BddContext`] minted by
//! [`bdd_context!`](crate::bdd_context):
//!
//! ```
//! use espresso_logic::{bdd_context, BoolExpr};
//!
//! # fn main() -> Result<(), espresso_logic::expression::ParseBoolExprError> {
//! let ctx = bdd_context!();
//! let a = ctx.var("a");
//! let b = ctx.var("b");
//!
//! // Handles are Copy; the BDD layer canonicalises, so logical laws hold.
//! assert!((a & b).equivalent_to(b & a));
//! assert!((a | !a).is_tautology());
//!
//! // Build a parsed expression into the same context and compare functions.
//! let parsed = ctx.parse("a & b")?;
//! assert!((a & b).equivalent_to(parsed));
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
//! use espresso_logic::{BoolExpr, Cover, CoverType, Minimizable};
//!
//! # fn main() -> std::io::Result<()> {
//! let a = BoolExpr::variable("a");
//! let b = BoolExpr::variable("b");
//! let expr = a.and(&b).or(&a.and(&b.not()));
//!
//! // Create cover and add expression
//! let mut cover = Cover::new(CoverType::F);
//! cover.add_expr(&expr, "output")?;
//!
//! // Access cover properties
//! println!("Input variables: {:?}", cover.input_labels());
//! println!("Number of cubes: {}", cover.num_cubes());
//!
//! // Minimise the cover
//! cover = cover.minimize()?;
//!
//! // Convert back to expression
//! let minimized = cover.to_expr("output")?;
//! println!("Minimised: {}", minimized);
//! # Ok(())
//! # }
//! ```
//!
//! ### 2. Manual Cube Construction
//!
//! Build covers by manually adding cubes (dimensions grow automatically):
//!
//! ```
//! use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};
//!
//! # fn main() -> std::io::Result<()> {
//! // Create a cover (dimensions grow automatically)
//! let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
//!
//! // Build the ON-set (truth table)
//! cover.push(Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F));  // 01 -> 1
//! cover.push(Cube::anonymous(&[Some(true), Some(false)], &[true], CubeType::F));  // 10 -> 1
//!
//! // Minimise (returns new instance)
//! cover = cover.minimize()?;
//!
//! // Iterate over minimised cubes
//! for cube in cover.cubes() {
//!     println!("Cube: {:?} -> {:?}", cube.inputs(), cube.outputs());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### 3. PLA Files
//!
//! Covers can be read from and written to PLA format files (compatible with original Espresso):
//!
//! ```
//! use espresso_logic::{Cover, CoverType, Minimizable, PlaCover, Symbol, PLAWriter};
//! # use std::io::Write;
//!
//! # fn main() -> std::io::Result<()> {
//! # let mut temp = tempfile::NamedTempFile::new()?;
//! # temp.write_all(b".i 2\n.o 1\n.p 1\n01 1\n.e\n")?;
//! # temp.flush()?;
//! # let input_path = temp.path();
//! // Read from a PLA file into a `PlaCover` (the variant reflects which label sections were present)
//! let mut cover = PlaCover::<Symbol>::from_pla_file(input_path)?;
//!
//! // Minimise
//! cover = cover.minimize()?;
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
//! let cover2 = PlaCover::<Symbol>::from_pla_reader(reader)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Cover Types
//!
//! The library supports different cover types for representing Boolean functions:
//!
//! - **F Type** - ON-set only (specifies where output is 1)
//! - **FD Type** - ON-set + Don't-cares
//! - **FR Type** - ON-set + OFF-set (specifies both 1s and 0s)
//! - **FDR Type** - ON-set + Don't-cares + OFF-set (complete specification)
//!
//! ```
//! use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType};
//!
//! # fn main() -> std::io::Result<()> {
//! // F type (ON-set only)
//! let mut f_cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
//! f_cover.push(Cube::anonymous(&[Some(true), Some(true)], &[true], CubeType::F));
//!
//! // FD type (ON-set + Don't-cares)
//! let mut fd_cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::FD);
//! fd_cover.push(Cube::anonymous(&[Some(true), Some(true)], &[true], CubeType::F));  // ON
//! fd_cover.push(Cube::anonymous(&[Some(false), Some(false)], &[true], CubeType::D));  // Don't-care
//! # Ok(())
//! # }
//! ```
//!
//! ## Thread Safety and Concurrency
//!
//! ### High-Level API ([`Cover`])
//!
//! [`Cover`] is `Send` and `Sync`, making it freely shareable across threads. The key
//! advantage is that Espresso instances are created **lazily on-demand** - only when
//! `.minimize()` is called, the thread-local Espresso instance is created for that thread.
//!
//! ```
//! use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};
//! use std::thread;
//!
//! # fn main() -> std::io::Result<()> {
//! // Covers can be freely moved between threads
//! let handles: Vec<_> = (0..4).map(|_| {
//!     thread::spawn(move || {
//!         let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
//!         cover.push(Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F));
//!         cover.push(Cube::anonymous(&[Some(true), Some(false)], &[true], CubeType::F));
//!         
//!         // Creates thread-local Espresso instance on first minimize()
//!         cover = cover.minimize()?;
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
//! ### Low-Level API ([`espresso`])
//!
//! The low-level API uses C11 thread-local storage. Each thread gets its own independent
//! Espresso instance and global state, but types are `!Send` and `!Sync`. See the
//! [`espresso`] module for details on dimension constraints.
//!
//! ## Using the Low-Level API (Advanced)
//!
//! For maximum performance and fine-grained control, use the [`espresso`] module directly:
//!
//! ```
//! use espresso_logic::espresso::{Espresso, EspressoCover};
//! use espresso_logic::EspressoConfig;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Explicit instance creation with custom config
//! let mut config = EspressoConfig::default();
//! config.single_expand = true;  // Faster mode
//! let _esp = Espresso::new(2, 1, &config);
//!
//! // Create cover with raw cube data
//! let cubes = [
//!     (&[0, 1][..], &[1][..]),  // 01 -> 1
//!     (&[1, 0][..], &[1][..]),  // 10 -> 1
//! ];
//! let cover = EspressoCover::from_cubes(&cubes, 2, 1)?;
//!
//! // Minimise and get all three covers (F, D, R)
//! let (f_result, d_result, r_result) = cover.minimize(None, None);
//!
//! println!("ON-set: {} cubes", f_result.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len());
//! println!("Don't-care: {} cubes", d_result.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len());
//! println!("OFF-set: {} cubes", r_result.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len());
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
//! let cubes1 = [(&[0, 1][..], &[1][..])];
//! let cover1 = EspressoCover::from_cubes(&cubes1, 2, 1)?;
//! let cubes2 = [(&[1, 0][..], &[1][..])];
//! let cover2 = EspressoCover::from_cubes(&cubes2, 2, 1)?;
//!
//! // Must drop before using different dimensions
//! drop(cover1);
//! drop(cover2);
//!
//! // Now 3 inputs works
//! let cubes3 = [(&[0, 1, 0][..], &[1][..])];
//! let cover3 = EspressoCover::from_cubes(&cubes3, 3, 1)?;
//! # Ok(())
//! # }
//! ```
//!
//! See the [`espresso`] module documentation for detailed safety guidelines and usage patterns.
//!
//! # Comprehensive Guides
//!
//! See the [`doc`] module for embedded guides:
//!
//! - [`doc::examples`] - Complete usage examples for all features
//! - [`doc::boolean_expressions`] - Boolean expression API deep dive
//! - [`doc::pla_format`] - PLA file format specification
//! - [`doc::cli`] - Command-line tool documentation

// Public modules
pub mod bdd;
pub mod cover;
pub mod error;
pub mod espresso;
pub mod expression;
pub mod symbol;
/// Raw bindgen-generated FFI bindings to the vendored C Espresso sources.
///
/// Hidden from the documented public surface: these are unsafe, ABI-level types whose shape is dictated
/// by bindgen and the C headers, not part of the stable API. Use the safe [`espresso`] wrappers
/// instead. Kept reachable only for the low-level wrapper layer.
#[doc(hidden)]
pub mod sys;

// Re-export high-level public API
pub use cover::pla::{PLAWriter, PlaCover, PlaLabel};
pub use cover::{
    Anonymous, Cover, CoverType, Cube, CubeType, Label, Minimizable, Minterm, OutputSet,
    ReconcilableLabel, StringLabel, Symbols,
};
pub use bdd::{Bdd, BddContext, BddNode, Brand, SyncBddContext};
pub use espresso::EspressoConfig;
pub use expression::{BoolExpr, ExprNode};
pub use symbol::Symbol;

/// Create a fresh, single-threaded [`BddContext`] with a private BDD manager.
///
/// Each call mints a unique brand, so handles ([`Bdd`]) from two different contexts cannot be combined
/// — it is a compile error, not a runtime check. The context owns an independent node table; there is
/// no process-global manager. The resulting `BddContext` is `!Send`/`!Sync`; use
/// [`sync_bdd_context!`](crate::sync_bdd_context) for a thread-safe one.
///
/// - `bdd_context!()` — an anonymous brand, unique to this call site/invocation.
/// - `bdd_context!(Name)` — a named brand. The name is only a readable label: each call still mints a
///   *distinct* brand (mixing two contexts is always a compile error, even two named the same), but a
///   mismatch then reads `expected Routing, found Timing` instead of an opaque internal type name.
///   Give distinct contexts distinct names; prefer the anonymous form when you do not need the label.
///
/// ```
/// use espresso_logic::{bdd_context, BoolExpr};
///
/// let ctx = bdd_context!();
/// let a = ctx.var("a");
/// let b = ctx.var("b");
/// assert!((a & b).equivalent_to(ctx.build(&BoolExpr::parse("a & b").unwrap())));
/// ```
#[macro_export]
macro_rules! bdd_context {
    () => {
        $crate::bdd_context!(__EspressoBddBrand)
    };
    ($name:ident) => {{
        #[derive(Clone, Copy)]
        struct $name;
        impl $crate::bdd::__macro_support::Sealed for $name {}
        impl $crate::bdd::Brand for $name {
            type Cell = $crate::bdd::__macro_support::LocalCell;
        }
        $crate::bdd::BddContext::<$name>::new()
    }};
}

/// Create a fresh, thread-safe [`SyncBddContext`] with a private BDD manager.
///
/// Like [`bdd_context!`](crate::bdd_context), but the minted brand selects a `RwLock`-backed cell, so
/// the resulting [`SyncBddContext`] is `Send + Sync` and can be moved to, or shared by reference
/// across, threads. Lock poisoning propagates. Each call mints a distinct brand, so handles from two
/// contexts never mix (a compile error).
///
/// ```
/// use espresso_logic::{sync_bdd_context, BoolExpr};
///
/// let ctx = sync_bdd_context!();
/// let a = ctx.var("a");
/// let b = ctx.var("b");
/// assert!((a | b).equivalent_to(ctx.build(&BoolExpr::parse("a | b").unwrap())));
/// ```
#[macro_export]
macro_rules! sync_bdd_context {
    () => {
        $crate::sync_bdd_context!(__EspressoSyncBddBrand)
    };
    ($name:ident) => {{
        #[derive(Clone, Copy)]
        struct $name;
        impl $crate::bdd::__macro_support::Sealed for $name {}
        impl $crate::bdd::Brand for $name {
            type Cell = $crate::bdd::__macro_support::SyncCell;
        }
        $crate::bdd::SyncBddContext::<$name>::new()
    }};
}

/// Comprehensive documentation guides
///
/// This module contains embedded guides from the `docs/` directory,
/// making the documentation available on docs.rs.
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
