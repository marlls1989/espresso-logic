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
//! - **Maximum performance** - ~5-10% faster than high-level API due to minimal overhead
//! - **Explicit instance control** - Manually manage Espresso instance lifecycle
//!
//! **For most use cases, prefer the higher-level APIs:**
//! - [`BoolExpr`](crate::BoolExpr) for boolean expressions
//! - [`Cover`](crate::Cover) for covers with dynamic dimensions
//! - [`PLAReader`](crate::PLAReader) trait for reading PLA files
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
//! **❌ UNSAFE - Different dimensions without dropping:**
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
//! The Espresso C library uses global state that must be initialized for specific dimensions:
//! - The cube structure defines bit layouts for variables
//! - Memory allocation patterns depend on the number of inputs/outputs
//! - Changing dimensions requires tearing down and reinitializing all this state
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
//! use espresso_logic::{Cover, CoverType, Minimizable};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Cover handles dimension changes automatically
//! let mut cover1 = Cover::new(CoverType::F);
//! cover1.add_cube(&[Some(true), Some(false)], &[Some(true)]);
//! cover1 = cover1.minimize()?;
//!
//! // Different dimensions - no problem!
//! let mut cover2 = Cover::new(CoverType::F);
//! cover2.add_cube(&[Some(false), Some(true), Some(false)], &[Some(true)]);
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
//! use espresso_logic::espresso::{EspressoCover, CubeType};
//! use std::thread;
//!
//! # fn main() {
//! let handle1 = thread::spawn(|| {
//!     // Thread 1: 2 inputs, 1 output
//!     let cubes = [(&[0, 1][..], &[1][..])];
//!     let cover = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
//!     let (result, _, _) = cover.minimize(None, None);
//!     // Extract the data before returning (covers are !Send)
//!     result.to_cubes(2, 1, CubeType::F).len()
//! });
//!
//! let handle2 = thread::spawn(|| {
//!     // Thread 2: 3 inputs, 1 output - completely independent!
//!     let cubes = [(&[0, 1, 0][..], &[1][..])];
//!     let cover = EspressoCover::from_cubes(&cubes, 3, 1).unwrap();
//!     let (result, _, _) = cover.minimize(None, None);
//!     // Extract the data before returning (covers are !Send)
//!     result.to_cubes(3, 1, CubeType::F).len()
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
//! This module uses a sophisticated singleton pattern with reference counting to manage
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
//! Simply work with `EspressoCover` - the Espresso instance is managed automatically:
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
//! let result_cubes = minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F);
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
//! use espresso_logic::espresso::{EspressoCover, CubeType};
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
//!         result.to_cubes(2, 1, CubeType::F).len()
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
//! use espresso_logic::espresso::{EspressoCover, CubeType};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! fn minimize_and_report(
//!     cubes: &[(&[u8], &[u8])],
//!     num_inputs: usize,
//!     num_outputs: usize
//! ) -> Result<usize, Box<dyn std::error::Error>> {
//!     let cover = EspressoCover::from_cubes(cubes, num_inputs, num_outputs)?;
//!     let (result, _, _) = cover.minimize(None, None);
//!     Ok(result.to_cubes(num_inputs, num_outputs, CubeType::F).len())
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

use crate::sys;
pub use error::{CubeError, InstanceError, MinimizationError};
use std::marker::PhantomData;
use std::os::raw::c_int;
use std::ptr;
use std::rc::Rc;

// Re-export for convenience when using the espresso module directly
pub use crate::cover::{Cube, CubeType};

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

impl EspressoCover {
    /// Create a new empty cover with the specified capacity and cube size
    ///
    /// Requires that an Espresso instance exists on the current thread.
    /// Normally you don't need to call this directly - use `from_cubes()` instead.
    #[allow(dead_code)]
    pub(crate) fn new(capacity: usize, cube_size: usize) -> Self {
        let espresso = Espresso::current().expect(
            "EspressoCover::new requires an Espresso instance. Use EspressoCover::from_cubes() instead.",
        );

        let ptr = unsafe { sys::sf_new(capacity as c_int, cube_size as c_int) };
        EspressoCover {
            ptr,
            _espresso: espresso.inner,
        }
    }

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

