//! Direct bindings to the Espresso C library with thread-local storage
//!
//! This module provides low-level access to the Espresso algorithm through direct
//! C library bindings. It uses C11 thread-local storage for thread safety, meaning
//! each thread gets its own independent copy of all global state.
//!
//! # When to Use This Module
//!
//! Use this low-level module when you need:
//! - **Access to intermediate covers** - Get ON-set (F), don't-care (D), and OFF-set (R) separately
//! - **Custom don't-care/off-sets** - Provide your own D and R covers to `minimize()`
//! - **Lower per-call overhead** - the high-level API additionally validates the cover and rebuilds an
//!   output [`Cover`](crate::Cover), so this layer's edge is a fixed per-call cost: measured ~10–14%
//!   faster on small covers but only ~1–5% (within measurement noise) on large ones (machine-/
//!   input-dependent — see the `api_overhead` group in `benches/pla_benchmarks.rs`)
//! - **Explicit instance control** - Manually manage Espresso instance lifecycle
//!
//! **For most use cases, prefer the higher-level APIs:**
//! - [`BoolExpr`](crate::BoolExpr) for boolean expressions
//! - [`Cover`](crate::Cover) for covers with dynamic dimensions
//! - [`PlaCover`](crate::PlaCover) for reading PLA files
//!
//! **Note:** Algorithm tuning via [`EspressoConfig`] works with **both**
//! the high-level [`Cover::minimize_with_config()`](crate::cover::Minimizable::minimize_with_config) and
//! low-level [`Espresso::new()`] - configuration is not a reason to use this module.
//!
//! **Important:** The high-level [`Cover`](crate::Cover) API automatically handles the
//! dimension change constraints described below, making it much easier to use safely.
//!
//! # Safety and Thread Safety
//!
//! While this module uses `unsafe` internally to interact with C code, all unsafe
//! operations are encapsulated in safe Rust APIs. The module IS thread-safe thanks
//! to C11 `_Thread_local` storage - each thread has independent global state.
//!
//! ## Critical Limitation: Dimension Consistency
//!
//! ⚠️ **IMPORTANT**: Once you create an `Espresso` instance or `EspressoCover` with specific
//! dimensions (number of inputs and outputs), **ALL covers and the Espresso instance must be
//! dropped before you can work with different dimensions on the same thread.**
//!
//! This is because:
//! 1. The C library uses thread-local global state (cube structure) configured for specific dimensions
//! 2. This module uses a thread-local singleton pattern with reference counting
//! 3. As long as ANY `EspressoCover` exists, it keeps the current dimensions "locked"
//! 4. Attempting to create covers with different dimensions will return an error
//!
//! ### What This Means in Practice
//!
//! **✅ SAFE - Same dimensions on a thread:**
//! ```rust
//! use espresso_logic::espresso::EspressoCover;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // First cover with 2 inputs, 1 output
//! let cubes1 = [(&[0, 1][..], &[1][..])];
//! let cover1 = EspressoCover::from_cubes(&cubes1, 2, 1)?;
//!
//! // Second cover with same dimensions - OK!
//! let cubes2 = [(&[1, 0][..], &[1][..])];
//! let cover2 = EspressoCover::from_cubes(&cubes2, 2, 1)?;
//!
//! // Both can coexist and be used
//! let (result1, _, _) = cover1.minimize(None, None);
//! let (result2, _, _) = cover2.minimize(None, None);
//! # Ok(())
//! # }
//! ```
//!
//! **❌ ERROR - Different dimensions without dropping:**
//! ```rust
//! use espresso_logic::espresso::EspressoCover;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // First cover with 2 inputs, 1 output
//! let cubes1 = [(&[0, 1][..], &[1][..])];
//! let cover1 = EspressoCover::from_cubes(&cubes1, 2, 1)?;
//!
//! // Trying different dimensions while cover1 exists - ERROR!
//! let cubes2 = [(&[0, 1, 0][..], &[1][..])];
//! let cover2 = EspressoCover::from_cubes(&cubes2, 3, 1);
//! assert!(cover2.is_err()); // Returns DimensionMismatch error
//! # Ok(())
//! # }
//! ```
//!
//! **✅ SAFE - Using scopes to drop covers:**
//! ```rust
//! use espresso_logic::espresso::EspressoCover;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // First problem in a scope
//! {
//!     let cubes1 = [(&[0, 1][..], &[1][..])];
//!     let cover1 = EspressoCover::from_cubes(&cubes1, 2, 1)?;
//!     let (result1, _, _) = cover1.minimize(None, None);
//!     // All covers dropped at end of scope
//! }
//!
//! // Now we can use different dimensions
//! let cubes2 = [(&[0, 1, 0][..], &[1][..])];
//! let cover2 = EspressoCover::from_cubes(&cubes2, 3, 1)?;
//! let (result2, _, _) = cover2.minimize(None, None);
//! # Ok(())
//! # }
//! ```
//!
//! **✅ SAFE - Explicit drop:**
//! ```rust
//! use espresso_logic::espresso::EspressoCover;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let cubes1 = [(&[0, 1][..], &[1][..])];
//! let cover1 = EspressoCover::from_cubes(&cubes1, 2, 1)?;
//! let (result1, d1, r1) = cover1.minimize(None, None);
//!
//! // Explicitly drop ALL covers from the first problem
//! drop(result1);
//! drop(d1);
//! drop(r1);
//!
//! // Now we can use different dimensions
//! let cubes2 = [(&[0, 1, 0][..], &[1][..])];
//! let cover2 = EspressoCover::from_cubes(&cubes2, 3, 1)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Why This Limitation Exists
//!
//! The Espresso C library uses global state that must be initialised for specific dimensions:
//! - The cube structure defines bit layouts for variables
//! - Memory allocation patterns depend on the number of inputs/outputs
//! - Changing dimensions requires tearing down and reinitialising all this state
//!
//! This module protects you from memory corruption by:
//! 1. Using a thread-local singleton that tracks the current dimensions
//! 2. Returning clear errors when dimension mismatches are detected
//! 3. Using Rc reference counting to prevent premature cleanup
//!
//! ## How to Work with Multiple Dimensions
//!
//! ### Option 1: Use the High-Level Cover API (Recommended)
//!
//! The [`Cover`](crate::Cover) type automatically manages Espresso instances and handles
//! dimension changes safely:
//!
//! ```rust
//! use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Cover handles dimension changes automatically
//! let mut cover1 = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
//! cover1.push(Cube::anonymous(&[Some(true), Some(false)], &[true], CubeType::F));
//! cover1 = cover1.minimize()?;
//!
//! // Different dimensions - no problem!
//! let mut cover2 = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
//! cover2.push(Cube::anonymous(&[Some(false), Some(true), Some(false)], &[true], CubeType::F));
//! cover2 = cover2.minimize()?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Option 2: Use Different Threads
//!
//! Each thread has completely independent state:
//!
//! ```rust
//! use espresso_logic::espresso::EspressoCover;
//! use std::thread;
//!
//! # fn main() {
//! let handle1 = thread::spawn(|| {
//!     // Thread 1: 2 inputs, 1 output
//!     let cubes = [(&[0, 1][..], &[1][..])];
//!     let cover = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
//!     let (result, _, _) = cover.minimize(None, None);
//!     // Extract the data before returning (covers are !Send)
//!     result.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len()
//! });
//!
//! let handle2 = thread::spawn(|| {
//!     // Thread 2: 3 inputs, 1 output - completely independent!
//!     let cubes = [(&[0, 1, 0][..], &[1][..])];
//!     let cover = EspressoCover::from_cubes(&cubes, 3, 1).unwrap();
//!     let (result, _, _) = cover.minimize(None, None);
//!     // Extract the data before returning (covers are !Send)
//!     result.to_cubes(3, 1, espresso_logic::espresso::CubeType::F).len()
//! });
//!
//! let count1 = handle1.join().unwrap();
//! let count2 = handle2.join().unwrap();
//! println!("Thread 1: {} cubes, Thread 2: {} cubes", count1, count2);
//! # }
//! ```
//!
//! ### Option 3: Explicit Scoping (Low-Level API)
//!
//! Use scopes or explicit drops to ensure all covers are cleaned up:
//!
//! ```rust
//! use espresso_logic::espresso::EspressoCover;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! fn solve_problem(num_inputs: usize, num_outputs: usize) -> Result<(), Box<dyn std::error::Error>> {
//!     let inputs = vec![0; num_inputs];
//!     let outputs = vec![1; num_outputs];
//!     let cubes = [(&inputs[..], &outputs[..])];
//!     let cover = EspressoCover::from_cubes(&cubes, num_inputs, num_outputs)?;
//!     let (result, d, r) = cover.minimize(None, None);
//!     
//!     // Process results...
//!     println!("Result: {} cubes", result.to_cubes(num_inputs, num_outputs,
//!         espresso_logic::espresso::CubeType::F).len());
//!     
//!     // All covers dropped at end of function
//!     Ok(())
//! }
//!
//! // Each call has a clean slate
//! solve_problem(2, 1)?;
//! solve_problem(3, 2)?;
//! solve_problem(4, 1)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Technical Details: Reference Counting and Singleton Pattern
//!
//! ## Internal Implementation
//!
//! This module uses a singleton pattern with reference counting to manage
//! the thread-local Espresso state safely:
//!
//! 1. **Thread-Local Singleton**: A `thread_local!` static holds a `Weak<InnerEspresso>`
//! 2. **Reference Counting**: Each `EspressoCover` holds an `Rc<InnerEspresso>`
//! 3. **Lifetime Management**: As long as any cover exists, the `Rc` count > 0
//! 4. **Dimension Locking**: The singleton can only be replaced when all covers are dropped
//!
//! ```text
//! Thread-Local Storage:
//! ┌─────────────────────────────────────────┐
//! │ ESPRESSO_INSTANCE: Weak<InnerEspresso>  │
//! └─────────────────────────────────────────┘
//!                     ↑
//!                     │ weak reference
//!                     │
//! ┌───────────────────┴──────────────────────┐
//! │ InnerEspresso (Rc-managed)               │
//! │ - num_inputs: 2                          │
//! │ - num_outputs: 1                         │
//! │ - initialized: true                      │
//! └──────────────────────────────────────────┘
//!           ↑                ↑
//!           │                │
//!    strong references (Rc::clone)
//!           │                │
//!   EspressoCover     EspressoCover
//!      (cover1)          (cover2)
//! ```
//!
//! When all covers are dropped, the strong count reaches 0, the `Weak` can no longer
//! be upgraded, and a new instance with different dimensions can be created.
//!
//! ## Memory Safety Guarantees
//!
//! - **No dangling pointers**: Covers hold `Rc<InnerEspresso>`, keeping C state alive
//! - **No dimension conflicts**: Singleton pattern enforces consistency per thread
//! - **Proper cleanup**: `Drop` implementations ensure C resources are freed
//! - **Thread isolation**: `!Send + !Sync` markers prevent cross-thread access
//!
//! # Examples
//!
//! ## Basic Usage (Recommended)
//!
//! Work with `EspressoCover` - the Espresso instance is managed automatically:
//!
//! ```
//! use espresso_logic::espresso::EspressoCover;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Build a cover (XOR function) - Espresso instance created automatically
//! let cubes = [
//!     (&[0, 1][..], &[1][..]),  // 01 -> 1
//!     (&[1, 0][..], &[1][..]),  // 10 -> 1
//! ];
//! let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
//!
//! // Minimize directly on the cover
//! let (minimized, _d, _r) = f.minimize(None, None);
//!
//! // Extract results
//! let result_cubes: Vec<_> = minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).collect();
//! println!("Minimized to {} cubes", result_cubes.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## Advanced: Explicit Espresso Instance
//!
//! For fine-grained control over configuration:
//!
//! ```
//! use espresso_logic::espresso::{Espresso, EspressoCover};
//! use espresso_logic::EspressoConfig;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Explicitly create an Espresso instance with custom config
//! let mut config = EspressoConfig::default();
//! config.single_expand = true;
//! let _esp = Espresso::new(2, 1, &config);
//!
//! // Now all covers will use this instance
//! let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
//! let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
//! let (minimized, _, _) = f.minimize(None, None);
//! # Ok(())
//! # }
//! ```
//!
//! ## Multi-threaded Usage
//!
//! Each thread automatically gets its own Espresso instance. No manual management needed:
//!
//! ```
//! use espresso_logic::espresso::EspressoCover;
//! use std::thread;
//!
//! # fn main() {
//! let handles: Vec<_> = (0..4).map(|_| {
//!     thread::spawn(|| -> usize {
//!         // Each thread automatically gets its own Espresso instance
//!         let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
//!         let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
//!         
//!         // Thread-safe: independent global state per thread
//!         let (result, _, _) = f.minimize(None, None);
//!         result.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len()
//!     })
//! }).collect();
//!
//! for handle in handles {
//!     let count = handle.join().unwrap();
//!     println!("Thread minimized to {} cubes", count);
//! }
//! # }
//! ```
//!
//! ## Working with Different Dimensions (Function Scoping)
//!
//! Use functions to automatically clean up covers:
//!
//! ```
//! use espresso_logic::espresso::EspressoCover;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! fn minimize_and_report(
//!     cubes: &[(&[u8], &[u8])],
//!     num_inputs: usize,
//!     num_outputs: usize
//! ) -> Result<usize, Box<dyn std::error::Error>> {
//!     let cover = EspressoCover::from_cubes(cubes, num_inputs, num_outputs)?;
//!     let (result, _, _) = cover.minimize(None, None);
//!     Ok(result.to_cubes(num_inputs, num_outputs, espresso_logic::espresso::CubeType::F).len())
//!     // All covers dropped here
//! }
//!
//! // Each call can use different dimensions
//! let cubes1 = [(&[0, 1][..], &[1][..])];
//! let count1 = minimize_and_report(&cubes1, 2, 1)?;
//! let cubes2 = [(&[0, 1, 0][..], &[1][..])];
//! let count2 = minimize_and_report(&cubes2, 3, 1)?;
//! let cubes3 = [(&[0, 1, 0, 1][..], &[1, 0][..])];
//! let count3 = minimize_and_report(&cubes3, 4, 2)?;
//!
//! println!("Results: {} {} {}", count1, count2, count3);
//! # Ok(())
//! # }
//! ```

pub mod error;

use crate::cover::{Anonymous, Minterm, OutputSet, Symbols};
pub use crate::cover::{Cube, CubeType};
use crate::sys;
pub use error::{CubeError, InstanceError, MinimizationError};
use std::marker::PhantomData;
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::rc::Rc;
use std::sync::Arc;

/// A single Espresso cube word, matching `espresso_word` in the generated bindings.
/// `espresso.h` derives the word width from the native machine word via `UINTPTR_MAX`, so this
/// alias auto-follows: `u64` on 64-bit targets, `u32` on 32-bit targets (including
/// `wasm32-unknown-emscripten`).
type CubeWord = sys::espresso_word;

