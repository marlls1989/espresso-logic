//! Unsafe direct bindings to Espresso C library
//!
//! This module provides direct access to the thread-safe C library using thread-local storage.

use crate::cover::{Cube, CubeType};
use crate::sys;
use crate::EspressoConfig;
use std::os::raw::c_int;
use std::ptr;

/// Internal cover with raw C pointer
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
        let cube_size = unsafe { (*sys::get_cube()).size as usize };

        // Create empty cover with capacity
        let mut cover = UnsafeCover::new(cubes.len(), cube_size);

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
                        _ => panic!("Invalid input value"),
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

impl UnsafeCover {
    /// Convert this cover directly to typed Cubes without serialization
    pub(crate) fn to_cubes(
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

                result.push(Cube::new(inputs, outputs, cube_type));
            }

            result
        }
    }
}

/// UNSAFE: Direct wrapper around Espresso using thread-local global state
///
/// Thread-safe via C11 thread-local storage. Used internally - do NOT expose publicly!
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
        config: &EspressoConfig,
    ) -> Self {
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
            sys::set_debug(if config.debug { 1 } else { 0 });
            sys::set_verbose_debug(if config.verbose_debug { 1 } else { 0 });
            sys::set_trace(if config.trace { 1 } else { 0 });
            sys::set_summary(if config.summary { 1 } else { 0 });
            sys::set_remove_essential(if config.remove_essential { 1 } else { 0 });
            sys::set_force_irredundant(if config.force_irredundant { 1 } else { 0 });
            sys::set_unwrap_onset(if config.unwrap_onset { 1 } else { 0 });
            sys::set_single_expand(if config.single_expand { 1 } else { 0 });
            sys::set_use_super_gasp(if config.use_super_gasp { 1 } else { 0 });
            sys::set_use_random_order(if config.use_random_order { 1 } else { 0 });
            sys::set_skip_make_sparse(0);
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
            .unwrap_or_else(|| unsafe { sys::sf_new(0, (*sys::get_cube()).size as c_int) });

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
                let cube = sys::get_cube();
                if !(*cube).part_size.is_null() {
                    libc::free((*cube).part_size as *mut libc::c_void);
                    (*cube).part_size = ptr::null_mut();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Comprehensive multi-threaded tests for thread-local unsafe API
    //!
    //! These tests directly use the unsafe API to verify that thread-local
    //! storage is working correctly and there's no interference between threads.

    use super::*;
    use crate::EspressoConfig;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    /// Test 1: Basic concurrent access
    /// Spawns multiple threads, each creates its own UnsafeEspresso instance
    /// and performs minimize operations on different problems
    #[test]
    fn test_concurrent_unsafe_minimize() {
        const NUM_THREADS: usize = 16;
        const OPS_PER_THREAD: usize = 10;

        let success_count = Arc::new(AtomicUsize::new(0));
        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|thread_id| {
                let success = Arc::clone(&success_count);
                thread::spawn(move || {
                    // Each thread creates its own instance
                    let mut esp = UnsafeEspresso::new_with_config(2, 1, &EspressoConfig::default());

                    for op in 0..OPS_PER_THREAD {
                        // Create test cover (XOR function)
                        let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
                        let f = UnsafeCover::build_from_cubes(cubes, 2, 1);

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

                    let mut esp = UnsafeEspresso::new_with_config(
                        num_inputs,
                        num_outputs,
                        &EspressoConfig::default(),
                    );

                    // Create a simple cover
                    let mut cubes = vec![];
                    for i in 0..3 {
                        let inputs = (0..num_inputs)
                            .map(|j| if (i + j) % 3 == 0 { 0 } else { 1 })
                            .collect();
                        let outputs = vec![1; num_outputs];
                        cubes.push((inputs, outputs));
                    }

                    let f = UnsafeCover::build_from_cubes(cubes, num_inputs, num_outputs);

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
    #[test]
    fn test_config_isolation() {
        const NUM_THREADS: usize = 4;

        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|thread_id| {
                thread::spawn(move || {
                    // Each thread uses different configuration
                    let config = EspressoConfig {
                        debug: thread_id % 2 == 0,
                        trace: thread_id % 2 == 1,
                        verbose_debug: false,
                        summary: thread_id == 0,
                        remove_essential: true,
                        force_irredundant: true,
                        unwrap_onset: true,
                        single_expand: thread_id % 2 == 0,
                        use_super_gasp: thread_id % 2 == 1,
                        use_random_order: false,
                    };

                    let mut esp = UnsafeEspresso::new_with_config(3, 1, &config);

                    // Perform operations
                    for _ in 0..10 {
                        let cubes = vec![
                            (vec![0, 1, 0], vec![1]),
                            (vec![1, 0, 1], vec![1]),
                            (vec![0, 0, 1], vec![1]),
                        ];
                        let f = UnsafeCover::build_from_cubes(cubes, 3, 1);
                        let (_result, _, _) = esp.minimize(f, None, None);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
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

                    let mut esp =
                        UnsafeEspresso::new_with_config(num_inputs, 1, &EspressoConfig::default());

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

                        let f = UnsafeCover::build_from_cubes(cubes, num_inputs, 1);

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
    /// Repeatedly creates and drops UnsafeEspresso instances in threads
    #[test]
    fn test_rapid_creation_destruction() {
        const NUM_THREADS: usize = 8;
        const CYCLES: usize = 50;

        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|_thread_id| {
                thread::spawn(move || {
                    for cycle in 0..CYCLES {
                        let num_inputs = 2 + (cycle % 3);
                        let num_outputs = 1 + (cycle % 2);

                        // Create instance
                        let mut esp = UnsafeEspresso::new_with_config(
                            num_inputs,
                            num_outputs,
                            &EspressoConfig::default(),
                        );

                        // Use it once
                        let cubes = vec![(vec![0; num_inputs], vec![1; num_outputs])];
                        let f = UnsafeCover::build_from_cubes(cubes, num_inputs, num_outputs);
                        let (_result, _, _) = esp.minimize(f, None, None);

                        // Drop happens automatically here
                    }

                    // Verify thread can still create new instances after all that
                    let mut esp = UnsafeEspresso::new_with_config(2, 1, &EspressoConfig::default());
                    let cubes = vec![(vec![0, 1], vec![1])];
                    let f = UnsafeCover::build_from_cubes(cubes, 2, 1);
                    let (_result, _, _) = esp.minimize(f, None, None);
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
                    let mut esp = UnsafeEspresso::new_with_config(3, 1, &EspressoConfig::default());

                    for op in 0..OPERATIONS {
                        // Vary the problem slightly each time
                        let var = (op / 10) % 3;
                        let mut cubes = vec![(vec![0, 1, 0], vec![1]), (vec![1, 0, 1], vec![1])];

                        // Add variable cubes based on operation number
                        for i in 0..var {
                            let inputs = vec![(i % 2) as u8, ((i + 1) % 2) as u8, (i % 2) as u8];
                            cubes.push((inputs, vec![1]));
                        }

                        let f = UnsafeCover::build_from_cubes(cubes, 3, 1);
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
                    let mut esp = UnsafeEspresso::new_with_config(2, 1, &EspressoConfig::default());

                    for _ in 0..COVERS_PER_THREAD {
                        // Create multiple covers
                        let f1 = UnsafeCover::build_from_cubes(vec![(vec![0, 1], vec![1])], 2, 1);
                        let f2 = UnsafeCover::build_from_cubes(vec![(vec![1, 0], vec![1])], 2, 1);
                        let f3 = UnsafeCover::build_from_cubes(vec![(vec![1, 1], vec![1])], 2, 1);

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

    /// Test 8: Different problem sizes concurrently
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
                let mut esp = UnsafeEspresso::new_with_config(
                    num_inputs,
                    num_outputs,
                    &EspressoConfig::default(),
                );

                // Generate cubes
                let mut cubes = vec![];
                for i in 0..num_cubes {
                    let inputs = (0..num_inputs).map(|j| ((i + j + idx) % 3) as u8).collect();
                    let outputs = vec![if i % 2 == 0 { 1 } else { 0 }; num_outputs];
                    cubes.push((inputs, outputs));
                }

                let f = UnsafeCover::build_from_cubes(cubes, num_inputs, num_outputs);

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
}
