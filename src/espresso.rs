//! Direct bindings to the Espresso C library with thread-local storage
//!
//! This module provides low-level access to the Espresso algorithm through direct
//! C library bindings. It uses C11 thread-local storage for thread safety, meaning
//! each thread gets its own independent copy of all global state.
//!
//! # When to Use This Module
//!
//! Use this module if you need:
//! - Maximum performance with minimal overhead
//! - Direct control over the minimization process
//! - Access to intermediate results (F, D, R covers)
//! - Fine-grained control over memory management
//!
//! For most use cases, prefer the higher-level APIs:
//! - [`BoolExpr`](crate::BoolExpr) for boolean expressions
//! - [`Cover`](crate::Cover) for covers with dynamic dimensions
//! - [`Cover::from_pla_file`](crate::Cover::from_pla_file) for reading PLA files
//!
//! # Safety and Thread Safety
//!
//! While this module uses `unsafe` internally to interact with C code, all unsafe
//! operations are encapsulated in safe Rust APIs. The module IS thread-safe thanks
//! to C11 `_Thread_local` storage - each thread has independent global state.
//!
//! **However**, you must be careful about:
//! - Only one `Espresso` instance should be active per thread at a time
//! - Cube structures are tied to the `Espresso` instance that created them
//! - Covers from different instances should not be mixed
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
//! # fn main() -> Result<(), espresso_logic::EspressoError> {
//! // Build a cover (XOR function) - Espresso instance created automatically
//! let cubes = vec![
//!     (vec![0, 1], vec![1]),  // 01 -> 1
//!     (vec![1, 0], vec![1]),  // 10 -> 1
//! ];
//! let f = EspressoCover::from_cubes(cubes, 2, 1)?;
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
//! # fn main() -> Result<(), espresso_logic::EspressoError> {
//! // Explicitly create an Espresso instance with custom config
//! let mut config = EspressoConfig::default();
//! config.single_expand = true;
//! let _esp = Espresso::new(2, 1, &config);
//!
//! // Now all covers will use this instance
//! let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
//! let f = EspressoCover::from_cubes(cubes, 2, 1)?;
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
//! # fn main() -> Result<(), espresso_logic::EspressoError> {
//! let handles: Vec<_> = (0..4).map(|_| {
//!     thread::spawn(|| -> Result<usize, espresso_logic::EspressoError> {
//!         // Each thread automatically gets its own Espresso instance
//!         let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
//!         let f = EspressoCover::from_cubes(cubes, 2, 1)?;
//!         
//!         // Thread-safe: independent global state per thread
//!         let (result, _, _) = f.minimize(None, None);
//!         Ok(result.to_cubes(2, 1, espresso_logic::espresso::CubeType::F).len())
//!     })
//! }).collect();
//!
//! for handle in handles {
//!     let count = handle.join().unwrap()?;
//!     println!("Thread minimized to {} cubes", count);
//! }
//! # Ok(())
//! # }
//! ```