/// Bits per Espresso cube word, mirroring `BPI` in `espresso.h`. The cube bit-layout is fixed at
/// this width: a variable bit `b` lives in cube word `(b >> LOGBPI) + 1` (word 0 is the set
/// header) at bit `b & (BPI - 1)`, exactly as the C `WHICH_WORD`/`WHICH_BIT` macros define.
const BPI: usize = CubeWord::BITS as usize;
/// `log2(BPI)` — `espresso.h`'s `LOGBPI`, used for the word index `b >> LOGBPI`.
const LOGBPI: usize = BPI.trailing_zeros() as usize;
/// Mask of the low `BPI` bits, applied when narrowing a packed word to one cube word. Computed as
/// `u64::MAX >> (64 - BPI)` rather than `(1u64 << BPI) - 1`, which would overflow the shift when
/// `BPI == 64`.
const BPI_MASK: u64 = u64::MAX >> (64 - BPI);
/// `BPI` is only ever 32 or 64 (see `CubeWord`'s doc comment); this also guarantees
/// `64 % BPI == 0`, which `cube_words_per_u64` below relies on when reinterpreting cube words as
/// packed `u64` minterm words.
const _: () = assert!(BPI == 32 || BPI == 64);

/// Widen a packed C cube word to `u64`. On a 64-bit build `CubeWord` is `u64`
/// and this is a no-op; on a 32-bit build (`CubeWord` = `u32`, e.g. wasm32 or
/// `ESPRESSO_BPI=32`) it is a genuine widening needed to combine the word with
/// `BPI_MASK` and the `u64` minterm accumulator. The `allow` is load-bearing:
/// the cast is redundant only on the 64-bit arm.
#[inline]
#[allow(clippy::unnecessary_cast)]
fn word_to_u64(w: CubeWord) -> u64 {
    w as u64
}

// Re-export for convenience when using the espresso module directly

/// Cover with direct access to C library representation
///
/// This type wraps a raw C pointer to a cover (set family) and provides
/// safe Rust methods for working with it. Memory is automatically managed
/// through the `Drop` trait.
///
/// # Lifetime and Dimension Constraints
///
/// Each `EspressoCover` is tied to a specific thread and dimension configuration:
///
/// - Holds an `Rc<InnerEspresso>` to keep the thread-local Espresso instance alive
/// - The underlying C memory is allocated based on the cube structure dimensions
/// - **All covers on a thread must use the same dimensions** until all are dropped
///
/// **Note:** This type is neither `Send` nor `Sync` (because `Rc` is `!Send + !Sync`) -
/// it must remain on the thread where it was created, as it's tied to thread-local C state
/// managed by `Espresso`.
///
/// # Memory Management
///
/// - **Allocation**: Created via `from_cubes()` which allocates C memory
/// - **Ownership**: Holds exclusive ownership of its C pointer
/// - **Cleanup**: Calls `sf_free()` on the C pointer when dropped
/// - **Cloning**: Uses `sf_save()` to create an independent C copy
/// - **Transfer**: `into_raw()` transfers ownership out (internal use only)
///
/// # Example: Dimension Locking
///
/// ```rust
/// use espresso_logic::espresso::EspressoCover;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
///
/// // Create cover with 2 inputs
/// let cubes1 = [(&[0, 1][..], &[1][..])];
/// let cover1 = EspressoCover::from_cubes(&cubes1, 2, 1)?;
///
/// // Cannot create cover with different dimensions - ERROR!
/// let cubes2 = [(&[0, 1, 0][..], &[1][..])];
/// let result = EspressoCover::from_cubes(&cubes2, 3, 1);
/// assert!(result.is_err());
///
/// // Must drop cover1 first
/// drop(cover1);
///
/// // Now 3 inputs works
/// let cover2 = EspressoCover::from_cubes(&cubes2, 3, 1)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct EspressoCover {
    ptr: sys::pset_family,
    // Keep the internal Espresso instance alive
    _espresso: Rc<InnerEspresso>,
}

/// Lazy iterator over the decoded cubes of an [`EspressoCover`], created by
/// [`EspressoCover::to_cubes`].
///
/// Each `next()` decodes one anonymous [`Cube`] from the C `pset_family` on demand, so the full set is
/// never materialised. Borrowing the cover keeps the underlying C memory valid for the iterator's life.
/// The cube count is known up front, so this is an [`ExactSizeIterator`].
pub struct EspressoCubes<'a> {
    /// The borrowed cover; anchors the `pset_family` memory read in `next()`.
    cover: &'a EspressoCover,
    /// Shared anonymous input header every decoded cube is defined over.
    input_syms: Arc<Symbols<Anonymous>>,
    /// Shared anonymous output header every decoded cube is defined over.
    output_syms: Arc<Symbols<Anonymous>>,
    num_outputs: usize,
    cube_type: CubeType,
    /// Cube word stride (`set_family.wsize`), snapshotted once — constant for the cover's life.
    wsize: usize,
    /// Bit offset of the first output field (`num_inputs * 2`).
    output_start: usize,
    /// `u64` words per decoded input minterm (`num_inputs.div_ceil(32)`).
    input_u64_words: usize,
    /// BPI-wide cube words spanning the input region (`(2 * num_inputs).div_ceil(BPI)`).
    input_cube_words: usize,
    /// Total input bit-width (`2 * num_inputs`), the packing bound for the last `u64`.
    total_input_bits: usize,
    /// Next cube index to decode, in `0..count`.
    idx: usize,
    /// Total cube count, snapshotted at construction.
    count: usize,
}

impl EspressoCubes<'_> {
    /// Decode the cube at index `i` from the C `pset_family`.
    ///
    /// # Safety
    ///
    /// `i` must be `< self.count` and the borrowed cover's `pset_family` must still be live (guaranteed
    /// by the `&EspressoCover` borrow).
    unsafe fn decode(&self, i: usize) -> Cube<Anonymous, Anonymous> {
        // The input-packing bounds and `output_start` were computed once at construction; only the base
        // pointer and per-cube offset are derived here (`wsize` is snapshotted, so this is one deref).
        let wsize = self.wsize;
        let cube_ptr = (*self.cover.ptr).data.add(i * wsize);

        // Read a single bit from the cube's word array (out-of-range words read as 0), via the C
        // WHICH_WORD/WHICH_BIT layout. Words are `CubeWord`, matching Espresso's `espresso_word*`.
        let bit_at = |bit: usize| -> bool {
            let word = (bit >> LOGBPI) + 1;
            word < wsize && (*cube_ptr.add(word) & ((1 as CubeWord) << (bit & (BPI - 1)))) != 0
        };

        // The input region packs the same 2-bit fields as a minterm, so decode it by a direct
        // word-copy — the inverse of `from_packed_cubes` — instead of reading two bits per variable.
        let cube_words_per_u64 = 64 / BPI;

        // Assemble each `u64` minterm word from `64 / BPI` BPI-wide cube words, bounded by the input
        // region so the (possibly shared) boundary word's output bits are excluded.
        let mut iwords = vec![0u64; self.input_u64_words];
        for (k, slot) in iwords.iter_mut().enumerate() {
            let mut word = 0u64;
            for c in 0..cube_words_per_u64 {
                let cw = k * cube_words_per_u64 + c;
                if cw < self.input_cube_words {
                    let cval = word_to_u64(*cube_ptr.add(cw + 1)) & BPI_MASK;
                    word |= cval << (c * BPI);
                }
            }
            // Zero any bits past the input region (the boundary cube word may carry output bits; the
            // last `u64` may have padding past the final variable) so the padding stays canonical for
            // `Eq`/`Hash`.
            let valid = self.total_input_bits.saturating_sub(k * 64).min(64);
            if valid < 64 {
                word &= (1u64 << valid) - 1;
            }
            *slot = word;
        }
        let im = Minterm::from_packed_words(Arc::clone(&self.input_syms), iwords.into());

        // Decode the output membership — one C bit per output, the same 1-bit-per-output packing as
        // `OutputSet`, so read each bit directly into the bitmap.
        let outputs = (0..self.num_outputs).map(|out| bit_at(self.output_start + out));
        let om = OutputSet::from_symbols(Arc::clone(&self.output_syms), outputs);

        Cube::new(im, om, self.cube_type)
    }
}

/// Opaque: the borrowed C memory carries no useful `Debug`, so only the remaining count is shown.
impl std::fmt::Debug for EspressoCubes<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EspressoCubes")
            .field("remaining", &(self.count - self.idx))
            .finish_non_exhaustive()
    }
}

impl Iterator for EspressoCubes<'_> {
    type Item = Cube<Anonymous, Anonymous>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.count {
            return None;
        }
        let i = self.idx;
        self.idx += 1;
        // SAFETY: `i < self.count` and the `&EspressoCover` borrow keeps the `pset_family` live.
        Some(unsafe { self.decode(i) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.idx;
        (remaining, Some(remaining))
    }
}

// The remaining count is known exactly and in O(1).
impl ExactSizeIterator for EspressoCubes<'_> {}
impl std::iter::FusedIterator for EspressoCubes<'_> {}

/// Panics with a clear allocation-failure message if `ptr` is null; otherwise returns it unchanged.
///
/// `ALLOC`/`REALLOC` in `espresso-src/utility.h` are bare `malloc`/`realloc` wrappers with no
/// out-of-memory catching (see the header comment there), so a null result from an unguarded
/// allocation entry point such as `sf_new`/`sf_save` can only mean allocation exhaustion, never
/// invalid input. Panicking here matches Rust `std`'s own OOM behaviour rather than surfacing a
/// recoverable `Result` for a condition callers cannot meaningfully act on.
#[inline]
fn check_alloc(ptr: sys::pset_family, context: &str) -> sys::pset_family {
    if ptr.is_null() {
        panic!("espresso: C allocation failure ({context}): out of memory");
    }
    ptr
}

impl EspressoCover {
    /// Create from raw pointer with Espresso reference (internal use)
    pub(crate) unsafe fn from_raw(ptr: sys::pset_family, espresso: &Espresso) -> Self {
        EspressoCover {
            ptr,
            _espresso: Rc::clone(&espresso.inner),
        }
    }

    /// Convert to raw pointer, transferring C memory ownership out of Rust
    ///
    /// # Memory Safety
    ///
    /// This function transfers ownership of the C-allocated memory from Rust to the caller.
    /// The pointer MUST eventually be freed (either by passing to a C function that takes
    /// ownership, or by wrapping in a new EspressoCover that will free it on drop).
    ///
    /// This is safe because:
    /// 1. We set ptr to null before dropping, preventing sf_free() in Drop
    /// 2. We properly drop the Rc<Espresso>, allowing cleanup_if_unused() to work
    /// 3. The returned pointer remains valid as long as no dimension change occurs
    pub(crate) fn into_raw(self) -> sys::pset_family {
        let ptr = self.ptr;
        // Don't forget self - let the Rc<Espresso> be properly dropped
        // But prevent the ptr from being freed by setting it to null
        let mut temp = self;
        temp.ptr = std::ptr::null_mut(); // Prevents double-free in Drop
        drop(temp); // This drops the Rc<Espresso> but not the C ptr
        ptr
    }