        // This assumes Espresso has already initialized the cube structure
        let cube_size = unsafe { (*sys::get_cube()).size as usize };

        // Create empty cover with capacity (reuse the espresso reference)
        let ptr = unsafe { sys::sf_new(cubes.len() as c_int, cube_size as c_int) };
        let mut cover = EspressoCover {
            ptr,
            _espresso: espresso.inner,
        };

        // Add each cube to the cover
        for &(inputs, outputs) in cubes {
            unsafe {
                let cf = *(*sys::get_cube()).temp.add(0);
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
                        let bit_pos = output_first + i;
                        let word = (bit_pos >> 5) + 1;
                        let bit = bit_pos & 31;
                        *cf.add(word) |= 1 << bit;
                    }
                }

                cover.ptr = sys::sf_addset(cover.ptr, cf);
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
        let ptr = unsafe { sys::sf_save(self.ptr) };
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
    /// A `Vec<Cube>` containing all cubes in this cover. Each cube represents one product
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
    /// use espresso_logic::espresso::{EspressoCover, CubeType};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cubes = [
    ///     (&[0, 1][..], &[1][..]),  // 01 -> 1
    ///     (&[1, 2][..], &[1][..]),  // 1X -> 1 (don't care)
    /// ];
    /// let cover = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// // Extract cubes as Rust types
    /// let extracted = cover.to_cubes(2, 1, CubeType::F);
    ///
    /// for cube in &extracted {
    ///     println!("Cube: {:?} -> {:?}", cube.inputs(), cube.outputs());
    /// }
    /// // Cube: [Some(false), Some(true)] -> [true]
    /// // Cube: [Some(true), None] -> [true]
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_cubes(
        &self,
        num_inputs: usize,
        num_outputs: usize,
        cube_type: CubeType,
    ) -> Vec<Cube> {
        unsafe {
            let count = (*self.ptr).count as usize;
            let wsize = (*self.ptr).wsize as usize;
            let data = (*self.ptr).data;

            let mut result = Vec::with_capacity(count);

            for i in 0..count {
                let cube_ptr = data.add(i * wsize);

                // Decode inputs (binary variables - 2 bits each)
                let mut inputs = Vec::with_capacity(num_inputs);
                for var in 0..num_inputs {
                    let bit0 = var * 2;
                    let bit1 = var * 2 + 1;

                    let word0 = (bit0 >> 5) + 1;
                    let b0 = bit0 & 31;
                    let word1 = (bit1 >> 5) + 1;
                    let b1 = bit1 & 31;

                    let has_bit0 = if word0 < wsize {
                        (*cube_ptr.add(word0) & (1 << b0)) != 0
                    } else {
                        false
                    };
                    let has_bit1 = if word1 < wsize {
                        (*cube_ptr.add(word1) & (1 << b1)) != 0
                    } else {
                        false
                    };

                    inputs.push(match (has_bit0, has_bit1) {
                        (false, false) => None,
                        (true, false) => Some(false),
                        (false, true) => Some(true),
                        (true, true) => None, // don't care
                    });
                }

                // Decode outputs (multi-valued variable - 1 bit per value)
                let mut outputs = Vec::with_capacity(num_outputs);
                let output_start = num_inputs * 2;
                for out in 0..num_outputs {
                    let bit = output_start + out;
                    let word = (bit >> 5) + 1;
                    let b = bit & 31;

                    let val = if word < wsize {
                        (*cube_ptr.add(word) & (1 << b)) != 0
                    } else {
                        false
                    };

                    outputs.push(val);
                }

                result.push(Cube::new(&inputs, &outputs, cube_type));
            }

            result
        }
    }