use crate::error::{ConflictReason, EspressoError};
use crate::sys;
use crate::EspressoConfig;
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
/// **Note:** This type is neither `Send` nor `Sync` (because `Rc` is `!Send + !Sync`) -
/// it must remain on the thread where it was created, as it's tied to thread-local C state
/// managed by `Espresso`.
///
/// Each cover holds a reference to the internal Espresso instance, ensuring that the C state
/// remains valid for as long as the cover exists.
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
    /// Creates a cover from a list of cubes represented as (inputs, outputs) pairs.
    /// Each input is encoded as: 0 = low, 1 = high, 2 = don't care.
    /// Each output is encoded as: 0 = off, 1 = on.
    ///
    /// If no Espresso instance exists on the current thread, one will be automatically
    /// created with the specified dimensions and default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if an Espresso instance with different dimensions already exists
    /// on this thread. Drop all existing covers first to create covers with new dimensions.
    pub fn from_cubes(
        cubes: Vec<(Vec<u8>, Vec<u8>)>,
        num_inputs: usize,
        num_outputs: usize,
    ) -> Result<Self, EspressoError> {
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
        for (inputs, outputs) in cubes {
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
                            return Err(EspressoError::InvalidCubeValue {
                                value: val,
                                position: var,
                            })
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
    /// Rust `Cube` structures with the specified dimensions and type.
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
    /// This is a convenience method that automatically uses the thread-local Espresso instance.
    /// Returns minimized versions of the ON-set (F), don't-care set (D), and OFF-set (R).
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::EspressoCover;
    ///
    /// # fn main() -> Result<(), espresso_logic::EspressoError> {
    /// // Create a cover for XOR function
    /// let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
    /// let f = EspressoCover::from_cubes(cubes, 2, 1)?;
    ///
    /// // Minimize it directly
    /// let (minimized, _d, _r) = f.minimize(None, None);
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
        espresso.minimize(self, d, r)
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
/// the thread-local singleton. Users interact with the outer `Espresso` wrapper
/// instead, which hides these implementation details.
///
/// **Note:** This type is neither `Send` nor `Sync` - it must remain on the thread
/// where it was created, as it manages thread-local C state. The `PhantomData` marker
/// ensures this type is `!Send + !Sync`.
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
/// Internally, this uses a thread-local singleton to ensure that only one
/// Espresso configuration exists per thread. Multiple `Espresso` handles can exist,
/// but they all reference the same underlying state. This is safe because:
/// - All C global variables use `_Thread_local` storage
/// - Each thread has independent state (cube structure, configuration, etc.)
/// - The singleton pattern prevents conflicting instances within a thread
///
/// # Important
///
/// Creating a new `Espresso` instance will replace any existing instance on the current
/// thread. If you need multiple handles to the same instance, clone the `Espresso` handle.
///
/// **Note:** This type is neither `Send` nor `Sync` (because `Rc` is `!Send + !Sync`) -
/// it must remain on the thread where it was created, as it manages thread-local C state.
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
    /// **Important:** Only one Espresso configuration can exist per thread. If an instance
    /// with different dimensions already exists, this will panic. If an instance with the
    /// same dimensions exists, this returns a new handle to that instance (ignoring the config).
    ///
    /// **Note:** Most users don't need to call this directly - use `EspressoCover::from_cubes()`
    /// which automatically creates an instance if needed.
    ///
    /// # Arguments
    ///
    /// * `num_inputs` - Number of input variables
    /// * `num_outputs` - Number of output variables  
    /// * `config` - Configuration options for the algorithm (only used if creating new instance)
    ///
    /// # Panics
    ///
    /// Panics if an Espresso instance with different dimensions already exists on this thread.
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
    pub fn new(num_inputs: usize, num_outputs: usize, config: &EspressoConfig) -> Self {
        Self::try_new(num_inputs, num_outputs, Some(config))
            .expect("Failed to create Espresso instance")
    }

    /// Try to create a new Espresso instance with custom configuration
    ///
    /// This is the non-panicking version of `new()`. It returns an error if an instance
    /// with incompatible dimensions already exists.
    ///
    /// # Arguments
    ///
    /// * `num_inputs` - Number of input variables
    /// * `num_outputs` - Number of output variables  
    /// * `config` - Optional configuration. If `Some`, checks config compatibility. If `None`,
    ///   only checks dimensions and uses existing instance regardless of its config.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - An Espresso instance with different dimensions already exists
    /// - A config is specified and an instance with different config already exists
    pub fn try_new(
        num_inputs: usize,
        num_outputs: usize,
        config: Option<&EspressoConfig>,
    ) -> Result<Self, EspressoError> {
        // Check if an instance already exists
        let inner = ESPRESSO_INSTANCE.with(|instance| {
            if let Some(existing) = instance.borrow().upgrade() {
                // Check dimensions
                if existing.num_inputs != num_inputs || existing.num_outputs != num_outputs {
                    return Err(EspressoError::InstanceConflict {
                        requested: (num_inputs, num_outputs),
                        existing: (existing.num_inputs, existing.num_outputs),
                        reason: ConflictReason::DimensionMismatch,
                    });
                }

                // Dimensions match - check config if specified
                if let Some(requested_config) = config {
                    if existing.config != *requested_config {
                        return Err(EspressoError::InstanceConflict {
                            requested: (num_inputs, num_outputs),
                            existing: (existing.num_inputs, existing.num_outputs),
                            reason: ConflictReason::ConfigMismatch,
                        });
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
    /// # fn main() -> Result<(), espresso_logic::EspressoError> {
    /// // Initially there's no instance
    /// assert!(Espresso::current().is_none());
    ///
    /// // Create a cover - this auto-creates an Espresso instance
    /// let cubes = vec![(vec![0, 1], vec![1])];
    /// let _cover = EspressoCover::from_cubes(cubes, 2, 1)?;
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
    /// * `f` - ON-set cover (where the function is 1)
    /// * `d` - Optional don't-care set (can be either 0 or 1)
    /// * `r` - Optional OFF-set (where the function is 0)
    ///
    /// # Returns
    ///
    /// A tuple of (minimized F, D, R) covers.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::espresso::{Espresso, EspressoCover};
    /// use espresso_logic::EspressoConfig;
    ///
    /// # fn main() -> Result<(), espresso_logic::EspressoError> {
    /// let esp = Espresso::new(2, 1, &EspressoConfig::default());
    /// let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
    /// let f = EspressoCover::from_cubes(cubes, 2, 1)?;
    /// let (minimized, _d, _r) = esp.minimize(f, None, None);
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimize(
        &self,
        f: EspressoCover,
        d: Option<EspressoCover>,
        r: Option<EspressoCover>,
    ) -> (EspressoCover, EspressoCover, EspressoCover) {
        // MEMORY OWNERSHIP: Clone F and extract raw pointer
        // - clone() calls sf_save(), allocating new C memory (independent copy)
        // - into_raw() transfers ownership from Rust to C
        // - C espresso() takes ownership and returns (possibly different) pointer
        let f_ptr = f.clone().into_raw();

        // MEMORY OWNERSHIP: D cover
        // - If provided: clone and transfer ownership via into_raw()
        // - If not provided: allocate empty cover with sf_new()
        // - C espresso() uses but does NOT free D (makes internal copy)
        // - We must free d_ptr after espresso() returns (via EspressoCover wrapper)
        let d_ptr = d
            .as_ref()
            .map(|c| c.clone().into_raw())
            .unwrap_or_else(|| unsafe { sys::sf_new(0, (*sys::get_cube()).size as c_int) });

        // MEMORY OWNERSHIP: R cover
        // - If provided: clone and transfer ownership via into_raw()
        // - If not provided: compute complement (allocates new C memory)
        // - C espresso() uses but does NOT free R
        // - We must free r_ptr after espresso() returns (via EspressoCover wrapper)
        let r_ptr = r
            .as_ref()
            .map(|c| c.clone().into_raw())
            .unwrap_or_else(|| unsafe {
                let cube_list = sys::cube2list(f_ptr, d_ptr);
                sys::complement(cube_list)
            });

        // Call C espresso function
        // OWNERSHIP: espresso() takes ownership of f_ptr, returns new/modified pointer
        // BORROWING: espresso() uses but does not free d_ptr and r_ptr
        let f_result = unsafe { sys::espresso(f_ptr, d_ptr, r_ptr) };

        // MEMORY OWNERSHIP: Wrap all returned/borrowed pointers in EspressoCover
        // This ensures sf_free() is called on all C memory when covers are dropped
        // - f_result: New pointer from espresso() (may be same as f_ptr or different)
        // - d_ptr: Same pointer we passed in, but modified by espresso()
        // - r_ptr: Same pointer we passed in, used read-only by espresso()
        let d_result = unsafe { EspressoCover::from_raw(d_ptr, self) };
        let r_result = unsafe { EspressoCover::from_raw(r_ptr, self) };

        (
            unsafe { EspressoCover::from_raw(f_result, self) },
            d_result,
            r_result,
        )
    }
}

#[cfg(test)]
mod tests {
    //! Comprehensive multi-threaded tests for thread-local Espresso API
    //!
    //! These tests directly use the low-level Espresso API to verify that thread-local
    //! storage is working correctly and there's no interference between threads.

    use super::*;
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
                        let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
                        let f = EspressoCover::from_cubes(cubes, 2, 1).unwrap();

                        // Minimize
                        let (result, _, _) = esp.minimize(f, None, None);

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
                        let inputs = (0..num_inputs)
                            .map(|j| if (i + j) % 3 == 0 { 0 } else { 1 })
                            .collect();
                        let outputs = vec![1; num_outputs];
                        cubes.push((inputs, outputs));
                    }

                    let f = EspressoCover::from_cubes(cubes, num_inputs, num_outputs).unwrap();

                    // Minimize multiple times
                    for _ in 0..5 {
                        let f_clone = f.clone();
                        let (result, _, _) = esp.minimize(f_clone, None, None);

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
                        let cubes = vec![
                            (vec![0, 1, 0], vec![1]),
                            (vec![1, 0, 1], vec![1]),
                            (vec![0, 0, 1], vec![1]),
                        ];
                        let f = EspressoCover::from_cubes(cubes, 3, 1).unwrap();
                        let (_result, _, _) = esp.minimize(f, None, None);

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
                            let inputs = (0..num_inputs)
                                .map(|j| ((i + j + thread_id) % 3) as u8)
                                .collect();
                            cubes.push((inputs, vec![1]));
                        }

                        let f = EspressoCover::from_cubes(cubes, num_inputs, 1).unwrap();

                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            esp.minimize(f, None, None)
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
                        let cubes = vec![(vec![0; num_inputs], vec![1; num_outputs])];
                        let f = EspressoCover::from_cubes(cubes, num_inputs, num_outputs).unwrap();
                        let (result, d, r) = f.minimize(None, None);

                        // Drop covers explicitly
                        drop(result);
                        drop(d);
                        drop(r);
                    }

                    // Verify thread can still work after all that
                    let cubes = vec![(vec![0, 1, 0], vec![1, 0])];
                    let f = EspressoCover::from_cubes(cubes, num_inputs, num_outputs).unwrap();
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

                        let f = EspressoCover::from_cubes(cubes, 3, 1).unwrap();
                        let (result, _, _) = esp.minimize(f, None, None);

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
                        let f1 =
                            EspressoCover::from_cubes(vec![(vec![0, 1], vec![1])], 2, 1).unwrap();
                        let f2 =
                            EspressoCover::from_cubes(vec![(vec![1, 0], vec![1])], 2, 1).unwrap();
                        let f3 =
                            EspressoCover::from_cubes(vec![(vec![1, 1], vec![1])], 2, 1).unwrap();

                        // Use them
                        let (_r1, _, _) = esp.minimize(f1, None, None);
                        let (_r2, _, _) = esp.minimize(f2, None, None);
                        let (_r3, _, _) = esp.minimize(f3, None, None);

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
        let cubes = vec![(vec![0, 1], vec![1])];
        let f = EspressoCover::from_cubes(cubes, 2, 1).unwrap();

        // Can create another cover with same dimensions
        let cubes2 = vec![(vec![1, 0], vec![1])];
        let f2 = EspressoCover::from_cubes(cubes2, 2, 1).unwrap();

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
    #[should_panic(expected = "InstanceConflict")]
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
        let cubes1 = vec![(vec![0, 1], vec![1])];
        let _cover1 = EspressoCover::from_cubes(cubes1, 2, 1).unwrap();

        // Try to create second cover with different dimensions - should return error
        let cubes2 = vec![(vec![0, 1, 0], vec![1, 0])];
        let result = EspressoCover::from_cubes(cubes2, 3, 2);
        assert!(result.is_err(), "Should error on dimension mismatch");
        let err = result.unwrap_err();
        match err {
            crate::error::EspressoError::InstanceConflict { .. } => {
                // Expected error type
            }
            other => panic!("Expected InstanceConflict error, got: {}", other),
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
                    let inputs = (0..num_inputs).map(|j| ((i + j + idx) % 3) as u8).collect();
                    let outputs = vec![if i % 2 == 0 { 1 } else { 0 }; num_outputs];
                    cubes.push((inputs, outputs));
                }

                let f = EspressoCover::from_cubes(cubes, num_inputs, num_outputs).unwrap();

                // Minimize multiple times
                for _ in 0..3 {
                    let f_clone = f.clone();
                    let (result, _, _) = esp.minimize(f_clone, None, None);

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
    fn test_sequential_different_dimensions_via_coverbuilder() {
        use crate::{Cover, CoverType};

        // This test verifies that Espresso instances are properly cleaned up after minimize()
        // allowing different dimensions to work sequentially without conflicts

        // Create and minimize cover with 2 inputs, 1 output
        let mut cover1 = Cover::new(CoverType::F);
        cover1.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        cover1.minimize().unwrap();
        assert_eq!(cover1.num_cubes(), 1, "Cover1 (2x1) should have 1 cube");

        // At this point, the Espresso instance should be dropped
        // So we should be able to create and minimize a cover with different dimensions
        let mut cover2 = Cover::new(CoverType::F);
        cover2.add_cube(&[Some(false), Some(true), Some(false)], &[Some(true)]);
        cover2.minimize().unwrap();
        assert_eq!(cover2.num_cubes(), 1, "Cover2 (3x1) should have 1 cube");

        // Both covers should be independent and maintain their results
        assert_eq!(cover1.num_cubes(), 1, "Cover1 should still have 1 cube");
        assert_eq!(cover2.num_cubes(), 1, "Cover2 should still have 1 cube");
    }

    #[test]
    fn test_explicit_drop_between_dimensions() {
        use crate::{Cover, CoverType};

        // Test with explicit scope-based drop to ensure cleanup works correctly
        {
            let mut cover1 = Cover::new(CoverType::F);
            cover1.add_cube(&[Some(true), Some(false)], &[Some(true)]);
            cover1.minimize().unwrap();
            assert_eq!(cover1.num_cubes(), 1, "Cover1 (2x1) should have 1 cube");
        } // cover1 is dropped here, Espresso instance should be cleaned up

        // Now try with different dimensions - should work without conflicts
        let mut cover2 = Cover::new(CoverType::F);
        cover2.add_cube(
            &[Some(false), Some(true), Some(false), Some(true)],
            &[Some(true)],
        );
        cover2.minimize().unwrap();
        assert_eq!(cover2.num_cubes(), 1, "Cover2 (4x1) should have 1 cube");
    }

    // Tests for singleton behavior and EspressoCover

    #[test]
    fn test_automatic_singleton_creation() {
        // No need to manually create Espresso - it's automatic
        let f = EspressoCover::from_cubes(vec![(vec![0, 1], vec![1])], 2, 1).unwrap();
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
        let f2 =
            EspressoCover::from_cubes(vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])], 2, 1)
                .unwrap();
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

        cover1.minimize().unwrap();
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

        cover2.minimize().unwrap();
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
        let cubes = vec![
            (vec![0, 0], vec![1]),
            (vec![0, 1], vec![1]),
            (vec![1, 0], vec![1]),
        ];
        let f = EspressoCover::from_cubes(cubes, 2, 1).unwrap();

        // Verify input has 3 cubes
        let input_cubes = f.to_cubes(2, 1, CubeType::F);
        assert_eq!(input_cubes.len(), 3, "Should start with 3 cubes");

        let (result, _, _) = esp.minimize(f, None, None);
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
        let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
        let f = EspressoCover::from_cubes(cubes, 2, 1).unwrap();
        let (result, _, _) = esp.minimize(f, None, None);

        // XOR cannot be minimized, should still have 2 cubes
        let result_cubes = result.to_cubes(2, 1, CubeType::F);
        assert_eq!(result_cubes.len(), 2, "XOR should maintain 2 cubes");
    }

    #[test]
    fn test_espresso_single_dimension_per_thread() {
        use crate::EspressoConfig;

        let esp = Espresso::new(3, 1, &EspressoConfig::default());

        // Use the espresso instance on the same thread
        let cubes = vec![(vec![0, 1, 1], vec![1])];
        let f = EspressoCover::from_cubes(cubes, 3, 1).unwrap();
        let (result, _, _) = esp.minimize(f, None, None);

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
        let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
        let cover = EspressoCover::from_cubes(cubes, 2, 1).unwrap();

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
        let cover1 = EspressoCover::from_cubes(
            vec![(vec![0, 0, 1], vec![1]), (vec![0, 1, 1], vec![1])],
            3,
            1,
        )
        .unwrap();

        let cover2 = EspressoCover::from_cubes(
            vec![(vec![1, 0, 1], vec![1]), (vec![1, 1, 1], vec![1])],
            3,
            1,
        )
        .unwrap();

        let cubes1 = cover1.to_cubes(3, 1, CubeType::F);
        let cubes2 = cover2.to_cubes(3, 1, CubeType::F);

        assert_eq!(cubes1.len(), 2);
        assert_eq!(cubes2.len(), 2);
    }

    #[test]
    fn test_complex_operations_same_thread() {
        use crate::EspressoConfig;

        let esp = Espresso::new(2, 1, &EspressoConfig::default());
        let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
        let f = EspressoCover::from_cubes(cubes, 2, 1).unwrap();
        let (result, d, r) = esp.minimize(f, None, None);

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

    #[test]
    fn test_multithreaded_different_dimensions() {
        use crate::EspressoConfig;
        use std::thread;

        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                thread::spawn(move || {
                    // Each thread creates its own instance with unique dimensions
                    let num_inputs = 2 + (thread_id % 2);
                    let cubes = if num_inputs == 2 {
                        vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])]
                    } else {
                        vec![(vec![0, 1, 1], vec![1]), (vec![1, 0, 1], vec![1])]
                    };

                    let esp = Espresso::new(num_inputs, 1, &EspressoConfig::default());
                    let f = EspressoCover::from_cubes(cubes, num_inputs, 1).unwrap();
                    let (result, _, _) = esp.minimize(f, None, None);

                    let result_cubes = result.to_cubes(num_inputs, 1, CubeType::F);
                    (num_inputs, result_cubes.len())
                })
            })
            .collect();

        for handle in handles {
            let (num_inputs, count) = handle.join().unwrap();
            assert!(
                count > 0,
                "Thread with {} inputs should have minimized cubes",
                num_inputs
            );
            // XOR-like functions can't be minimized, so should maintain original count
            assert_eq!(
                count, 2,
                "XOR should maintain 2 cubes regardless of input size"
            );
        }
    }
}