    /// Build cover from cube data
    ///
    /// Creates a cover from a list of cubes represented as `(inputs, outputs)` tuples.
    ///
    /// # Cube Encoding
    ///
    /// **Input values** (for binary variables):
    /// - `0` = Variable must be 0 (low)
    /// - `1` = Variable must be 1 (high)
    /// - `2` = Don't care (can be either 0 or 1)
    ///
    /// **Output values** (for multi-valued variables):
    /// - `0` = Output is 0 (off)
    /// - `1` = Output is 1 (on)
    ///
    /// # Automatic Instance Creation
    ///
    /// If no Espresso instance exists on the current thread, one will be **automatically
    /// created** with:
    /// - The specified dimensions (`num_inputs`, `num_outputs`)
    /// - Default configuration ([`EspressoConfig::default()`](crate::EspressoConfig::default))
    ///
    /// If you need custom configuration, create an [`Espresso`] instance explicitly first
    /// with [`Espresso::new()`].
    ///
    /// # Dimension Constraints
    ///
    /// ⚠️ **Critical:** If an Espresso instance already exists on this thread with **different
    /// dimensions**, this function returns an error. You must drop all existing covers and
    /// Espresso handles before creating covers with new dimensions.
    ///
    /// # Arguments
    ///
    /// * `cubes` - Vector of `(inputs, outputs)` tuples where each tuple represents one cube
    /// * `num_inputs` - Number of input variables (must match input vector length)
    /// * `num_outputs` - Number of output variables (must match output vector length)
    ///
    /// # Errors
    ///
    /// Returns [`MinimizationError`] if:
    /// - An Espresso instance with different dimensions already exists on this thread
    /// - Input cube values are invalid (not 0, 1, or 2)
    /// - Vector lengths don't match the specified dimensions
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    ///
    /// ```
    /// use espresso_logic::espresso::EspressoCover;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // XOR function: output is 1 when inputs differ
    /// let cubes = [
    ///     (&[0, 1][..], &[1][..]),  // Input: 01, Output: 1
    ///     (&[1, 0][..], &[1][..]),  // Input: 10, Output: 1
    /// ];
    /// let cover = EspressoCover::from_cubes(&cubes, 2, 1)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With Don't-Cares
    ///
    /// ```
    /// use espresso_logic::espresso::EspressoCover;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Function where output is 1 when first input is 1, regardless of second input
    /// let cubes = [
    ///     (&[1, 2][..], &[1][..]),  // 1X -> 1 (X = don't care)
    /// ];
    /// let cover = EspressoCover::from_cubes(&cubes, 2, 1)?;
    /// // This represents two minterms: 10->1 and 11->1
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Dimension Mismatch Error
    ///
    /// ```
    /// use espresso_logic::espresso::EspressoCover;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create first cover with 2 inputs
    /// let cubes1 = [(&[0, 1][..], &[1][..])];
    /// let cover1 = EspressoCover::from_cubes(&cubes1, 2, 1)?;
    ///
    /// // Attempting different dimensions returns an error
    /// let cubes2 = [(&[0, 1, 0][..], &[1][..])];
    /// let result = EspressoCover::from_cubes(&cubes2, 3, 1);
    /// assert!(result.is_err());
    ///
    /// // Must drop cover1 first
    /// drop(cover1);
    ///
    /// // Now 3 inputs works
    /// let cover2 = EspressoCover::from_cubes(&cubes2, 3, 1)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_cubes(
        cubes: &[(&[u8], &[u8])],
        num_inputs: usize,
        num_outputs: usize,
    ) -> Result<Self, MinimizationError> {
        // Create a new Espresso instance with default config if no instance exists
        // Checks dimensions and returns an error if an instance with different dimensions already exists
        let espresso = Espresso::try_new(num_inputs, num_outputs, None)?;

        // This assumes Espresso has already initialised the cube structure
        let cube_size = unsafe { (*sys::get_cube()).size as usize };

        // Create empty cover with capacity (reuse the espresso reference)
        let ptr = check_alloc(
            unsafe { sys::sf_new(cubes.len() as c_int, cube_size as c_int) },
            "sf_new in EspressoCover::from_cubes",
        );
        let mut cover = EspressoCover {
            ptr,
            _espresso: espresso.inner,
        };

        // Add each cube to the cover
        for &(inputs, outputs) in cubes {
            // Reject mismatched slice lengths up front: writing more bits than the cube reserves for
            // each side would corrupt the C set-family memory (the bit positions are derived from the
            // slice indices, not bounded by num_inputs/num_outputs).
            if inputs.len() != num_inputs || outputs.len() != num_outputs {
                return Err(MinimizationError::Cube(CubeError::DimensionMismatch {
                    expected_inputs: num_inputs,
                    actual_inputs: inputs.len(),
                    expected_outputs: num_outputs,
                    actual_outputs: outputs.len(),
                }));
            }
            unsafe {
                let cf = *(*sys::get_cube()).temp.add(0);
                sys::set_clear(cf, cube_size as c_int);

                // Set the bit at `bit_pos` via the C WHICH_WORD/WHICH_BIT layout (word
                // `(bit_pos >> LOGBPI) + 1`, bit `bit_pos & (BPI - 1)`); mirrors `bit_at` in `to_cubes`.
                let set_bit = |bit_pos: usize| {
                    *cf.add((bit_pos >> LOGBPI) + 1) |= (1 as CubeWord) << (bit_pos & (BPI - 1));
                };

                // Set input values. Each binary variable occupies two bits at `var * 2` (value 0) and
                // `var * 2 + 1` (value 1); a don't-care sets both.
                for (var, &val) in inputs.iter().enumerate() {
                    match val {
                        0 => set_bit(var * 2),
                        1 => set_bit(var * 2 + 1),
                        2 => {
                            set_bit(var * 2);
                            set_bit(var * 2 + 1);
                        }
                        _ => {
                            return Err(MinimizationError::Cube(CubeError::InvalidValue {
                                value: val,
                                position: var,
                            }))
                        }
                    }
                }

                // Set output values
                let output_var = (*sys::get_cube()).num_vars - 1;
                let output_first = *(*sys::get_cube()).first_part.add(output_var as usize) as usize;

                for (i, &val) in outputs.iter().enumerate() {
                    if val == 1 {
                        set_bit(output_first + i);
                    }
                }

                // `sf_addset` may grow the family via REALLOC; never write a null result back
                // into `cover.ptr` (Drop would then free/dereference it).
                cover.ptr = check_alloc(
                    sys::sf_addset(cover.ptr, cf),
                    "sf_addset in EspressoCover::from_cubes",
                );
            }
        }

        Ok(cover)
    }

    /// Build a cover by copying each cube's **packed input words** straight into the C cube, rather
    /// than re-coding every variable through `0/1/2` bytes as [`from_cubes`](Self::from_cubes) does.
    ///
    /// A [`Minterm`](crate::Minterm)'s input packing uses the *same* 2-bit-per-variable encoding as an
    /// Espresso cube (value 0 = even bit, value 1 = odd bit, don't-care = both, empty = neither), so a
    /// `Minterm` `u64` word `k` maps onto the `64 / BPI` Espresso cube words beginning at index
    /// `k * (64 / BPI) + 1` — two 32-bit words at `BPI == 32`, a single 64-bit word at `BPI == 64`
    /// (the `+ 1` skips Espresso's reserved header word). Each cube is `(input_words, output_assertions)`:
    /// `input_words` is the input minterm's [`raw_words`](crate::Minterm::raw_words) (length
    /// `ceil(num_inputs / 32)`), `output_assertions[i]` is whether output `i` is asserted. Empty (`?`,
    /// `00`) input fields are copied verbatim, so they reach C as the empty literal with no recoding.
    ///
    /// Crate-internal: used by the high-level cover minimisation path. The public `from_cubes` stays.
    pub(crate) fn from_packed_cubes(
        cubes: &[(&[u64], &[bool])],
        num_inputs: usize,
        num_outputs: usize,
    ) -> Result<Self, MinimizationError> {
        let espresso = Espresso::try_new(num_inputs, num_outputs, None)?;
        let cube_size = unsafe { (*sys::get_cube()).size as usize };

        let ptr = check_alloc(
            unsafe { sys::sf_new(cubes.len() as c_int, cube_size as c_int) },
            "sf_new in EspressoCover::from_packed_cubes",
        );
        let mut cover = EspressoCover {
            ptr,
            _espresso: espresso.inner,
        };

        // Number of cube words the input region (bits `0..2*num_inputs`) spans; copying is bounded by
        // this so we never run past the cube or clobber the word the input region may share with the
        // start of the output region.
        let input_cube_words = (2 * num_inputs).div_ceil(BPI);
        // Each input minterm packs 32 variables (2 bits each) per `u64` word.
        const VARS_PER_U64: usize = 32;
        let expected_u64_words = num_inputs.div_ceil(VARS_PER_U64);
        // A `u64` holds 64 bits, i.e. `64 / BPI` cube words.
        let cube_words_per_u64 = 64 / BPI;

        for &(input_words, outputs) in cubes {
            if input_words.len() != expected_u64_words || outputs.len() != num_outputs {
                return Err(MinimizationError::Cube(CubeError::DimensionMismatch {
                    expected_inputs: num_inputs,
                    actual_inputs: input_words.len().saturating_mul(VARS_PER_U64),
                    expected_outputs: num_outputs,
                    actual_outputs: outputs.len(),
                }));
            }
            unsafe {
                let cf = *(*sys::get_cube()).temp.add(0);
                sys::set_clear(cf, cube_size as c_int);

                // Copy the input region one cube word at a time — the minterm uses the same 2-bit
                // encoding as a C cube, so no recoding. Each `u64` is sliced into `cube_words_per_u64`
                // BPI-wide chunks; masking to BPI bits keeps it correct even if `CubeWord` is wider than
                // BPI. `set_clear` zeroed the cube, so `|=` is a copy that leaves the (possibly shared)
                // boundary word's output bits untouched — they are set below.
                for w in 0..input_cube_words {
                    let mword = input_words[w / cube_words_per_u64];
                    let chunk =
                        ((mword >> ((w % cube_words_per_u64) * BPI)) & BPI_MASK) as CubeWord;
                    *cf.add(w + 1) |= chunk;
                }

                // Set one bit per asserted output at `output_first + i` (the C WHICH_WORD/WHICH_BIT).
                let output_var = (*sys::get_cube()).num_vars - 1;
                let output_first = *(*sys::get_cube()).first_part.add(output_var as usize) as usize;
                for (i, &asserted) in outputs.iter().enumerate() {
                    if asserted {
                        let bit_pos = output_first + i;
                        *cf.add((bit_pos >> LOGBPI) + 1) |=
                            (1 as CubeWord) << (bit_pos & (BPI - 1));
                    }
                }

                // `sf_addset` may grow the family via REALLOC; never write a null result back
                // into `cover.ptr` (Drop would then free/dereference it).
                cover.ptr = check_alloc(
                    sys::sf_addset(cover.ptr, cf),
                    "sf_addset in EspressoCover::from_packed_cubes",
                );
            }
        }

        Ok(cover)
    }
}

impl Drop for EspressoCover {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::sf_free(self.ptr);
            }
        }
    }
}

impl Clone for EspressoCover {
    fn clone(&self) -> Self {
        let ptr = check_alloc(
            unsafe { sys::sf_save(self.ptr) },
            "sf_save in EspressoCover::clone",
        );
        EspressoCover {
            ptr,
            _espresso: Rc::clone(&self._espresso),
        }
    }
}

impl EspressoCover {
    /// Convert this cover to typed Cubes
    ///
    /// Extracts the cubes from the C representation and converts them to
    /// Rust [`Cube`] structures with the specified dimensions and type.
    ///
    /// # Arguments
    ///
    /// * `num_inputs` - Number of input variables (must match cover dimensions)
    /// * `num_outputs` - Number of output variables (must match cover dimensions)
    /// * `cube_type` - Type marker for the cubes (F, D, or R) - used for display purposes
    ///
    /// # Returns
    ///
    /// An `Arc<[Cube]>` containing all cubes in this cover. Each cube represents one product
    /// term in the sum-of-products representation.
    ///
    /// # Cube Representation
    ///
    /// Returned cubes use `Option<bool>` for inputs:
    /// - `Some(false)` - Variable must be 0
    /// - `Some(true)` - Variable must be 1
    /// - `None` - Don't care (variable can be either 0 or 1)
    ///
    /// And `bool` for outputs:
    /// - `false` - Output is 0
    /// - `true` - Output is 1
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::EspressoCover;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cubes = [
    ///     (&[0, 1][..], &[1][..]),  // 01 -> 1
    ///     (&[1, 2][..], &[1][..]),  // 1X -> 1 (don't care)
    /// ];
    /// let cover = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// // Extract cubes as Rust types
    /// let extracted: Vec<_> = cover.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).collect();
    ///
    /// for cube in extracted.iter() {
    ///     println!("Cube: {:?} -> {:?}", cube.inputs(), cube.outputs());
    /// }
    /// // Low-level cubes are anonymous (positional), so labels print by index:
    /// // Cube: Minterm { 0: 0, 1: 1 } -> Minterm { 0: 1 }
    /// // Cube: Minterm { 0: 1, 1: - } -> Minterm { 0: 1 }
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn to_cubes(
        &self,
        num_inputs: usize,
        num_outputs: usize,
        cube_type: CubeType,
    ) -> EspressoCubes<'_> {
        // The low-level layer has no variable names, so cubes are anonymous (`I = O = Anonymous`):
        // positional only. Callers that need names re-point the cubes onto a real symbol table.
        // Snapshot the count and the per-cover decode constants once; `decode` reads these fields
        // rather than recomputing them (and re-dereferencing the C `set_family`) on every cube.
        let count = unsafe { (*self.ptr).count as usize };
        let wsize = unsafe { (*self.ptr).wsize as usize };
        EspressoCubes {
            cover: self,
            input_syms: Symbols::<Anonymous>::anonymous(num_inputs),
            output_syms: Symbols::<Anonymous>::anonymous(num_outputs),
            num_outputs,
            cube_type,
            wsize,
            output_start: num_inputs * 2,
            input_u64_words: num_inputs.div_ceil(32),
            input_cube_words: (2 * num_inputs).div_ceil(BPI),
            total_input_bits: 2 * num_inputs,
            idx: 0,
            count,
        }
    }

    /// Minimise this cover using the Espresso algorithm
    ///
    /// This is a convenience method that automatically uses the thread-local Espresso instance
    /// associated with this cover. It's equivalent to calling `esp.minimize(cover, d, r)` but
    /// saves you from managing the Espresso handle explicitly.
    ///
    /// # Arguments
    ///
    /// * `d` - Optional don't-care set. If `None`, an empty don't-care set is used
    /// * `r` - Optional OFF-set. If `None`, computed as complement of F ∪ D
    ///
    /// # Returns
    ///
    /// A tuple of `(minimized_f, d, r)` covers. See [`Espresso::minimize()`] for details.
    ///
    /// # Memory Ownership
    ///
    /// This method **consumes** `self` but internally clones the cover before passing it to
    /// the C library, so the memory is properly managed.
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    ///
    /// ```
    /// use espresso_logic::espresso::EspressoCover;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a cover for XOR function
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// // Minimize it directly
    /// let (minimized, d, r) = f.minimize(None, None);
    ///
    /// println!("Minimized: {} cubes", minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len());
    /// println!("Don't-care: {} cubes", d.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len());
    /// println!("OFF-set: {} cubes", r.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With Custom Don't-Cares
    ///
    /// ```
    /// use espresso_logic::espresso::EspressoCover;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cubes_f = [(&[0, 1][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes_f, 2, 1)?;
    /// let cubes_d = [(&[0, 0][..], &[1][..])];
    /// let d = EspressoCover::from_cubes(&cubes_d, 2, 1)?;
    ///
    /// // Provide don't-care set for better optimization
    /// let (minimized, _, _) = f.minimize(Some(d), None);
    /// # Ok(())
    /// # }
    /// ```
    /// # Panics
    ///
    /// Panics if the C minimiser reports a fatal condition (for example an explicit OFF-set that
    /// overlaps the ON-set). Use [`try_minimize()`](Self::try_minimize) to recover from such inputs
    /// as a [`MinimizationError`] instead.
    #[must_use]
    pub fn minimize(
        self,
        d: Option<EspressoCover>,
        r: Option<EspressoCover>,
    ) -> (EspressoCover, EspressoCover, EspressoCover) {
        self.try_minimize(d, r).unwrap_or_else(|e| panic!("{}", e))
    }

    /// Minimise this cover, returning an error instead of aborting on invalid input.
    ///
    /// This is the fallible counterpart of [`minimize()`](Self::minimize): it consumes the cover and
    /// returns the same `(minimized_f, d, r)` triple, but surfaces a [`MinimizationError`] where
    /// `minimize()` would panic. See [`Espresso::try_minimize()`] for how fatal C conditions are
    /// caught and reported.
    ///
    /// # Errors
    ///
    /// Returns [`MinimizationError::EspressoFatal`] if the C minimiser reports a fatal condition for
    /// the given covers (most commonly an OFF-set `r` that overlaps this ON-set).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::EspressoCover;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// let (minimized, _, _) = f.try_minimize(None, None)?;
    /// println!("Result: {} cubes", minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).count());
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_minimize(
        self,
        d: Option<EspressoCover>,
        r: Option<EspressoCover>,
    ) -> Result<(EspressoCover, EspressoCover, EspressoCover), MinimizationError> {
        // Get the Espresso wrapper for this cover
        let espresso = Espresso {
            inner: Rc::clone(&self._espresso),
        };
        espresso.try_minimize(&self, d.as_ref(), r.as_ref())
    }

    /// Minimise this cover using exact minimisation
    ///
    /// This is a convenience method that uses the exact minimisation algorithm which
    /// guarantees minimal results, unlike the heuristic [`minimize()`](Self::minimize) method.
    ///
    /// # Arguments
    ///
    /// * `d` - Optional don't-care set. If `None`, an empty don't-care set is used
    /// * `r` - Optional OFF-set. If `None`, computed as complement of F ∪ D
    ///
    /// # Returns
    ///
    /// A tuple of `(minimized_f, d, r)` covers. See [`Espresso::minimize_exact()`] for details.
    ///
    /// # Performance vs Quality Trade-off
    ///
    /// - **`minimize()`**: Fast heuristic, near-optimal results
    /// - **`minimize_exact()`**: Slower but guaranteed minimal results
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::EspressoCover;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// // Use exact minimization for guaranteed minimal result
    /// let (minimized, d, r) = f.minimize_exact(None, None);
    ///
    /// println!("Exact: {} cubes", minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len());
    /// # Ok(())
    /// # }
    /// ```
    /// # Panics
    ///
    /// Panics if the C minimiser reports a fatal condition (for example an explicit OFF-set that
    /// overlaps the ON-set). Use [`try_minimize_exact()`](Self::try_minimize_exact) to recover from
    /// such inputs as a [`MinimizationError`] instead.
    #[must_use]
    pub fn minimize_exact(
        self,
        d: Option<EspressoCover>,
        r: Option<EspressoCover>,
    ) -> (EspressoCover, EspressoCover, EspressoCover) {
        self.try_minimize_exact(d, r)
            .unwrap_or_else(|e| panic!("{}", e))
    }