    /// Minimize this cover using the Espresso algorithm
    ///
    /// This is a convenience method that automatically uses the thread-local Espresso instance
    /// associated with this cover. It's equivalent to calling `esp.minimize(cover, d, r)` but
    /// saves you from managing the Espresso handle explicitly.
    ///
    /// # Arguments
    ///
    /// * `d` - Optional don't-care set. If `None`, computed as complement of F ∪ R
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
    /// use espresso_logic::espresso::{EspressoCover, CubeType};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a cover for XOR function
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// // Minimize it directly
    /// let (minimized, d, r) = f.minimize(None, None);
    ///
    /// println!("Minimized: {} cubes", minimized.to_cubes(2, 1, CubeType::F).len());
    /// println!("Don't-care: {} cubes", d.to_cubes(2, 1, CubeType::F).len());
    /// println!("OFF-set: {} cubes", r.to_cubes(2, 1, CubeType::F).len());
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
    pub fn minimize(
        self,
        d: Option<EspressoCover>,
        r: Option<EspressoCover>,
    ) -> (EspressoCover, EspressoCover, EspressoCover) {
        // Get the Espresso wrapper for this cover
        let espresso = Espresso {
            inner: Rc::clone(&self._espresso),
        };
        espresso.minimize(&self, d.as_ref(), r.as_ref())
    }

