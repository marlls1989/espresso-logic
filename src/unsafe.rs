//! Unsafe direct bindings to Espresso C library
//!
//! This module is internal and should only be used by worker processes.
//! It directly manipulates global state and is NOT thread-safe.

use crate::sys;
use crate::worker::{SerializedCover, SerializedCube, WorkerSerializable};
use std::os::raw::c_int;
use std::ptr;

/// Internal cover with raw C pointer (only used in workers)
pub(crate) struct UnsafeCover {
    ptr: sys::pset_family,
}

impl UnsafeCover {
    /// Create a new empty cover
    pub(crate) fn new(capacity: usize, cube_size: usize) -> Self {
        let ptr = unsafe { sys::sf_new(capacity as c_int, cube_size as c_int) };
        UnsafeCover { ptr }
    }

    /// Create from raw pointer
    pub(crate) unsafe fn from_raw(ptr: sys::pset_family) -> Self {
        UnsafeCover { ptr }
    }

    /// Convert to raw pointer
    pub(crate) fn into_raw(self) -> sys::pset_family {
        let ptr = self.ptr;
        std::mem::forget(self);
        ptr
    }

    /// Build cover from cube data (INTERNAL: only in worker processes)
    pub(crate) fn build_from_cubes(
        cubes: Vec<(Vec<u8>, Vec<u8>)>,
        _num_inputs: usize,
        _num_outputs: usize,
    ) -> Self {
        // This assumes UnsafeEspresso has already initialized the cube structure
        let cube_size = unsafe { sys::cube.size as usize };

        // Create empty cover with capacity
        let mut cover = UnsafeCover::new(cubes.len(), cube_size);

        // Add each cube to the cover
        for (inputs, outputs) in cubes {
            unsafe {
                let cf = *sys::cube.temp.add(0);
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
                        _ => panic!("Invalid input value"),
                    }
                }

                // Set output values
                let output_var = sys::cube.num_vars - 1;
                let output_first = *sys::cube.first_part.add(output_var as usize) as usize;

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

        cover
    }
}

impl Drop for UnsafeCover {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::sf_free(self.ptr);
            }
        }
    }
}

impl Clone for UnsafeCover {
    fn clone(&self) -> Self {
        let ptr = unsafe { sys::sf_save(self.ptr) };
        UnsafeCover { ptr }
    }
}

impl WorkerSerializable for UnsafeCover {
    /// Serialize this cover for IPC
    fn serialize(&self) -> SerializedCover {
        unsafe {
            let count = (*self.ptr).count as usize;
            let wsize = (*self.ptr).wsize as usize;
            let sf_size = (*self.ptr).sf_size as usize;
            let data = (*self.ptr).data;

            let mut cubes = Vec::with_capacity(count);
            for i in 0..count {
                let cube_ptr = data.add(i * wsize);
                let mut cube_data = Vec::with_capacity(wsize);
                for j in 0..wsize {
                    cube_data.push(*cube_ptr.add(j));
                }
                cubes.push(SerializedCube { data: cube_data });
            }

            SerializedCover {
                count,
                wsize,
                sf_size,
                cubes,
            }
        }
    }
}

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
        config: crate::worker::IpcConfig,
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