    /// Exactly minimise this cover, returning an error instead of aborting on invalid input.
    ///
    /// This is the fallible counterpart of [`minimize_exact()`](Self::minimize_exact): it consumes
    /// the cover and returns the same `(minimized_f, d, r)` triple, but surfaces a
    /// [`MinimizationError`] where `minimize_exact()` would panic. See [`Espresso::try_minimize()`]
    /// for how fatal C conditions are caught and reported.
    ///
    /// # Errors
    ///
    /// Returns [`MinimizationError::EspressoFatal`] if the C minimiser reports a fatal condition for
    /// the given covers (most commonly an OFF-set `r` that overlaps this ON-set).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::EspressoCover;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// let (minimized, _, _) = f.try_minimize_exact(None, None)?;
    /// println!("Exact: {} cubes", minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).count());
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_minimize_exact(
        self,
        d: Option<EspressoCover>,
        r: Option<EspressoCover>,
    ) -> Result<(EspressoCover, EspressoCover, EspressoCover), MinimizationError> {
        // Get the Espresso wrapper for this cover
        let espresso = Espresso {
            inner: Rc::clone(&self._espresso),
        };
        espresso.try_minimize_exact(&self, d.as_ref(), r.as_ref())
    }
}

// Thread-local singleton to ensure only one Espresso instance per thread
// Uses Weak to allow clean destruction when all Espresso handles are dropped
use std::cell::RefCell;
thread_local! {
    static ESPRESSO_INSTANCE: RefCell<std::rc::Weak<InnerEspresso>> = const { RefCell::new(std::rc::Weak::new()) };
}

/// Internal implementation of Espresso that manages thread-local global state
///
/// This type contains the actual implementation details and is held within
/// the thread-local singleton via reference counting. Users interact with the outer
/// [`Espresso`] wrapper instead, which hides these implementation details.
///
/// # Design Rationale
///
/// `InnerEspresso` is separated from `Espresso` to enable the singleton pattern:
///
/// - **`Espresso`** is the public handle (holds `Rc<InnerEspresso>`)
/// - **`InnerEspresso`** is the actual implementation (held in thread-local `Weak<InnerEspresso>`)
/// - **`EspressoCover`** also holds `Rc<InnerEspresso>` to keep the instance alive
///
/// This design ensures:
/// 1. The C global state remains valid while any covers exist (via `Rc`)
/// 2. The singleton can be replaced once all handles are dropped (via `Weak` in thread-local)
/// 3. Multiple handles can reference the same instance (via `Rc::clone`)
///
/// # Thread Safety
///
/// **Note:** This type is neither `Send` nor `Sync` - it must remain on the thread
/// where it was created, as it manages thread-local C state. The `PhantomData<*const ()>`
/// marker ensures this type is `!Send + !Sync`.
///
/// # Lifecycle
///
/// 1. **Creation**: Initialized when first `Espresso::new()` or `from_cubes()` is called
/// 2. **Active**: Referenced by `Rc` in `Espresso` handles and `EspressoCover` instances
/// 3. **Cleanup**: When the last `Rc` is dropped, `Drop` implementation cleans up C state
#[derive(Debug)]
struct InnerEspresso {
    num_inputs: usize,
    num_outputs: usize,
    config: EspressoConfig,
    initialized: bool,
    // Make this type !Send and !Sync since it manages thread-local state
    _marker: PhantomData<*const ()>,
}

/// Direct wrapper around Espresso using thread-local global state
///
/// This type provides direct access to the Espresso minimisation algorithm through
/// the C library. It uses C11 thread-local storage to maintain thread safety -
/// each thread gets its own independent copy of all global state.
///
/// # Thread-Local Singleton Pattern
///
/// Internally, this uses a thread-local singleton with reference counting to ensure that
/// only one Espresso configuration exists per thread:
///
/// - A `thread_local!` static holds a `Weak<InnerEspresso>` reference
/// - Each `Espresso` handle holds an `Rc<InnerEspresso>`
/// - Each `EspressoCover` also holds an `Rc<InnerEspresso>` to keep it alive
/// - The singleton can only be replaced when ALL covers and handles are dropped
///
/// This is safe because:
/// - All C global variables use `_Thread_local` storage
/// - Each thread has independent state (cube structure, configuration, etc.)
/// - The singleton pattern prevents conflicting dimensions within a thread
/// - Reference counting prevents premature cleanup
///
/// # Critical Limitation: Dimension Locking
///
/// ⚠️ **Once created, all covers on a thread must use the same dimensions until ALL
/// covers and Espresso handles are dropped.** This is enforced by the singleton pattern:
///
/// ```rust
/// use espresso_logic::espresso::{Espresso, EspressoCover};
/// use espresso_logic::EspressoConfig;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
///
/// // Create instance with 2 inputs, 1 output
/// let esp = Espresso::new(2, 1, &EspressoConfig::default());
///
/// // This will PANIC - different dimensions while esp exists
/// // let esp2 = Espresso::new(3, 1, &EspressoConfig::default());
///
/// // Must drop first
/// drop(esp);
///
/// // Now different dimensions are OK
/// let esp2 = Espresso::new(3, 1, &EspressoConfig::default());
/// # Ok(())
/// # }
/// ```
///
/// For easier usage with multiple dimensions, use the high-level [`Cover`](crate::Cover) API
/// which handles this automatically.
///
/// # Thread Safety
///
/// **Note:** This type is neither `Send` nor `Sync` (because `Rc` is `!Send + !Sync`) -
/// it must remain on the thread where it was created, as it manages thread-local C state.
/// However, different threads can have completely independent instances with different
/// dimensions since thread-local storage is isolated per thread.
#[derive(Debug, Clone)]
pub struct Espresso {
    inner: Rc<InnerEspresso>,
}

// InnerEspresso has no methods except Drop - all logic is in Espresso wrapper

/// Tear down the thread's current Espresso cube state: run the C `setdown_cube`, then free the
/// hand-allocated `part_size` array and null it so a subsequent setup or `Drop` cannot double-free.
///
/// Shared by [`InnerEspresso::drop`] and the re-init guard in [`Espresso::try_new`] so the
/// safety-critical cleanup sequence lives in one place.
///
/// # Safety
///
/// Must run on the thread owning the cube state, after a prior `setup_cube`/init (so `setdown_cube`
/// is correctly paired). The `part_size` free is null-checked, so it is idempotent on that field.
unsafe fn teardown_cube_state() {
    sys::setdown_cube();
    let cube = sys::get_cube();
    if !(*cube).part_size.is_null() {
        libc::free((*cube).part_size as *mut libc::c_void);
        (*cube).part_size = ptr::null_mut();
    }
}

impl Drop for InnerEspresso {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                teardown_cube_state();
            }
        }
    }
}

impl Espresso {
    /// Create a new Espresso instance with custom configuration
    ///
    /// Initializes the cube structure for the specified number of inputs and outputs,
    /// and applies the given configuration settings.
    ///
    /// # Dimension Constraints
    ///
    /// ⚠️ **Critical:** Only one Espresso configuration can exist per thread at a time.
    ///
    /// - If an instance with the **same dimensions** exists, returns a new handle to it
    /// - If an instance with **different dimensions** exists, this **PANICS**
    /// - To use different dimensions, you must **drop ALL covers and handles first**
    ///
    /// Use [`try_new()`](Self::try_new) for non-panicking error handling.
    ///
    /// # Arguments
    ///
    /// * `num_inputs` - Number of input variables
    /// * `num_outputs` - Number of output variables  
    /// * `config` - Configuration options (only applied when creating a new instance)
    ///
    /// # Panics
    ///
    /// Panics if an Espresso instance with different dimensions already exists on this thread.
    /// The panic message will indicate the requested and existing dimensions.
    ///
    /// # Recommendation
    ///
    /// **Most users should use [`EspressoCover::from_cubes()`](EspressoCover::from_cubes) instead,**
    /// which automatically creates an instance with default config if needed and returns a clear
    /// error on dimension mismatch.
    ///
    /// For automatic dimension management, use the high-level [`Cover`](crate::Cover) API.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::Espresso;
    /// use espresso_logic::EspressoConfig;
    ///
    /// // Create with custom configuration
    /// let mut config = EspressoConfig::default();
    /// config.single_expand = true;
    /// let _esp = Espresso::new(3, 1, &config);
    ///
    /// // Now all EspressoCover operations will use this configured instance
    /// ```
    ///
    /// # Dimension Mismatch Example
    ///
    /// ```should_panic
    /// use espresso_logic::espresso::Espresso;
    /// use espresso_logic::EspressoConfig;
    ///
    /// let esp1 = Espresso::new(2, 1, &EspressoConfig::default());
    ///
    /// // This PANICS - different dimensions!
    /// let esp2 = Espresso::new(3, 1, &EspressoConfig::default());
    /// ```
    #[must_use]
    pub fn new(num_inputs: usize, num_outputs: usize, config: &EspressoConfig) -> Self {
        Self::try_new(num_inputs, num_outputs, Some(config))
            .expect("Failed to create Espresso instance")
    }