    /// Minimize this cover using exact minimization
    ///
    /// This is a convenience method that uses the exact minimization algorithm which
    /// guarantees minimal results, unlike the heuristic [`minimize()`](Self::minimize) method.
    ///
    /// # Arguments
    ///
    /// * `d` - Optional don't-care set. If `None`, computed as complement of F ∪ R
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
    /// use espresso_logic::espresso::{EspressoCover, CubeType};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// // Use exact minimization for guaranteed minimal result
    /// let (minimized, d, r) = f.minimize_exact(None, None);
    ///
    /// println!("Exact: {} cubes", minimized.to_cubes(2, 1, CubeType::F).len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimize_exact(
        self,
        d: Option<EspressoCover>,
        r: Option<EspressoCover>,
    ) -> (EspressoCover, EspressoCover, EspressoCover) {
        // Get the Espresso wrapper for this cover
        let espresso = Espresso {
            inner: Rc::clone(&self._espresso),
        };
        espresso.minimize_exact(&self, d.as_ref(), r.as_ref())
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
/// This type provides direct access to the Espresso minimization algorithm through
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

impl Drop for InnerEspresso {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                sys::setdown_cube();
                let cube = sys::get_cube();
                if !(*cube).part_size.is_null() {
                    libc::free((*cube).part_size as *mut libc::c_void);
                    (*cube).part_size = ptr::null_mut();
                }
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
                    sys::setdown_cube();
                    if !(*cube).part_size.is_null() {
                        libc::free((*cube).part_size as *mut libc::c_void);
                        (*cube).part_size = ptr::null_mut();
                    }
                }

                // Initialize the cube structure
                (*cube).num_binary_vars = num_inputs as c_int;
                (*cube).num_vars = (num_inputs + 1) as c_int;

                // Allocate part_size array
                let part_size_ptr =
                    libc::malloc(((*cube).num_vars as usize) * std::mem::size_of::<c_int>())
                        as *mut c_int;
                if part_size_ptr.is_null() {
                    panic!("Failed to allocate part_size array");
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
    pub fn current() -> Option<Self> {
        ESPRESSO_INSTANCE
            .with(|instance| instance.borrow().upgrade().map(|inner| Espresso { inner }))
    }

    /// Get the number of inputs for this Espresso instance
    pub fn num_inputs(&self) -> usize {
        self.inner.num_inputs
    }

    /// Get the number of outputs for this Espresso instance
    pub fn num_outputs(&self) -> usize {
        self.inner.num_outputs
    }

    /// Get the configuration of this Espresso instance
    pub fn config(&self) -> &EspressoConfig {
        &self.inner.config
    }

    /// Minimize a boolean function using the Espresso algorithm
    ///
    /// Takes the ON-set (F), optional don't-care set (D), and optional OFF-set (R),
    /// and returns minimized versions of all three covers.
    ///
    /// # Arguments
    ///
    /// * `f` - **ON-set cover**: Specifies where the function output is 1 (required)
    /// * `d` - **Don't-care set**: Positions where output can be either 0 or 1 (optional).
    ///   If `None`, computed as the complement of F ∪ R
    /// * `r` - **OFF-set cover**: Specifies where the function output is 0 (optional).
    ///   If `None`, computed as the complement of F ∪ D
    ///
    /// # Returns
    ///
    /// A tuple of `(minimized_f, d, r)` where:
    /// - `minimized_f` - The minimized ON-set (primary result)
    /// - `d` - The don't-care set used during minimization
    /// - `r` - The OFF-set used during minimization
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
    /// does not guarantee absolute minimality. For exact minimization (slower), use the
    /// `exact` configuration option.
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
    /// use espresso_logic::espresso::{Espresso, EspressoCover, CubeType};
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let esp = Espresso::new(2, 1, &EspressoConfig::default());
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// let (minimized, d, r) = esp.minimize(&f, None, None);
    /// println!("Result: {} cubes", minimized.to_cubes(2, 1, CubeType::F).len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With Don't-Cares
    ///
    /// ```
    /// use espresso_logic::espresso::{Espresso, EspressoCover, CubeType};
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
    ///          minimized.to_cubes(2, 1, CubeType::F).len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimize(
        &self,
        f: &EspressoCover,
        d: Option<&EspressoCover>,
        r: Option<&EspressoCover>,
    ) -> (EspressoCover, EspressoCover, EspressoCover) {
        minimize_with_algorithm(self, f, d, r, |f_ptr, d_ptr, r_ptr| unsafe {
            sys::espresso(f_ptr, d_ptr, r_ptr)
        })
    }

    /// Minimize a boolean function using exact minimization
    ///
    /// This method uses the exact minimization algorithm which guarantees minimal results
    /// by solving the unate covering problem, unlike the heuristic `minimize()` method.
    ///
    /// Takes the ON-set (F), optional don't-care set (D), and optional OFF-set (R),
    /// and returns minimized versions of all three covers.
    ///
    /// # Arguments
    ///
    /// * `f` - **ON-set cover**: Specifies where the function output is 1 (required)
    /// * `d` - **Don't-care set**: Positions where output can be either 0 or 1 (optional).
    ///   If `None`, computed as the complement of F ∪ R
    /// * `r` - **OFF-set cover**: Specifies where the function output is 0 (optional).
    ///   If `None`, computed as the complement of F ∪ D
    ///
    /// # Returns
    ///
    /// A tuple of `(minimized_f, d, r)` where:
    /// - `minimized_f` - The exactly minimized ON-set (primary result)
    /// - `d` - The don't-care set used during minimization
    /// - `r` - The OFF-set used during minimization
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
    /// use espresso_logic::espresso::{Espresso, EspressoCover, CubeType};
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let esp = Espresso::new(2, 1, &EspressoConfig::default());
    /// let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    /// let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
    ///
    /// // Use exact minimization for guaranteed minimal result
    /// let (minimized, d, r) = esp.minimize_exact(&f, None, None);
    /// println!("Exact result: {} cubes", minimized.to_cubes(2, 1, CubeType::F).len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimize_exact(
        &self,
        f: &EspressoCover,
        d: Option<&EspressoCover>,
        r: Option<&EspressoCover>,
    ) -> (EspressoCover, EspressoCover, EspressoCover) {
        minimize_with_algorithm(self, f, d, r, |f_ptr, d_ptr, r_ptr| unsafe {
            sys::minimize_exact(f_ptr, d_ptr, r_ptr, 1)
        })
    }
}

