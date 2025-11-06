//! Unsafe direct bindings to Espresso C library
//!
//! This module is internal and should only be used by worker processes.
//! It directly manipulates global state and is NOT thread-safe.

use crate::sys;
use crate::UnsafeCover;
use std::os::raw::c_int;
use std::ptr;

/// UNSAFE: Direct wrapper around Espresso that uses global state
///
/// This is only used internally by worker processes. Do NOT expose this publicly!
pub(crate) struct UnsafeEspresso {
    pub(crate) initialized: bool,
}

impl UnsafeEspresso {
    /// Create a new UnsafeEspresso instance with custom configuration
    ///
    /// SAFETY: This manipulates global state and is NOT thread-safe
    pub(crate) fn new_with_config(
        num_inputs: usize,
        num_outputs: usize,
        config: crate::ipc::IpcConfig,
    ) -> Self {
        unsafe {
            // Always tear down existing cube state to avoid interference
            if !sys::cube.fullset.is_null() {
                sys::setdown_cube();
                if !sys::cube.part_size.is_null() {
                    libc::free(sys::cube.part_size as *mut libc::c_void);
                    sys::cube.part_size = ptr::null_mut();
                }
            }

            // Initialize the cube structure
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

            // Set the output size
            *sys::cube.part_size.add(num_inputs) = num_outputs as c_int;

            // Setup cube
            sys::cube_setup();

            // Apply custom configuration
            sys::debug = if config.debug { 1 } else { 0 };
            sys::verbose_debug = if config.verbose_debug { 1 } else { 0 };
            sys::trace = if config.trace { 1 } else { 0 };
            sys::summary = if config.summary { 1 } else { 0 };
            sys::remove_essential = if config.remove_essential { 1 } else { 0 };
            sys::force_irredundant = if config.force_irredundant { 1 } else { 0 };
            sys::unwrap_onset = if config.unwrap_onset { 1 } else { 0 };
            sys::single_expand = if config.single_expand { 1 } else { 0 };
            sys::use_super_gasp = if config.use_super_gasp { 1 } else { 0 };
            sys::use_random_order = if config.use_random_order { 1 } else { 0 };
            sys::skip_make_sparse = 0;
        }

        UnsafeEspresso { initialized: true }
    }

    /// Minimize using direct C API (UNSAFE: uses global state)
    /// Returns (F cover, D cover, R cover)
    pub(crate) fn minimize(
        &mut self,
        f: UnsafeCover,
        d: Option<UnsafeCover>,
        r: Option<UnsafeCover>,
    ) -> (UnsafeCover, UnsafeCover, UnsafeCover) {
        let f_ptr = f.clone().into_raw();

        let d_ptr = d
            .as_ref()
            .map(|c| c.clone().into_raw())
            .unwrap_or_else(|| unsafe { sys::sf_new(0, sys::cube.size as c_int) });

        let r_ptr = r
            .as_ref()
            .map(|c| c.clone().into_raw())
            .unwrap_or_else(|| unsafe {
                let cube_list = sys::cube2list(f_ptr, d_ptr);
                sys::complement(cube_list)
            });

        let f_result = unsafe { sys::espresso(f_ptr, d_ptr, r_ptr) };

        // espresso() modifies D in place and we can retrieve it
        // Always return D and R covers since they're always processed
        let d_result = unsafe { UnsafeCover::from_raw(d_ptr) };
        let r_result = unsafe { UnsafeCover::from_raw(r_ptr) };

        (
            unsafe { UnsafeCover::from_raw(f_result) },
            d_result,
            r_result,
        )
    }
}

impl Drop for UnsafeEspresso {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                sys::setdown_cube();
                if !sys::cube.part_size.is_null() {
                    libc::free(sys::cube.part_size as *mut libc::c_void);
                    sys::cube.part_size = ptr::null_mut();
                }
            }
        }
    }
}