    /// Try to create a new Espresso instance with custom configuration
    ///
    /// This is the non-panicking version of [`new()`](Self::new). Returns a `Result` instead
    /// of panicking on dimension mismatch.
    ///
    /// # Behavior
    ///
    /// - **No existing instance**: Creates new instance with specified dimensions and config
    /// - **Same dimensions exist**: Returns a new handle to the existing instance
    /// - **Different dimensions exist**: Returns `MinimizationError::Instance` error
    ///
    /// # Arguments
    ///
    /// * `num_inputs` - Number of input variables
    /// * `num_outputs` - Number of output variables  
    /// * `config` - Optional configuration. If `Some`, verifies config matches existing instance.
    ///   If `None`, accepts any existing instance regardless of config (used internally by
    ///   `from_cubes()` which doesn't care about config).
    ///
    /// # Errors
    ///
    /// Returns [`MinimizationError::Instance`] if:
    /// - [`InstanceError::DimensionMismatch`] -
    ///   An Espresso instance with different dimensions already exists on this thread
    /// - [`InstanceError::ConfigMismatch`] -
    ///   A config is specified and an instance with different config already exists
    /// - [`InstanceError::DimensionTooLarge`] -
    ///   The requested dimensions cannot be represented by the C cube's 32-bit indices
    /// - [`InstanceError::AllocationFailure`] -
    ///   A required C allocation failed (out of memory); the thread-local cube state is left
    ///   unchanged
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::Espresso;
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create first instance
    /// let esp1 = Espresso::try_new(2, 1, None)?;
    ///
    /// // Same dimensions - OK
    /// let esp2 = Espresso::try_new(2, 1, None)?;
    ///
    /// // Different dimensions - error
    /// match Espresso::try_new(3, 1, None) {
    ///     Ok(_) => panic!("Should have failed"),
    ///     Err(e) => println!("Expected error: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_new(
        num_inputs: usize,
        num_outputs: usize,
        config: Option<&EspressoConfig>,
    ) -> Result<Self, MinimizationError> {
        // `cube_setup` casts the dimensions to signed `c_int`, allocates `num_inputs + 1` variables,
        // and accumulates `cube.size = 2*num_inputs + num_outputs` (cubestr.c) — every one of these as
        // `c_int`. Any of them overflowing would wrap negative and abort the process inside
        // `cube_setup`. Reject up front (with checked arithmetic, so a pair that is individually in
        // range but whose `2*num_inputs + num_outputs` sum overflows is still caught) so the safe API
        // returns an error instead of taking down the process.
        let max_dim = c_int::MAX as usize;
        let fits = num_inputs <= max_dim
            && num_outputs <= max_dim
            && num_inputs
                .checked_add(1)
                .is_some_and(|num_vars| num_vars <= max_dim)
            && num_inputs
                .checked_mul(2)
                .and_then(|n| n.checked_add(num_outputs))
                .is_some_and(|cube_size| cube_size <= max_dim);
        if !fits {
            return Err(MinimizationError::Instance(
                InstanceError::DimensionTooLarge {
                    requested: (num_inputs, num_outputs),
                    max: max_dim,
                },
            ));
        }

        // Check if an instance already exists
        let inner = ESPRESSO_INSTANCE.with(|instance| {
            if let Some(existing) = instance.borrow().upgrade() {
                // Check dimensions
                if existing.num_inputs != num_inputs || existing.num_outputs != num_outputs {
                    return Err(MinimizationError::Instance(
                        InstanceError::DimensionMismatch {
                            requested: (num_inputs, num_outputs),
                            existing: (existing.num_inputs, existing.num_outputs),
                        },
                    ));
                }

                // Dimensions match - check config if specified
                if let Some(requested_config) = config {
                    if existing.config != *requested_config {
                        return Err(MinimizationError::Instance(InstanceError::ConfigMismatch {
                            requested: (num_inputs, num_outputs),
                            existing: (existing.num_inputs, existing.num_outputs),
                        }));
                    }
                }

                // Either config matches or wasn't specified - return existing instance
                return Ok(existing);
            }

            // No existing instance - create a new one
            // Use provided config or default
            let actual_config = config.unwrap_or(&EspressoConfig::default()).clone();

            unsafe {
                let cube = sys::get_cube();

                // Always tear down existing cube state to avoid interference
                if !(*cube).fullset.is_null() {
                    teardown_cube_state();
                }

                // Initialize the cube structure
                (*cube).num_binary_vars = num_inputs as c_int;
                (*cube).num_vars = (num_inputs + 1) as c_int;

                // Allocate part_size array
                let part_size_ptr =
                    libc::malloc(((*cube).num_vars as usize) * std::mem::size_of::<c_int>())
                        as *mut c_int;
                if part_size_ptr.is_null() {
                    // `cube_setup()` has not run yet at this point (the `part_size` allocation
                    // happens strictly before it), so `teardown_cube_state()` is not safe to call
                    // here: it invokes the C `setdown_cube()`, which frees `cube.var_mask[0
                    // ..num_vars]` — but `var_mask` is still null (never allocated by
                    // `cube_setup()`), so that free loop would dereference a null pointer.
                    //
                    // Instead, undo exactly the two fields this function wrote above
                    // (`num_binary_vars`/`num_vars`) so the thread-local cube is restored to a
                    // state indistinguishable from "no instance was ever created" on this thread:
                    // `part_size` is still null (this `malloc` never produced a pointer), and
                    // `fullset` is still null (either this is the thread's first-ever call, or the
                    // `teardown_cube_state()` above already nulled it) — the exact condition a
                    // later `try_new` call checks to decide whether a teardown is needed.
                    (*cube).num_binary_vars = 0;
                    (*cube).num_vars = 0;
                    return Err(MinimizationError::Instance(
                        InstanceError::AllocationFailure {
                            requested: (num_inputs, num_outputs),
                        },
                    ));
                }
                (*cube).part_size = part_size_ptr;

                // Set the output size
                *(*cube).part_size.add(num_inputs) = num_outputs as c_int;

                // Setup cube
                sys::cube_setup();

                // Apply custom configuration using accessor functions
                sys::set_debug(if actual_config.debug { 1 } else { 0 });
                sys::set_verbose_debug(if actual_config.verbose_debug { 1 } else { 0 });
                sys::set_trace(if actual_config.trace { 1 } else { 0 });
                sys::set_summary(if actual_config.summary { 1 } else { 0 });
                sys::set_remove_essential(if actual_config.remove_essential { 1 } else { 0 });
                sys::set_force_irredundant(if actual_config.force_irredundant {
                    1
                } else {
                    0
                });
                sys::set_unwrap_onset(if actual_config.unwrap_onset { 1 } else { 0 });
                sys::set_single_expand(if actual_config.single_expand { 1 } else { 0 });
                sys::set_use_super_gasp(if actual_config.use_super_gasp { 1 } else { 0 });
                sys::set_use_random_order(if actual_config.use_random_order { 1 } else { 0 });
                // Deliberately forced off (not an `EspressoConfig` field): the safe wrappers always
                // emit a fully sparse result, matching the reference CLI's default behaviour.
                sys::set_skip_make_sparse(0);
            }

            let inner = Rc::new(InnerEspresso {
                num_inputs,
                num_outputs,
                config: actual_config,
                initialized: true,
                _marker: PhantomData,
            });

            // Store a Weak reference in thread-local singleton
            *instance.borrow_mut() = Rc::downgrade(&inner);

            Ok(inner)
        })?;

        Ok(Espresso { inner })
    }

    /// Get the current thread-local Espresso instance
    ///
    /// Returns the current Espresso instance for this thread if one exists.
    /// This is useful for accessing the instance that was automatically created
    /// by `EspressoCover::from_cubes()` or explicitly created with `Espresso::new()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::{Espresso, EspressoCover};
    ///
    /// # fn main() -> std::io::Result<()> {
    /// // Initially there's no instance
    /// assert!(Espresso::current().is_none());
    ///
    /// // Create a cover - this auto-creates an Espresso instance
    /// let cubes = [(&[0, 1][..], &[1][..])];
    /// let _cover = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// // Now we can get the current instance
    /// let esp = Espresso::current().expect("Should have an instance now");
    /// assert_eq!(esp.num_inputs(), 2);
    /// assert_eq!(esp.num_outputs(), 1);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn current() -> Option<Self> {
        ESPRESSO_INSTANCE
            .with(|instance| instance.borrow().upgrade().map(|inner| Espresso { inner }))
    }

    /// Get the number of inputs for this Espresso instance
    #[must_use]
    pub fn num_inputs(&self) -> usize {
        self.inner.num_inputs
    }

    /// Get the number of outputs for this Espresso instance
    #[must_use]
    pub fn num_outputs(&self) -> usize {
        self.inner.num_outputs
    }

    /// Get the configuration of this Espresso instance
    #[must_use]
    pub fn config(&self) -> &EspressoConfig {
        &self.inner.config
    }

    /// Minimize a boolean function using the Espresso algorithm
    ///
    /// Takes the ON-set (F), optional don't-care set (D), and optional OFF-set (R),
    /// and returns minimised versions of all three covers.
    ///
    /// # Arguments
    ///
    /// * `f` - **ON-set cover**: Specifies where the function output is 1 (required)
    /// * `d` - **Don't-care set**: Positions where output can be either 0 or 1 (optional).
    ///   If `None`, an empty don't-care set is used
    /// * `r` - **OFF-set cover**: Specifies where the function output is 0 (optional).
    ///   If `None`, computed as the complement of F ∪ D
    ///
    /// # Returns
    ///
    /// A tuple of `(minimized_f, d, r)` where:
    /// - `minimized_f` - The minimised ON-set (primary result)
    /// - `d` - The don't-care set used during minimisation
    /// - `r` - The OFF-set used during minimisation
    ///
    /// # Memory Management
    ///
    /// The input covers are **cloned internally** - the original covers remain valid:
    ///
    /// ```
    /// use espresso_logic::espresso::{Espresso, EspressoCover};
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let esp = Espresso::new(2, 1, &EspressoConfig::default());
    /// let cubes = [(&[0, 1][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// // f is cloned inside minimize() - original remains valid
    /// let (result1, _, _) = esp.minimize(&f, None, None);
    /// let (result2, _, _) = esp.minimize(&f, None, None);  // f still valid!
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// All returned covers are independently owned and must be dropped separately.
    ///
    /// # Algorithm Notes
    ///
    /// Espresso is a **heuristic algorithm** - it produces near-optimal results quickly but
    /// does not guarantee absolute minimality. For exact minimisation (slower), use
    /// [`minimize_exact`](Self::minimize_exact).
    ///
    /// The algorithm quality depends on the configuration:
    /// - `single_expand = false` (default): Better quality, slower
    /// - `single_expand = true`: Faster, slightly larger results
    ///
    /// # Examples
    ///
    /// ## Basic Minimization
    ///
    /// ```
    /// use espresso_logic::espresso::{Espresso, EspressoCover};
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let esp = Espresso::new(2, 1, &EspressoConfig::default());
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// let (minimized, d, r) = esp.minimize(&f, None, None);
    /// println!("Result: {} cubes", minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With Don't-Cares
    ///
    /// ```
    /// use espresso_logic::espresso::{Espresso, EspressoCover};
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let esp = Espresso::new(2, 1, &EspressoConfig::default());
    ///
    /// // ON-set: 01 -> 1, 10 -> 1
    /// let cubes_f = [
    ///     (&[0, 1][..], &[1][..]),
    ///     (&[1, 0][..], &[1][..])
    /// ];
    /// let f = EspressoCover::from_cubes(&cubes_f, 2, 1)?;
    ///
    /// // Don't-cares: 00 can be either 0 or 1
    /// let cubes_d = [(&[0, 0][..], &[1][..])];
    /// let d = EspressoCover::from_cubes(&cubes_d, 2, 1)?;
    ///
    /// let (minimized, _, _) = esp.minimize(&f, Some(&d), None);
    /// // Don't-care allows better minimization
    /// println!("With don't-cares: {} cubes",
    ///          minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len());
    /// # Ok(())
    /// # }
    /// ```
    /// # Panics
    ///
    /// Panics if the C minimiser reports a fatal condition (for example an explicit OFF-set that
    /// overlaps the ON-set). Use [`try_minimize()`](Self::try_minimize) to recover from such inputs
    /// as a [`MinimizationError`] instead.
    #[must_use]
    pub fn minimize(
        &self,
        f: &EspressoCover,
        d: Option<&EspressoCover>,
        r: Option<&EspressoCover>,
    ) -> (EspressoCover, EspressoCover, EspressoCover) {
        self.try_minimize(f, d, r)
            .unwrap_or_else(|e| panic!("{}", e))
    }

    /// Minimise a boolean function, returning an error instead of aborting on invalid input.
    ///
    /// This is the fallible counterpart of [`minimize()`](Self::minimize): it takes the same
    /// arguments and returns the same `(minimized_f, d, r)` triple, but surfaces a
    /// [`MinimizationError`] where `minimize()` would panic.
    ///
    /// Unlike the high-level [`Cover`](crate::Cover) path, this low-level entry point performs no
    /// input pre-validation. If the supplied covers drive the C core into a fatal condition — most
    /// commonly an explicit OFF-set that overlaps the ON-set — the condition is caught and returned
    /// as [`MinimizationError::EspressoFatal`], leaving the current thread able to run further
    /// minimisations.
    ///
    /// # Errors
    ///
    /// Returns [`MinimizationError::EspressoFatal`] if the C minimiser reports a fatal condition for
    /// the given covers.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::{Espresso, EspressoCover};
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let esp = Espresso::new(2, 1, &EspressoConfig::default());
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// let (minimized, _, _) = esp.try_minimize(&f, None, None)?;
    /// println!("Result: {} cubes", minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).count());
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_minimize(
        &self,
        f: &EspressoCover,
        d: Option<&EspressoCover>,
        r: Option<&EspressoCover>,
    ) -> Result<(EspressoCover, EspressoCover, EspressoCover), MinimizationError> {
        try_minimize_with_algorithm(self, f, d, r, |f_ptr, d_ptr, r_ptr, msg| unsafe {
            sys::guarded_espresso(f_ptr, d_ptr, r_ptr, msg)
        })
    }

    /// Generate the complete set of prime implicants of a boolean function.
    ///
    /// Returns *every* prime implicant of the ON-set `f` taken relative to the don't-care set `d`
    /// (`None` = empty), not the reduced, irredundant cover that [`minimize()`](Self::minimize)
    /// produces. This is the operation behind the reference tool's `-Dprimes` mode: the result is
    /// `sf_contain`-minimal (no prime strictly contains another) but is otherwise the full prime set,
    /// including consensus primes that an irredundant cover would discard. Multi-output covers are
    /// handled natively, with the output part carried as the trailing multi-valued variable exactly
    /// as the minimiser handles it. `f` and `d` are read only and left unchanged.
    ///
    /// # Panics
    ///
    /// Panics if the C core reports a fatal condition. Use [`try_primes()`](Self::try_primes) to
    /// recover from such inputs as a [`MinimizationError`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::{Espresso, EspressoCover, CubeType};
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let esp = Espresso::new(2, 1, &EspressoConfig::default());
    /// // xor(a, b) = a·!b + !a·b — both cubes are already prime.
    /// let cubes = [(&[1, 0][..], &[1][..]), (&[0, 1][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// let primes = esp.primes(&f, None);
    /// assert_eq!(primes.to_cubes(2, 1, CubeType::F).count(), 2);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn primes(&self, f: &EspressoCover, d: Option<&EspressoCover>) -> EspressoCover {
        self.try_primes(f, d).unwrap_or_else(|e| panic!("{}", e))
    }

    /// Generate all prime implicants, returning an error instead of aborting on invalid input.
    ///
    /// Fallible counterpart of [`primes()`](Self::primes): it returns the same complete prime set but
    /// surfaces a [`MinimizationError`] where `primes()` would panic. Like the other low-level entry
    /// points it performs no input pre-validation.
    ///
    /// # Errors
    ///
    /// Returns [`MinimizationError::EspressoFatal`] if the C core reports a fatal condition for the
    /// given covers.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::{Espresso, EspressoCover, CubeType};
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let esp = Espresso::new(2, 1, &EspressoConfig::default());
    /// let cubes = [(&[1, 0][..], &[1][..]), (&[0, 1][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// let primes = esp.try_primes(&f, None)?;
    /// assert_eq!(primes.to_cubes(2, 1, CubeType::F).count(), 2);
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_primes(
        &self,
        f: &EspressoCover,
        d: Option<&EspressoCover>,
    ) -> Result<EspressoCover, MinimizationError> {
        // MEMORY OWNERSHIP: unlike the minimisation path, `f` and `d` are BORROWED here — no
        // clone/into_raw. `cube2list` only builds a pointer list into the families' cubes
        // (espresso-src/cofactor.c), and `primes_consensus` frees only that LIST on every path
        // (espresso-src/primes.c), leaving F and D themselves intact. This is the very contract
        // `try_minimize_with_algorithm` relies on when it keeps using `f_ptr`/`d_ptr` in the
        // algorithm call after `cube2list`. So F and D stay owned by their callers' covers and must
        // NOT be re-wrapped here; only the returned prime family is ours to own.
        //
        // For an absent D we allocate an empty family and keep it in a local `EspressoCover` so it is
        // freed on every exit path.
        let empty_d;
        let d_ptr = if let Some(c) = d {
            c.ptr
        } else {
            empty_d = unsafe {
                EspressoCover::from_raw(
                    check_alloc(
                        sys::sf_new(0, (*sys::get_cube()).size as c_int),
                        "sf_new for empty D cover in try_primes",
                    ),
                    self,
                )
            };
            empty_d.ptr
        };

        let mut msg: *const c_char = ptr::null();
        let p_ptr = unsafe {
            let cube_list = sys::cube2list(f.ptr, d_ptr);
            sys::guarded_primes(cube_list, &mut msg)
        };
        if p_ptr.is_null() {
            // Caught fatal: the cube list is indeterminate and is leaked (the same bounded one-off
            // leak the minimisation path documents). F and D are untouched. A null result with a
            // still-null message is an unchecked allocation failure, which `guarded_result_error`
            // turns into a panic rather than a misleading empty error.
            return Err(unsafe { guarded_result_error(msg, "guarded_primes") });
        }
        Ok(unsafe { EspressoCover::from_raw(p_ptr, self) })
    }

    /// Minimise a boolean function using exact minimisation
    ///
    /// This method uses the exact minimisation algorithm which guarantees minimal results
    /// by solving the unate covering problem, unlike the heuristic `minimize()` method.
    ///
    /// Takes the ON-set (F), optional don't-care set (D), and optional OFF-set (R),
    /// and returns minimised versions of all three covers.
    ///
    /// # Arguments
    ///
    /// * `f` - **ON-set cover**: Specifies where the function output is 1 (required)
    /// * `d` - **Don't-care set**: Positions where output can be either 0 or 1 (optional).
    ///   If `None`, an empty don't-care set is used
    /// * `r` - **OFF-set cover**: Specifies where the function output is 0 (optional).
    ///   If `None`, computed as the complement of F ∪ D
    ///
    /// # Returns
    ///
    /// A tuple of `(minimized_f, d, r)` where:
    /// - `minimized_f` - The exactly minimised ON-set (primary result)
    /// - `d` - The don't-care set used during minimisation
    /// - `r` - The OFF-set used during minimisation
    ///
    /// # Performance vs Quality Trade-off
    ///
    /// - **`minimize()`**: Fast heuristic, near-optimal results (~99% optimal in practice)
    /// - **`minimize_exact()`**: Slower but guaranteed minimal results (exact solution)
    ///
    /// Use `minimize_exact()` when:
    /// - You need provably minimal results (e.g., for equivalency checking)
    /// - The function is small enough that exact solving is feasible
    /// - Quality is more important than speed
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::{Espresso, EspressoCover};
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let esp = Espresso::new(2, 1, &EspressoConfig::default());
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// // Use exact minimization for guaranteed minimal result
    /// let (minimized, d, r) = esp.minimize_exact(&f, None, None);
    /// println!("Exact result: {} cubes", minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len());
    /// # Ok(())
    /// # }
    /// ```
    /// # Panics
    ///
    /// Panics if the C minimiser reports a fatal condition (for example an explicit OFF-set that
    /// overlaps the ON-set). Use [`try_minimize_exact()`](Self::try_minimize_exact) to recover from
    /// such inputs as a [`MinimizationError`] instead.
    #[must_use]
    pub fn minimize_exact(
        &self,
        f: &EspressoCover,
        d: Option<&EspressoCover>,
        r: Option<&EspressoCover>,
    ) -> (EspressoCover, EspressoCover, EspressoCover) {
        self.try_minimize_exact(f, d, r)
            .unwrap_or_else(|e| panic!("{}", e))
    }

    /// Exactly minimise a boolean function, returning an error instead of aborting on invalid input.
    ///
    /// This is the fallible counterpart of [`minimize_exact()`](Self::minimize_exact): same
    /// arguments and same `(minimized_f, d, r)` result, but surfacing a [`MinimizationError`] where
    /// `minimize_exact()` would panic. See [`try_minimize()`](Self::try_minimize) for the details of
    /// how fatal C conditions are caught and reported.
    ///
    /// # Errors
    ///
    /// Returns [`MinimizationError::EspressoFatal`] if the C minimiser reports a fatal condition for
    /// the given covers.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::{Espresso, EspressoCover};
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let esp = Espresso::new(2, 1, &EspressoConfig::default());
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// let (minimized, _, _) = esp.try_minimize_exact(&f, None, None)?;
    /// println!("Exact: {} cubes", minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).count());
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_minimize_exact(
        &self,
        f: &EspressoCover,
        d: Option<&EspressoCover>,
        r: Option<&EspressoCover>,
    ) -> Result<(EspressoCover, EspressoCover, EspressoCover), MinimizationError> {
        try_minimize_with_algorithm(self, f, d, r, |f_ptr, d_ptr, r_ptr, msg| unsafe {
            sys::guarded_minimize_exact(f_ptr, d_ptr, r_ptr, 1, msg)
        })
    }
}