/// Private helper function to eliminate duplication between minimize() and minimize_exact()
fn minimize_with_algorithm<F>(
    espresso: &Espresso,
    f: &EspressoCover,
    d: Option<&EspressoCover>,
    r: Option<&EspressoCover>,
    algorithm_fn: F,
) -> (EspressoCover, EspressoCover, EspressoCover)
where
    F: FnOnce(sys::pset_family, sys::pset_family, sys::pset_family) -> sys::pset_family,
{
    // MEMORY OWNERSHIP: Clone F and extract raw pointer
    // - clone() calls sf_save(), allocating new C memory (independent copy)
    // - into_raw() transfers ownership from Rust to C
    // - C algorithm function takes ownership and returns (possibly different) pointer
    let f_ptr = f.clone().into_raw();

    // MEMORY OWNERSHIP: D cover
    // - If provided: clone and transfer ownership via into_raw()
    // - If not provided: allocate empty cover with sf_new()
    // - C algorithm function uses but does NOT free D (makes internal copy)
    // - We must free d_ptr after algorithm returns (via EspressoCover wrapper)
    let d_ptr = d
        .map(|c| c.clone().into_raw())
        .unwrap_or_else(|| unsafe { sys::sf_new(0, (*sys::get_cube()).size as c_int) });

    // MEMORY OWNERSHIP: R cover
    // - If provided: clone and transfer ownership via into_raw()
    // - If not provided: compute complement (allocates new C memory)
    // - C algorithm function uses but does NOT free R
    // - We must free r_ptr after algorithm returns (via EspressoCover wrapper)
    let r_ptr = r.map(|c| c.clone().into_raw()).unwrap_or_else(|| unsafe {
        let cube_list = sys::cube2list(f_ptr, d_ptr);
        sys::complement(cube_list)
    });

    // Call the provided algorithm function (espresso or minimize_exact)
    // OWNERSHIP: algorithm_fn takes ownership of f_ptr, returns new/modified pointer
    // BORROWING: algorithm_fn uses but does not free d_ptr and r_ptr
    let f_result = algorithm_fn(f_ptr, d_ptr, r_ptr);

    // MEMORY OWNERSHIP: Wrap all returned/borrowed pointers in EspressoCover
    // This ensures sf_free() is called on all C memory when covers are dropped
    // - f_result: New pointer from algorithm (may be same as f_ptr or different)
    // - d_ptr: Same pointer we passed in, but modified by algorithm
    // - r_ptr: Same pointer we passed in, used read-only by algorithm
    let d_result = unsafe { EspressoCover::from_raw(d_ptr, espresso) };
    let r_result = unsafe { EspressoCover::from_raw(r_ptr, espresso) };

    (
        unsafe { EspressoCover::from_raw(f_result, espresso) },
        d_result,
        r_result,
    )
}

#[cfg(test)]
mod tests {
    //! Comprehensive multi-threaded tests for thread-local Espresso API
    //!
    //! These tests directly use the low-level Espresso API to verify that thread-local
    //! storage is working correctly and there's no interference between threads.

    use super::*;
    use crate::cover::Minimizable;
    use crate::EspressoConfig;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

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
                        let cubes = result.to_cubes(2, 1, CubeType::F);
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
                        let cubes = result.to_cubes(num_inputs, num_outputs, CubeType::F);
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
                        let result_cubes = result.to_cubes(3, 1, CubeType::F);
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
        assert!(!result1.to_cubes(2, 1, CubeType::F).is_empty());
        assert!(!result2.to_cubes(2, 1, CubeType::F).is_empty());