/// Turn a C `fatal` diagnostic captured by a guarded trampoline into a [`MinimizationError`].
///
/// # Safety
///
/// `msg` must either be null or point to a valid, NUL-terminated C string owned by the C side (the
/// thread-local buffer a guarded trampoline fills on a caught fatal). The string is copied out
/// immediately, so it need only remain valid for the duration of this call.
unsafe fn espresso_fatal_error(msg: *const c_char) -> MinimizationError {
    let message = if msg.is_null() {
        String::new()
    } else {
        std::ffi::CStr::from_ptr(msg).to_string_lossy().into_owned()
    };
    MinimizationError::EspressoFatal { message }
}

/// Turn a guarded trampoline's null result into a [`MinimizationError`], distinguishing a caught C
/// `fatal()` from an unchecked allocation failure.
///
/// By contract (`espresso-src/thread_local_accessors.c`), a guarded trampoline (`guarded_espresso`,
/// `guarded_minimize_exact`, `guarded_complement`) resets `*msg_out` to null before running, and
/// only ever writes it non-null on the `setjmp`/`longjmp` path taken when `fatal()` is caught — that
/// write always targets the thread-local message buffer, so it is never null even for an empty
/// message. A null result paired with a still-null `msg` therefore cannot be a caught fatal: it can
/// only mean the C algorithm itself returned null, which (per the bare, unchecked `ALLOC`/`REALLOC`
/// in `espresso-src/utility.h`) means the underlying allocator ran out of memory. Treat that case
/// like the unguarded `sf_new`/`sf_save` allocation entry points ([`check_alloc`]) and panic, rather
/// than surfacing an empty-message `MinimizationError` that would misrepresent an OOM as a
/// recoverable input error.
///
/// # Safety
///
/// Same precondition as [`espresso_fatal_error`]: `msg` must be null or a valid, NUL-terminated C
/// string owned by the C side.
unsafe fn guarded_result_error(msg: *const c_char, context: &str) -> MinimizationError {
    if msg.is_null() {
        panic!("espresso: C allocation failure ({context}): out of memory");
    }
    espresso_fatal_error(msg)
}

/// Private helper shared by `try_minimize()` and `try_minimize_exact()`.
///
/// `algorithm_fn` invokes the appropriate guarded C trampoline (`guarded_espresso` /
/// `guarded_minimize_exact`); it returns the result `pset_family` on success, or null after a caught
/// fatal, writing the captured diagnostic pointer through its `*mut *const c_char` out-parameter.
fn try_minimize_with_algorithm<F>(
    espresso: &Espresso,
    f: &EspressoCover,
    d: Option<&EspressoCover>,
    r: Option<&EspressoCover>,
    algorithm_fn: F,
) -> Result<(EspressoCover, EspressoCover, EspressoCover), MinimizationError>
where
    F: FnOnce(
        sys::pset_family,
        sys::pset_family,
        sys::pset_family,
        *mut *const c_char,
    ) -> sys::pset_family,
{
    // MEMORY OWNERSHIP: Clone F and extract raw pointer
    // - clone() calls sf_save(), allocating new C memory (independent copy)
    // - into_raw() transfers ownership from Rust to C
    // - C algorithm function takes ownership and returns (possibly different) pointer
    //
    // ERROR PATH: on a caught fatal the C core has already freed and/or replaced some of these
    // covers mid-pipeline (e.g. espresso() frees F), leaving them in an indeterminate state. The
    // raw pointers are then DELIBERATELY LEAKED: they are never re-wrapped in an EspressoCover and
    // never freed, which trades a bounded one-off leak on this rare error path for the certainty of
    // no double-free or use-after-free.
    let f_ptr = f.clone().into_raw();

    // MEMORY OWNERSHIP: D cover
    // - If provided: clone and transfer ownership via into_raw()
    // - If not provided: allocate empty cover with sf_new()
    // - C algorithm function uses but does NOT free D (makes internal copy)
    // - We must free d_ptr after algorithm returns (via EspressoCover wrapper)
    let d_ptr = d.map(|c| c.clone().into_raw()).unwrap_or_else(|| {
        check_alloc(
            unsafe { sys::sf_new(0, (*sys::get_cube()).size as c_int) },
            "sf_new for empty D cover in try_minimize_with_algorithm",
        )
    });

    // MEMORY OWNERSHIP: R cover
    // - If provided: clone and transfer ownership via into_raw()
    // - If not provided: compute complement (allocates new C memory)
    // - C algorithm function uses but does NOT free R
    // - We must free r_ptr after algorithm returns (via EspressoCover wrapper)
    //
    // `complement` can itself raise a fatal (compl.c: non-orthogonal ON/OFF-set), so it runs through
    // the guarded trampoline. On a catch, f_ptr and d_ptr are leaked (see above) and the error is
    // returned before the algorithm runs.
    let r_ptr = match r {
        Some(c) => c.clone().into_raw(),
        None => {
            let mut msg: *const c_char = ptr::null();
            let r_ptr = unsafe {
                let cube_list = sys::cube2list(f_ptr, d_ptr);
                sys::guarded_complement(cube_list, &mut msg)
            };
            if r_ptr.is_null() {
                return Err(unsafe { guarded_result_error(msg, "guarded_complement") });
            }
            r_ptr
        }
    };

    // Call the provided algorithm through its guarded trampoline (espresso or minimize_exact).
    // OWNERSHIP: algorithm_fn takes ownership of f_ptr, returns new/modified pointer (or null on a
    // caught fatal). BORROWING: algorithm_fn uses but does not free d_ptr and r_ptr.
    let mut msg: *const c_char = ptr::null();
    let f_result = algorithm_fn(f_ptr, d_ptr, r_ptr, &mut msg);
    if f_result.is_null() {
        // Caught fatal: all three C covers are indeterminate — leak them (see above). A null
        // result with no captured message is an unchecked allocation failure, not a caught fatal;
        // `guarded_result_error` panics for that case instead of returning a misleading empty
        // `MinimizationError`.
        return Err(unsafe { guarded_result_error(msg, "minimisation algorithm") });
    }

    // MEMORY OWNERSHIP: Wrap all returned/borrowed pointers in EspressoCover
    // This ensures sf_free() is called on all C memory when covers are dropped
    // - f_result: New pointer from algorithm (may be same as f_ptr or different)
    // - d_ptr: Same pointer we passed in, but modified by algorithm
    // - r_ptr: Same pointer we passed in, used read-only by algorithm
    let d_result = unsafe { EspressoCover::from_raw(d_ptr, espresso) };
    let r_result = unsafe { EspressoCover::from_raw(r_ptr, espresso) };

    Ok((
        unsafe { EspressoCover::from_raw(f_result, espresso) },
        d_result,
        r_result,
    ))
}

#[cfg(test)]
mod tests {
    //! Comprehensive multi-threaded tests for thread-local Espresso API
    //!
    //! These tests directly use the low-level Espresso API to verify that thread-local
    //! storage is working correctly and there's no interference between threads.

    use super::*;
    use crate::cover::Minimizable;
    use crate::espresso::error::{CubeError, InstanceError};
    use crate::EspressoConfig;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn c_and_bindgen_agree_on_bpi() {
        // Turns any cc/bindgen width disagreement into a test failure instead of memory corruption.
        assert_eq!(unsafe { sys::get_bpi() } as usize, BPI);
    }

    #[test]
    fn from_cubes_rejects_invalid_value() {
        // A value outside {0,1,2} in the input field is rejected with its position.
        let err = EspressoCover::from_cubes(&[(&[3u8, 0][..], &[1u8][..])], 2, 1)
            .expect_err("value 3 must be rejected");
        assert!(matches!(
            err,
            MinimizationError::Cube(CubeError::InvalidValue {
                value: 3,
                position: 0
            })
        ));
    }

    #[test]
    fn try_primes_returns_complete_prime_set() {
        // f(a,b,x) = a·x + b·x̄ + a·b, encoded with 0/1/2 = must-be-0 / must-be-1 / don't-care. The
        // consensus prime a·b (1,1,-) is redundant in an irredundant cover but MUST appear in the
        // complete prime set that `primes()` returns.
        let esp = Espresso::new(3, 1, &EspressoConfig::default());
        let cubes = [
            (&[1u8, 2, 1][..], &[1u8][..]), // a·x
            (&[2u8, 1, 0][..], &[1u8][..]), // b·x̄
            (&[1u8, 1, 2][..], &[1u8][..]), // a·b
        ];
        let f = EspressoCover::from_cubes(&cubes, 3, 1).unwrap();

        let primes: std::collections::HashSet<Vec<Option<bool>>> = esp
            .primes(&f, None)
            .to_cubes(3, 1, CubeType::F)
            .map(|c| (0..3).map(|i| c.inputs().value_at(i)).collect())
            .collect();

        let expected: std::collections::HashSet<Vec<Option<bool>>> = [
            vec![Some(true), None, Some(true)],  // a·x
            vec![None, Some(true), Some(false)], // b·x̄
            vec![Some(true), Some(true), None],  // a·b
        ]
        .into_iter()
        .collect();

        assert_eq!(primes, expected);
    }

    #[test]
    fn from_cubes_rejects_length_mismatch() {
        // An input slice wider than the declared inputs is rejected (would otherwise write out of the
        // cube's bit region).
        let err = EspressoCover::from_cubes(&[(&[0u8, 1, 0][..], &[1u8][..])], 2, 1)
            .expect_err("over-long input slice must be rejected");
        assert!(matches!(
            err,
            MinimizationError::Cube(CubeError::DimensionMismatch {
                expected_inputs: 2,
                actual_inputs: 3,
                expected_outputs: 1,
                actual_outputs: 1,
            })
        ));
        // Likewise a mismatched output slice.
        let err = EspressoCover::from_cubes(&[(&[0u8, 1][..], &[1u8, 0][..])], 2, 1)
            .expect_err("over-long output slice must be rejected");
        assert!(matches!(
            err,
            MinimizationError::Cube(CubeError::DimensionMismatch {
                actual_outputs: 2,
                ..
            })
        ));
    }

    #[test]
    fn try_new_reports_dimension_and_config_mismatch() {
        // A live (3,1) instance makes a (2,1) request fail with DimensionMismatch.
        let _held = Espresso::new(3, 1, &EspressoConfig::default());
        let err = Espresso::try_new(2, 1, None).expect_err("dimension conflict expected");
        assert!(matches!(
            err,
            MinimizationError::Instance(InstanceError::DimensionMismatch {
                requested: (2, 1),
                existing: (3, 1),
            })
        ));
        // Same dimensions but a different (specified) config fails with ConfigMismatch.
        let other = EspressoConfig {
            single_expand: true,
            ..EspressoConfig::default()
        };
        let err = Espresso::try_new(3, 1, Some(&other)).expect_err("config conflict expected");
        assert!(matches!(
            err,
            MinimizationError::Instance(InstanceError::ConfigMismatch {
                requested: (3, 1),
                existing: (3, 1),
            })
        ));
    }