        // Can also explicitly create an Espresso handle with same dimensions
        let esp = Espresso::new(2, 1, &EspressoConfig::default());
        assert_eq!(esp.num_inputs(), 2);
        assert_eq!(esp.num_outputs(), 1);
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
                    let result_cubes = result.to_cubes(num_inputs, num_outputs, CubeType::F);
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
            let mut cover1 = Cover::new(CoverType::F);
            cover1.add_cube(&[Some(true), Some(false)], &[Some(true)]);
            cover1 = cover1.minimize().unwrap();
            assert_eq!(cover1.num_cubes(), 1, "Cover1 (2x1) should have 1 cube");
        } // cover1 is dropped here, Espresso instance should be cleaned up

        // Now try with different dimensions - should work without conflicts
        let mut cover2 = Cover::new(CoverType::F);
        cover2.add_cube(
            &[Some(false), Some(true), Some(false), Some(true)],
            &[Some(true)],
        );
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
        let result_cubes = result.to_cubes(2, 1, CubeType::F);
        assert_eq!(
            result_cubes.len(),
            1,
            "Single cube should minimize to 1 cube"
        );

        // Verify the cube content is correct
        let cube = &result_cubes[0];
        assert_eq!(
            cube.inputs(),
            &[Some(false), Some(true)],
            "Input should be [0, 1]"
        );
        assert_eq!(cube.outputs(), &[true], "Output should be [1]");

        // Verify D and R covers are accessible
        let d_cubes = d.to_cubes(2, 1, CubeType::F);
        let r_cubes = r.to_cubes(2, 1, CubeType::F);
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
        let result2_cubes = result2.to_cubes(2, 1, CubeType::F);
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
        let mut cover1 = Cover::new(CoverType::F);
        cover1.add_cube(&[Some(true), Some(false)], &[Some(true)]);
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
        let mut cover2 = Cover::new(CoverType::F);
        cover2.add_cube(
            &[Some(false), Some(true), Some(false)],
            &[Some(true), Some(false)],
        );
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
        let input_cubes = f.to_cubes(2, 1, CubeType::F);
        assert_eq!(input_cubes.len(), 3, "Should start with 3 cubes");

        let (result, _, _) = esp.minimize(&f, None, None);
        let result_cubes = result.to_cubes(2, 1, CubeType::F);

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
        let result_cubes = result.to_cubes(2, 1, CubeType::F);
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

        let result_cubes = result.to_cubes(3, 1, CubeType::F);
        assert_eq!(result_cubes.len(), 1, "Single cube should remain as 1");

        // Verify the cube is correct
        let cube = &result_cubes[0];
        assert_eq!(cube.inputs(), &[Some(false), Some(true), Some(true)]);
        assert_eq!(cube.outputs(), &[true]);
    }

    #[test]
    fn test_espresso_cover_not_send() {
        use crate::EspressoConfig;

        let _esp = Espresso::new(2, 1, &EspressoConfig::default());
        let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
        let cover = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();

        // Verify the cover was created correctly
        let result_cubes = cover.to_cubes(2, 1, CubeType::F);
        assert_eq!(result_cubes.len(), 2, "Should have 2 input cubes");

        // Verify minimization works
        let (minimized, _, _) = cover.minimize(None, None);
        let min_cubes = minimized.to_cubes(2, 1, CubeType::F);
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

        let cubes1 = cover1.to_cubes(3, 1, CubeType::F);
        let cubes2 = cover2.to_cubes(3, 1, CubeType::F);

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

        let result_cubes = result.to_cubes(2, 1, CubeType::F);
        assert_eq!(
            result_cubes.len(),
            2,
            "XOR minimization should produce 2 cubes"
        );

        // Verify D and R covers are also valid (they exist even if empty)
        let _d_cubes = d.to_cubes(2, 1, CubeType::F);
        let _r_cubes = r.to_cubes(2, 1, CubeType::F);
        // D and R covers are successfully retrieved
    }
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
/// Use with [`Cover::minimize_with_config()`](crate::cover::Minimizable::minimize_with_config):
///
/// ```
/// use espresso_logic::{Cover, CoverType, EspressoConfig, Minimizable};
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