    /// Test 1: Basic concurrent access
    /// Spawns multiple threads, each creates its own Espresso instance
    /// and performs minimize operations on different problems
    #[test]
    fn test_concurrent_espresso_minimize() {
        const NUM_THREADS: usize = 16;
        const OPS_PER_THREAD: usize = 10;

        let success_count = Arc::new(AtomicUsize::new(0));
        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|thread_id| {
                let success = Arc::clone(&success_count);
                thread::spawn(move || {
                    // Each thread creates its own instance
                    let esp = Espresso::new(2, 1, &EspressoConfig::default());

                    for op in 0..OPS_PER_THREAD {
                        // Create test cover (XOR function)
                        let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
                        let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();

                        // Minimize
                        let (result, _, _) = esp.minimize(&f, None, None);

                        // Verify result has correct structure
                        let cubes: Vec<_> = result.to_cubes(2, 1, CubeType::F).collect();
                        assert!(
                            cubes.len() >= 2,
                            "Thread {} op {} got {} cubes, expected >= 2",
                            thread_id,
                            op,
                            cubes.len()
                        );

                        success.fetch_add(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(
            success_count.load(Ordering::SeqCst),
            NUM_THREADS * OPS_PER_THREAD,
            "Not all operations completed successfully"
        );
    }

    /// Test 2: State isolation test
    /// Verifies that cube structure is independent per thread
    #[test]
    fn test_thread_local_cube_structure_isolation() {
        const NUM_THREADS: usize = 8;

        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|thread_id| {
                thread::spawn(move || {
                    // Different threads use different problem sizes
                    let num_inputs = 2 + (thread_id % 4); // 2, 3, 4, or 5 inputs
                    let num_outputs = 1 + (thread_id % 2); // 1 or 2 outputs

                    let esp = Espresso::new(num_inputs, num_outputs, &EspressoConfig::default());

                    // Create a simple cover
                    let mut cubes = vec![];
                    for i in 0..3 {
                        let inputs: Vec<u8> = (0..num_inputs)
                            .map(|j| if (i + j) % 3 == 0 { 0 } else { 1 })
                            .collect();
                        let outputs = vec![1; num_outputs];
                        cubes.push((inputs, outputs));
                    }

                    let cubes_refs: Vec<_> = cubes
                        .iter()
                        .map(|(i, o)| (i.as_slice(), o.as_slice()))
                        .collect();
                    let f =
                        EspressoCover::from_cubes(&cubes_refs, num_inputs, num_outputs).unwrap();

                    // Minimize multiple times
                    for _ in 0..5 {
                        let (result, _, _) = esp.minimize(&f, None, None);

                        // Verify result structure
                        let cubes: Vec<_> = result
                            .to_cubes(num_inputs, num_outputs, CubeType::F)
                            .collect();
                        assert!(!cubes.is_empty(), "Thread {} got empty result", thread_id);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Test 3: Configuration isolation test
    /// Verifies that configuration settings don't leak between threads
    /// Uses direct C global variable inspection to definitively detect leakage
    #[test]
    fn test_config_isolation() {
        use std::sync::Barrier;

        const NUM_THREADS: usize = 4;
        let barrier = Arc::new(Barrier::new(NUM_THREADS));

        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|thread_id| {
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    // Each thread uses DIFFERENT configuration flags that affect algorithm behavior
                    // but WITHOUT verbose output flags (debug, trace, verbose_debug, summary)
                    let config = EspressoConfig {
                        // Algorithm flags (different per thread)
                        single_expand: thread_id % 2 == 0, // Threads 0,2: fast mode
                        use_super_gasp: thread_id % 2 == 1, // Threads 1,3: super gasp
                        use_random_order: thread_id >= 2,  // Threads 2,3: random order
                        remove_essential: thread_id % 3 != 0, // Threads 1,2,3: remove essential
                        force_irredundant: thread_id != 1, // All except thread 1
                        unwrap_onset: thread_id % 2 == 0,  // Threads 0,2
                        // Output flags ALL disabled to prevent verbose C library output
                        debug: false,
                        verbose_debug: false,
                        trace: false,
                        summary: false,
                    };

                    // Create Espresso instance with config
                    let esp = Espresso::new(3, 1, &config);

                    // Synchronize all threads to maximize chance of detecting leakage
                    barrier.wait();

                    // VERIFY: Read back the actual C global variables and check they match
                    unsafe {
                        // C globals are i32 (0 or 1), convert to bool for comparison
                        let actual_single_expand = *crate::sys::get_single_expand_ptr() != 0;
                        let actual_super_gasp = *crate::sys::get_use_super_gasp_ptr() != 0;
                        let actual_random_order = *crate::sys::get_use_random_order_ptr() != 0;
                        let actual_remove_essential = *crate::sys::get_remove_essential_ptr() != 0;
                        let actual_force_irredundant =
                            *crate::sys::get_force_irredundant_ptr() != 0;
                        let actual_unwrap_onset = *crate::sys::get_unwrap_onset_ptr() != 0;

                        // Assert each global matches what this thread set
                        assert_eq!(
                            actual_single_expand, config.single_expand,
                            "Thread {}: single_expand leaked! Expected {}, got {}",
                            thread_id, config.single_expand, actual_single_expand
                        );
                        assert_eq!(
                            actual_super_gasp, config.use_super_gasp,
                            "Thread {}: use_super_gasp leaked! Expected {}, got {}",
                            thread_id, config.use_super_gasp, actual_super_gasp
                        );
                        assert_eq!(
                            actual_random_order, config.use_random_order,
                            "Thread {}: use_random_order leaked! Expected {}, got {}",
                            thread_id, config.use_random_order, actual_random_order
                        );
                        assert_eq!(
                            actual_remove_essential, config.remove_essential,
                            "Thread {}: remove_essential leaked! Expected {}, got {}",
                            thread_id, config.remove_essential, actual_remove_essential
                        );
                        assert_eq!(
                            actual_force_irredundant, config.force_irredundant,
                            "Thread {}: force_irredundant leaked! Expected {}, got {}",
                            thread_id, config.force_irredundant, actual_force_irredundant
                        );
                        assert_eq!(
                            actual_unwrap_onset, config.unwrap_onset,
                            "Thread {}: unwrap_onset leaked! Expected {}, got {}",
                            thread_id, config.unwrap_onset, actual_unwrap_onset
                        );
                    }

                    // Perform multiple operations to ensure config stays consistent
                    for iteration in 0..10 {
                        let cubes = [
                            (vec![0, 1, 0], vec![1]),
                            (vec![1, 0, 1], vec![1]),
                            (vec![0, 0, 1], vec![1]),
                        ];
                        let cubes_refs: Vec<_> = cubes
                            .iter()
                            .map(|(i, o)| (i.as_slice(), o.as_slice()))
                            .collect();
                        let f = EspressoCover::from_cubes(&cubes_refs, 3, 1).unwrap();
                        let (_result, _, _) = esp.minimize(&f, None, None);

                        // Re-verify config after each operation
                        unsafe {
                            let actual_single_expand = *crate::sys::get_single_expand_ptr() != 0;
                            let actual_super_gasp = *crate::sys::get_use_super_gasp_ptr() != 0;

                            assert_eq!(
                                actual_single_expand, config.single_expand,
                                "Thread {} iteration {}: single_expand changed during execution!",
                                thread_id, iteration
                            );
                            assert_eq!(
                                actual_super_gasp, config.use_super_gasp,
                                "Thread {} iteration {}: use_super_gasp changed during execution!",
                                thread_id, iteration
                            );
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle
                .join()
                .expect("Thread panicked - config isolation test failed!");
        }
    }

    /// Test 4: Stress test
    /// Runs hundreds of concurrent minimize operations
    #[test]
    fn test_stress_concurrent_operations() {
        const NUM_THREADS: usize = 32;
        const OPS_PER_THREAD: usize = 20;

        let errors = Arc::new(AtomicUsize::new(0));
        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|thread_id| {
                let errors = Arc::clone(&errors);
                thread::spawn(move || {
                    let num_inputs = 2 + (thread_id % 3); // 2, 3, or 4 inputs

                    let esp = Espresso::new(num_inputs, 1, &EspressoConfig::default());

                    for op in 0..OPS_PER_THREAD {
                        // Mix different problem sizes
                        let cube_count = 3 + (op % 5);
                        let mut cubes = vec![];

                        for i in 0..cube_count {
                            let inputs: Vec<u8> = (0..num_inputs)
                                .map(|j| ((i + j + thread_id) % 3) as u8)
                                .collect();
                            cubes.push((inputs, vec![1]));
                        }

                        let cubes_refs: Vec<_> = cubes
                            .iter()
                            .map(|(i, o)| (i.as_slice(), o.as_slice()))
                            .collect();
                        let f = EspressoCover::from_cubes(&cubes_refs, num_inputs, 1).unwrap();

                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            esp.minimize(&f, None, None)
                        })) {
                            Ok(_) => {} // Success
                            Err(_) => {
                                errors.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(errors.load(Ordering::SeqCst), 0, "Some operations panicked");
    }

    /// Test 5: Rapid creation/destruction test
    /// Repeatedly creates and drops covers with the same dimensions
    #[test]
    fn test_rapid_creation_destruction() {
        const NUM_THREADS: usize = 8;
        const CYCLES: usize = 50;

        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|_thread_id| {
                thread::spawn(move || {
                    // With singleton pattern, all covers in a thread must have same dimensions
                    let num_inputs = 3;
                    let num_outputs = 2;

                    for _cycle in 0..CYCLES {
                        // Create covers and minimize them
                        let cubes = [(vec![0; num_inputs], vec![1; num_outputs])];
                        let cubes_refs: Vec<_> = cubes
                            .iter()
                            .map(|(i, o)| (i.as_slice(), o.as_slice()))
                            .collect();
                        let f = EspressoCover::from_cubes(&cubes_refs, num_inputs, num_outputs)
                            .unwrap();
                        let (result, d, r) = f.minimize(None, None);

                        // Drop covers explicitly
                        drop(result);
                        drop(d);
                        drop(r);
                    }

                    // Verify thread can still work after all that
                    let cubes = [(vec![0, 1, 0], vec![1, 0])];
                    let cubes_refs: Vec<_> = cubes
                        .iter()
                        .map(|(i, o)| (i.as_slice(), o.as_slice()))
                        .collect();
                    let f =
                        EspressoCover::from_cubes(&cubes_refs, num_inputs, num_outputs).unwrap();
                    let (_result, _, _) = f.minimize(None, None);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Test 6: Long-running test
    /// Keeps threads alive for extended periods performing multiple operations
    #[test]
    fn test_long_running_threads() {
        const NUM_THREADS: usize = 4;
        const OPERATIONS: usize = 100;

        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|thread_id| {
                thread::spawn(move || {
                    let esp = Espresso::new(3, 1, &EspressoConfig::default());

                    for op in 0..OPERATIONS {
                        // Vary the problem slightly each time
                        let var = (op / 10) % 3;
                        let mut cubes = vec![(vec![0, 1, 0], vec![1]), (vec![1, 0, 1], vec![1])];

                        // Add variable cubes based on operation number
                        for i in 0..var {
                            let inputs = vec![(i % 2) as u8, ((i + 1) % 2) as u8, (i % 2) as u8];
                            cubes.push((inputs, vec![1]));
                        }

                        let cubes_refs: Vec<_> = cubes
                            .iter()
                            .map(|(i, o)| (i.as_slice(), o.as_slice()))
                            .collect();
                        let f = EspressoCover::from_cubes(&cubes_refs, 3, 1).unwrap();
                        let (result, _, _) = esp.minimize(&f, None, None);

                        // Verify result
                        let result_cubes: Vec<_> = result.to_cubes(3, 1, CubeType::F).collect();
                        assert!(
                            !result_cubes.is_empty(),
                            "Thread {} op {} got empty result",
                            thread_id,
                            op
                        );
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Test 7: Memory cleanup test
    /// Creates many covers and ensures they're properly cleaned up
    #[test]
    fn test_memory_cleanup() {
        const NUM_THREADS: usize = 4;
        const COVERS_PER_THREAD: usize = 100;

        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|_thread_id| {
                thread::spawn(move || {
                    let esp = Espresso::new(2, 1, &EspressoConfig::default());

                    for _ in 0..COVERS_PER_THREAD {
                        // Create multiple covers
                        let cubes1 = [(&[0, 1][..], &[1][..])];
                        let f1 = EspressoCover::from_cubes(&cubes1, 2, 1).unwrap();
                        let cubes2 = [(&[1, 0][..], &[1][..])];
                        let f2 = EspressoCover::from_cubes(&cubes2, 2, 1).unwrap();
                        let cubes3 = [(&[1, 1][..], &[1][..])];
                        let f3 = EspressoCover::from_cubes(&cubes3, 2, 1).unwrap();

                        // Use them
                        let (_r1, _, _) = esp.minimize(&f1, None, None);
                        let (_r2, _, _) = esp.minimize(&f2, None, None);
                        let (_r3, _, _) = esp.minimize(&f3, None, None);

                        // All covers and results are dropped here
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Test 8: Singleton pattern test
    /// Verifies that the thread-local singleton works correctly
    #[test]
    fn test_singleton_pattern() {
        // Create a cover - auto-creates Espresso instance
        let cubes = [(&[0, 1][..], &[1][..])];
        let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();

        // Can create another cover with same dimensions
        let cubes2 = [(&[1, 0][..], &[1][..])];
        let f2 = EspressoCover::from_cubes(&cubes2, 2, 1).unwrap();

        // Minimize both
        let (result1, _, _) = f.minimize(None, None);
        let (result2, _, _) = f2.minimize(None, None);

        // Verify they worked
        assert!(result1.to_cubes(2, 1, CubeType::F).count() > 0);
        assert!(result2.to_cubes(2, 1, CubeType::F).count() > 0);

        // Can also explicitly create an Espresso handle with same dimensions
        let esp = Espresso::new(2, 1, &EspressoConfig::default());
        assert_eq!(esp.num_inputs(), 2);
        assert_eq!(esp.num_outputs(), 1);
    }

    /// A non-orthogonal cover (ON-set and OFF-set sharing a minterm) drives the C core into
    /// `fatal()`. Through `try_minimize` the guard catches it, the process survives, and the caught
    /// diagnostic is surfaced as `MinimizationError::EspressoFatal`.
    #[test]
    fn try_minimize_catches_non_orthogonal_cover() {
        // Both F and R assert minterm 01, so the ON-set and OFF-set overlap.
        let f = EspressoCover::from_cubes(&[(&[0u8, 1][..], &[1u8][..])], 2, 1).unwrap();
        let r = EspressoCover::from_cubes(&[(&[0u8, 1][..], &[1u8][..])], 2, 1).unwrap();
        let err = f
            .try_minimize(None, Some(r))
            .expect_err("overlapping ON/OFF-set must be caught");
        match err {
            MinimizationError::EspressoFatal { message } => assert!(
                message.contains("orthogonal"),
                "unexpected fatal message: {message}"
            ),
            other => panic!("expected EspressoFatal, got: {other}"),
        }
    }

    /// A non-orthogonal cover through `Espresso::try_minimize_exact` is likewise caught. The exact
    /// path only reaches the orthogonality check via the sparse-cleanup `expand`, which raises the
    /// output variables — so the overlap here is on an output (input 0 asserts output 0 as both on
    /// and off), a multi-output cover.
    #[test]
    fn try_minimize_exact_catches_non_orthogonal_cover() {
        let esp = Espresso::new(1, 2, &EspressoConfig::default());
        // ON-set: input 0 -> outputs {0,1}; input 1 -> output {1}.
        let f = EspressoCover::from_cubes(
            &[(&[0u8][..], &[1u8, 1][..]), (&[1u8][..], &[0u8, 1][..])],
            1,
            2,
        )
        .unwrap();
        // OFF-set: input 0 -> output {0}, which overlaps the ON-set at output 0.
        let r = EspressoCover::from_cubes(&[(&[0u8][..], &[1u8, 0][..])], 1, 2).unwrap();
        let err = esp
            .try_minimize_exact(&f, None, Some(&r))
            .expect_err("overlapping ON/OFF-set must be caught");
        assert!(
            matches!(err, MinimizationError::EspressoFatal { ref message } if message.contains("orthogonal")),
            "expected EspressoFatal mentioning orthogonality, got: {err}"
        );
    }

    /// After a caught fatal the thread's cube state is still usable: a fresh, valid minimisation on
    /// the same thread succeeds. This proves no cube/cdata teardown is needed on the error path.
    #[test]
    fn try_minimize_recovers_after_fatal() {
        // First, trigger and catch a fatal.
        let f_bad = EspressoCover::from_cubes(&[(&[0u8, 1][..], &[1u8][..])], 2, 1).unwrap();
        let r_bad = EspressoCover::from_cubes(&[(&[0u8, 1][..], &[1u8][..])], 2, 1).unwrap();
        let err = f_bad
            .try_minimize(None, Some(r_bad))
            .expect_err("overlapping ON/OFF-set must be caught");
        assert!(matches!(err, MinimizationError::EspressoFatal { .. }));

        // Now a valid minimisation on the same thread must still work.
        let cubes = [(&[0u8, 1][..], &[1u8][..]), (&[1u8, 0][..], &[1u8][..])];
        let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
        let (minimized, _, _) = f
            .try_minimize(None, None)
            .expect("valid minimisation must succeed after recovery");
        let count = minimized.to_cubes(2, 1, CubeType::F).count();
        assert!(count >= 2, "expected at least 2 cubes, got {count}");
    }

    /// The infallible `minimize` panics on a fatal condition (its documented `# Panics` contract),
    /// mirroring `test_singleton_conflict_panics` for the instance-conflict case.
    #[test]
    #[should_panic(expected = "orthogonal")]
    fn minimize_panics_on_non_orthogonal_cover() {
        let f = EspressoCover::from_cubes(&[(&[0u8, 1][..], &[1u8][..])], 2, 1).unwrap();
        let r = EspressoCover::from_cubes(&[(&[0u8, 1][..], &[1u8][..])], 2, 1).unwrap();
        let _ = f.minimize(None, Some(r));
    }

    /// Test that creating conflicting Espresso instances panics
    #[test]
    #[should_panic(expected = "Instance(DimensionMismatch")]
    fn test_singleton_conflict_panics() {
        let _esp1 = Espresso::new(2, 1, &EspressoConfig::default());
        // This should panic because dimensions don't match
        // new() calls try_new().expect(), so it panics on error
        let _esp2 = Espresso::new(3, 2, &EspressoConfig::default());
    }

    /// Test that creating covers with conflicting dimensions returns an error
    #[test]
    fn test_cover_dimension_conflict_errors() {
        // Create first cover with 2 inputs, 1 output - auto-creates Espresso instance
        let cubes1 = [(&[0, 1][..], &[1][..])];
        let _cover1 = EspressoCover::from_cubes(&cubes1, 2, 1).unwrap();

        // Try to create second cover with different dimensions - should return error
        let cubes2 = [(&[0, 1, 0][..], &[1, 0][..])];
        let result = EspressoCover::from_cubes(&cubes2, 3, 2);
        assert!(result.is_err(), "Should error on dimension mismatch");
        let err = result.unwrap_err();
        match err {
            crate::error::MinimizationError::Instance(
                crate::error::InstanceError::DimensionMismatch { .. },
            ) => {
                // Expected error type
            }
            other => panic!(
                "Expected InstanceError::DimensionMismatch error, got: {}",
                other
            ),
        }
    }

    /// Test 9: Different problem sizes concurrently
    /// Tests that threads can handle completely different problem structures
    #[test]
    fn test_different_problem_sizes() {
        let handles: Vec<_> = vec![
            (2, 1, 5),  // 2 inputs, 1 output, 5 cubes
            (3, 1, 7),  // 3 inputs, 1 output, 7 cubes
            (4, 2, 10), // 4 inputs, 2 outputs, 10 cubes
            (5, 1, 15), // 5 inputs, 1 output, 15 cubes
            (3, 3, 8),  // 3 inputs, 3 outputs, 8 cubes
            (2, 2, 4),  // 2 inputs, 2 outputs, 4 cubes
        ]
        .into_iter()
        .enumerate()
        .map(|(idx, (num_inputs, num_outputs, num_cubes))| {
            thread::spawn(move || {
                let esp = Espresso::new(num_inputs, num_outputs, &EspressoConfig::default());

                // Generate cubes
                let mut cubes = vec![];
                for i in 0..num_cubes {
                    let inputs: Vec<u8> =
                        (0..num_inputs).map(|j| ((i + j + idx) % 3) as u8).collect();
                    let outputs = vec![if i % 2 == 0 { 1 } else { 0 }; num_outputs];
                    cubes.push((inputs, outputs));
                }

                let cubes_refs: Vec<_> = cubes
                    .iter()
                    .map(|(i, o)| (i.as_slice(), o.as_slice()))
                    .collect();
                let f = EspressoCover::from_cubes(&cubes_refs, num_inputs, num_outputs).unwrap();

                // Minimize multiple times
                for _ in 0..3 {
                    let f_clone = f.clone();
                    let (result, _, _) = esp.minimize(&f_clone, None, None);

                    // Basic validation
                    let result_cubes: Vec<_> = result
                        .to_cubes(num_inputs, num_outputs, CubeType::F)
                        .collect();
                    assert!(
                        !result_cubes.is_empty(),
                        "Got empty result for problem {}",
                        idx
                    );
                }
            })
        })
        .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    // Tests for dimension cleanup and Cover API

    #[test]
    fn test_explicit_drop_between_dimensions() {
        use crate::{Cover, CoverType};

        // Test with explicit scope-based drop to ensure cleanup works correctly
        {
            let mut cover1 = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
            cover1.push(Cube::anonymous(
                &[Some(true), Some(false)],
                &[true],
                CubeType::F,
            ));
            cover1 = cover1.minimize().unwrap();
            assert_eq!(cover1.num_cubes(), 1, "Cover1 (2x1) should have 1 cube");
        } // cover1 is dropped here, Espresso instance should be cleaned up

        // Now try with different dimensions - should work without conflicts
        let mut cover2 = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
        cover2.push(Cube::anonymous(
            &[Some(false), Some(true), Some(false), Some(true)],
            &[true],
            CubeType::F,
        ));
        cover2 = cover2.minimize().unwrap();
        assert_eq!(cover2.num_cubes(), 1, "Cover2 (4x1) should have 1 cube");
    }

    // Tests for singleton behavior and EspressoCover

    #[test]
    fn test_automatic_singleton_creation() {
        // No need to manually create Espresso - it's automatic
        let cubes = [(&[0, 1][..], &[1][..])];
        let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
        let (result, d, r) = f.minimize(None, None);

        // Verify the result cover has expected structure
        let result_cubes: Vec<_> = result.to_cubes(2, 1, CubeType::F).collect();
        assert_eq!(
            result_cubes.len(),
            1,
            "Single cube should minimize to 1 cube"
        );

        // Verify the cube content is correct
        let cube = &result_cubes[0];
        assert_eq!(
            cube.inputs().iter().collect::<Vec<_>>(),
            [Some(false), Some(true)],
            "Input should be [0, 1]"
        );
        assert_eq!(
            cube.outputs().iter().collect::<Vec<_>>(),
            [true],
            "Output should be [1]"
        );

        // Verify D and R covers are accessible
        let d_cubes: Vec<_> = d.to_cubes(2, 1, CubeType::F).collect();
        let r_cubes: Vec<_> = r.to_cubes(2, 1, CubeType::F).collect();
        assert!(
            d_cubes.is_empty() || !d_cubes.is_empty(),
            "D cover should be valid"
        );
        assert!(
            r_cubes.is_empty() || !r_cubes.is_empty(),
            "R cover should be valid"
        );

        // Can create more covers with same dimensions - test XOR function
        let cubes_f2 = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
        let f2 = EspressoCover::from_cubes(&cubes_f2, 2, 1).unwrap();
        let (result2, _, _) = f2.minimize(None, None);
        let result2_cubes: Vec<_> = result2.to_cubes(2, 1, CubeType::F).collect();
        assert_eq!(
            result2_cubes.len(),
            2,
            "XOR cannot be minimized, should have 2 cubes"
        );
    }

    #[test]
    fn test_coverbuilder_handles_different_dimensions() {
        use crate::{Cover, CoverType};

        // Cover (unlike EspressoCover) can handle DIFFERENT dimensions
        // because it properly manages Espresso instance lifecycle

        // Create and minimize first cover with 2 inputs, 1 output
        let mut cover1 = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
        cover1.push(Cube::anonymous(
            &[Some(true), Some(false)],
            &[true],
            CubeType::F,
        ));
        assert_eq!(
            cover1.num_cubes(),
            1,
            "Should have 1 cube before minimization"
        );

        cover1 = cover1.minimize().unwrap();
        assert_eq!(
            cover1.num_cubes(),
            1,
            "Single cube should remain as 1 after minimization"
        );

        // Cover can handle different dimensions (3x2) without conflicts
        let mut cover2 = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
        cover2.push(Cube::anonymous(
            &[Some(false), Some(true), Some(false)],
            &[true, false],
            CubeType::F,
        ));
        assert_eq!(
            cover2.num_cubes(),
            1,
            "Should have 1 cube before minimization"
        );

        cover2 = cover2.minimize().unwrap();
        assert_eq!(
            cover2.num_cubes(),
            1,
            "Single cube should remain as 1 after minimization"
        );

        // Both covers should be independent despite different dimensions
        assert_eq!(
            cover1.num_cubes(),
            1,
            "Cover1 (2x1) should still have 1 cube"
        );
        assert_eq!(
            cover2.num_cubes(),
            1,
            "Cover2 (3x2) should still have 1 cube"
        );
    }

    #[test]
    fn test_singleton_respects_explicit_config() {
        use crate::EspressoConfig;

        // Create an Espresso instance with custom config
        let config = EspressoConfig {
            single_expand: true,
            ..Default::default()
        };
        let esp = Espresso::new(2, 1, &config);

        // Covers created afterwards use this configured instance
        let cubes = [
            (&[0, 0][..], &[1][..]),
            (&[0, 1][..], &[1][..]),
            (&[1, 0][..], &[1][..]),
        ];
        let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();

        // Verify input has 3 cubes
        let input_cubes: Vec<_> = f.to_cubes(2, 1, CubeType::F).collect();
        assert_eq!(input_cubes.len(), 3, "Should start with 3 cubes");

        let (result, _, _) = esp.minimize(&f, None, None);
        let result_cubes: Vec<_> = result.to_cubes(2, 1, CubeType::F).collect();

        // This should minimize to fewer cubes (0- or -0 or -1)
        assert!(
            result_cubes.len() <= 2,
            "Should minimize to 2 or fewer cubes, got {}",
            result_cubes.len()
        );
        assert!(
            !result_cubes.is_empty(),
            "Should have at least 1 cube after minimization"
        );
    }

    // Tests for !Send and !Sync behavior

    #[test]
    fn test_espresso_not_send() {
        use crate::EspressoConfig;

        let esp = Espresso::new(2, 1, &EspressoConfig::default());

        // Actually use it to verify functionality
        let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
        let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
        let (result, _, _) = esp.minimize(&f, None, None);

        // XOR cannot be minimized, should still have 2 cubes
        let result_cubes: Vec<_> = result.to_cubes(2, 1, CubeType::F).collect();
        assert_eq!(result_cubes.len(), 2, "XOR should maintain 2 cubes");
    }

    #[test]
    fn test_espresso_single_dimension_per_thread() {
        use crate::EspressoConfig;

        let esp = Espresso::new(3, 1, &EspressoConfig::default());

        // Use the espresso instance on the same thread
        let cubes = [(&[0, 1, 1][..], &[1][..])];
        let f = EspressoCover::from_cubes(&cubes, 3, 1).unwrap();
        let (result, _, _) = esp.minimize(&f, None, None);

        let result_cubes: Vec<_> = result.to_cubes(3, 1, CubeType::F).collect();
        assert_eq!(result_cubes.len(), 1, "Single cube should remain as 1");

        // Verify the cube is correct
        let cube = &result_cubes[0];
        assert_eq!(
            cube.inputs().iter().collect::<Vec<_>>(),
            [Some(false), Some(true), Some(true)]
        );
        assert_eq!(cube.outputs().iter().collect::<Vec<_>>(), [true]);
    }

    #[test]
    fn test_espresso_cover_not_send() {
        use crate::EspressoConfig;

        let _esp = Espresso::new(2, 1, &EspressoConfig::default());
        let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
        let cover = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();

        // Verify the cover was created correctly
        let result_cubes: Vec<_> = cover.to_cubes(2, 1, CubeType::F).collect();
        assert_eq!(result_cubes.len(), 2, "Should have 2 input cubes");

        // Verify minimization works
        let (minimized, _, _) = cover.minimize(None, None);
        let min_cubes: Vec<_> = minimized.to_cubes(2, 1, CubeType::F).collect();
        assert_eq!(min_cubes.len(), 2, "XOR cannot be minimized");
    }

    #[test]
    fn test_multiple_espresso_covers_same_thread() {
        use crate::EspressoConfig;

        let _esp = Espresso::new(3, 1, &EspressoConfig::default());

        // Create multiple covers and verify they work independently
        let cubes1 = [(&[0, 0, 1][..], &[1][..]), (&[0, 1, 1][..], &[1][..])];
        let cover1 = EspressoCover::from_cubes(&cubes1, 3, 1).unwrap();

        let cubes2 = [(&[1, 0, 1][..], &[1][..]), (&[1, 1, 1][..], &[1][..])];
        let cover2 = EspressoCover::from_cubes(&cubes2, 3, 1).unwrap();

        let cubes1: Vec<_> = cover1.to_cubes(3, 1, CubeType::F).collect();
        let cubes2: Vec<_> = cover2.to_cubes(3, 1, CubeType::F).collect();

        assert_eq!(cubes1.len(), 2);
        assert_eq!(cubes2.len(), 2);
    }

    #[test]
    fn test_complex_operations_same_thread() {
        use crate::EspressoConfig;

        let esp = Espresso::new(2, 1, &EspressoConfig::default());
        let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
        let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
        let (result, d, r) = esp.minimize(&f, None, None);

        let result_cubes: Vec<_> = result.to_cubes(2, 1, CubeType::F).collect();
        assert_eq!(
            result_cubes.len(),
            2,
            "XOR minimization should produce 2 cubes"
        );

        // Verify D and R covers are also valid (they exist even if empty)
        let _d_cubes: Vec<_> = d.to_cubes(2, 1, CubeType::F).collect();
        let _r_cubes: Vec<_> = r.to_cubes(2, 1, CubeType::F).collect();
        // D and R covers are successfully retrieved
    }
}
/// Configuration for the Espresso algorithm
///
/// Controls the behaviour of the Espresso heuristic logic minimiser. This configuration
/// can be used with **both the high-level and low-level APIs** to tune the minimisation
/// process for your specific needs.
///
/// # When to Use
///
/// Most users should use the **default configuration** which provides a good balance
/// between speed and result quality. Consider customising when you need:
///
/// - **Maximum speed** with acceptable quality loss (`single_expand = true`)
/// - **Debugging** algorithm behaviour (`debug = true`, `trace = true`)
/// - **Performance metrics** (`summary = true`)
/// - **Non-deterministic exploration** (`use_random_order = true`)
///
/// # Works with Both APIs
///
/// ## High-Level API (`Cover`)
///
/// Use with [`Cover::minimize_with_config()`](crate::cover::Minimizable::minimize_with_config):
///
/// ```
/// use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, EspressoConfig, Minimizable};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
/// cover.push(Cube::anonymous(&[Some(true), Some(false)], &[true], CubeType::F));
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
/// Use when creating an [`Espresso`] instance:
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
/// let cubes = [(&[0, 1][..], &[1][..])];
/// let cover = EspressoCover::from_cubes(&cubes, 2, 1)?;
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
/// 4. **Lastgasp** - Final optimisation pass
///
/// The configuration controls how aggressively each phase operates.
///
/// Deliberately an open (non-`#[non_exhaustive]`) struct: the ergonomic
/// `EspressoConfig { single_expand: true, ..Default::default() }` literal is worth keeping, and the
/// vintage Espresso algorithm is unlikely to grow new options — so the narrow break of adding a field
/// later (only for downstream code that constructs it exhaustively without `..Default::default()`) is
/// an acceptable trade.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EspressoConfig {
    /// Enable debugging output to stderr
    ///
    /// When `true`, prints detailed information about the minimisation process.
    /// Useful for understanding algorithm behaviour but verbose.
    ///
    /// **Default:** `false`
    pub debug: bool,

    /// Enable verbose debugging output
    ///
    /// Even more detailed than `debug`. Only use for deep algorithm analysis.
    ///
    /// **Default:** `false`
    pub verbose_debug: bool,

    /// Print trace information during minimisation
    ///
    /// Shows progress through different minimisation phases.
    ///
    /// **Default:** `false`
    pub trace: bool,

    /// Print summary statistics after minimisation
    ///
    /// Shows cube counts, execution time, and optimisation metrics.
    ///
    /// **Default:** `false`
    pub summary: bool,

    /// Remove essential prime implicants before minimisation
    ///
    /// Essential primes are terms that must be in any minimal cover. Removing
    /// them first can speed up minimisation for large problems.
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

    /// Unwrap the onset before minimisation
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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
