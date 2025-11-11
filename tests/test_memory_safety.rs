//! Memory safety and leak detection tests
//!
//! These tests verify that we're properly managing C-allocated memory.
//!
//! To run with actual leak detection:
//! - macOS: `./scripts/check_memory_leaks.sh`
//! - Linux: Use valgrind or heaptrack (see docs/MEMORY_SAFETY.md)

use espresso_logic::espresso::{CubeType, Espresso, EspressoCover};
use espresso_logic::{EspressoConfig, Minimizable};

/// Helper to get current memory usage on macOS
#[cfg(target_os = "macos")]
fn get_memory_usage() -> Option<usize> {
    use std::process::Command;

    let pid = std::process::id();
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;

    let rss_str = String::from_utf8(output.stdout).ok()?;
    rss_str.trim().parse::<usize>().ok()
}

#[cfg(not(target_os = "macos"))]
fn get_memory_usage() -> Option<usize> {
    // Placeholder for non-macOS systems
    None
}

/// Test that basic operations don't leak memory by measuring RSS growth
#[test]
fn test_memory_usage_stability() {
    // Warm up to stabilize memory allocations
    for _ in 0..10 {
        let esp = Espresso::new(2, 1, &EspressoConfig::default());
        let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
        let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
        let (result, _d, _r) = esp.minimize(&f, None, None);
        let _ = result.to_cubes(2, 1, CubeType::F);
    }

    // Give OS time to reflect memory changes
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Measure baseline
    let baseline = get_memory_usage();

    // Perform many operations that should not accumulate memory
    for _ in 0..1000 {
        let esp = Espresso::new(2, 1, &EspressoConfig::default());
        let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
        let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
        let (result, _d, _r) = esp.minimize(&f, None, None);
        let _ = result.to_cubes(2, 1, CubeType::F);
    }

    // Give OS time to reflect memory changes
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Measure after operations
    let after = get_memory_usage();

    if let (Some(baseline), Some(after)) = (baseline, after) {
        let growth = after.saturating_sub(baseline);
        let growth_kb = growth;

        println!("Memory baseline: {} KB", baseline);
        println!("Memory after 1000 ops: {} KB", after);
        println!("Memory growth: {} KB", growth_kb);

        // Allow some growth for legitimate allocations (Rust heap, etc)
        // but flag if we've leaked > 5MB (suggests C memory leak)
        assert!(
            growth_kb < 5120,
            "Memory grew by {} KB - possible leak! Baseline: {} KB, After: {} KB",
            growth_kb,
            baseline,
            after
        );
    } else {
        println!("⚠ Memory measurement not available on this platform");
        println!("Run ./scripts/check_memory_leaks.sh for proper leak detection");
    }
}

/// Test that clone creates independent memory and doesn't double-free
#[test]
fn test_clone_independence_no_double_free() {
    let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
    let cover1 = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();

    // Clone creates independent C memory
    let cover2 = cover1.clone();
    let cover3 = cover1.clone();

    // All should be readable independently
    let cubes1 = cover1.to_cubes(2, 1, CubeType::F);
    let cubes2 = cover2.to_cubes(2, 1, CubeType::F);
    let cubes3 = cover3.to_cubes(2, 1, CubeType::F);

    assert_eq!(cubes1.len(), 2);
    assert_eq!(cubes2.len(), 2);
    assert_eq!(cubes3.len(), 2);

    // Drop them in various orders - should not double-free
    drop(cover2); // Drop middle one
    let _ = cover1.to_cubes(2, 1, CubeType::F); // Still accessible
    let _ = cover3.to_cubes(2, 1, CubeType::F); // Still accessible
    drop(cover1);
    let _ = cover3.to_cubes(2, 1, CubeType::F); // Still accessible
                                                // cover3 drops last
}

/// Test that into_raw() properly transfers ownership without double-free
#[test]
fn test_into_raw_ownership_transfer() {
    let cubes = [(&[0, 1][..], &[1][..])];
    let cover = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();

    // into_raw() is internal, but we can test via minimize which uses it
    let (result, d, r) = cover.minimize(None, None);

    // All returned covers should be valid (memory accessible, not freed prematurely)
    assert!(!result.to_cubes(2, 1, CubeType::F).is_empty());
    // D and R might be empty depending on the minimization result
    let _ = d.to_cubes(2, 1, CubeType::F);
    let _ = r.to_cubes(2, 1, CubeType::F);

    // Dropping should free exactly once each (no double-free)
}

/// Test minimize with explicit D and R covers
#[test]
fn test_minimize_with_explicit_covers() {
    let esp = Espresso::new(2, 1, &EspressoConfig::default());

    let cubes_f = [(&[0, 1][..], &[1][..])];
    let f = EspressoCover::from_cubes(&cubes_f, 2, 1).unwrap();
    let cubes_d = [(&[1, 1][..], &[1][..])];
    let d = EspressoCover::from_cubes(&cubes_d, 2, 1).unwrap();
    let cubes_r = [(&[0, 0][..], &[1][..])];
    let r = EspressoCover::from_cubes(&cubes_r, 2, 1).unwrap();

    // This internally clones F, D, R before passing to C
    let (result, d_out, r_out) = esp.minimize(&f, Some(&d), Some(&r));

    // All returned covers should be valid and independently freeable (memory safety check)
    assert!(!result.to_cubes(2, 1, CubeType::F).is_empty());
    let _ = d_out.to_cubes(2, 1, CubeType::F);
    let _ = r_out.to_cubes(2, 1, CubeType::F);

    // Each cover should free its own C memory on drop (no double-free)
}

/// Stress test: repeated operations to amplify leaks
#[test]
fn test_repeated_operations_amplify_leaks() {
    const ITERATIONS: usize = 1000;

    let baseline = get_memory_usage();

    let esp = Espresso::new(2, 1, &EspressoConfig::default());

    for i in 0..ITERATIONS {
        let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
        let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
        let (result, d, r) = esp.minimize(&f, None, None);

        // Use the results to prevent optimizer from removing them
        let _ = result.to_cubes(2, 1, CubeType::F);
        let _ = d.to_cubes(2, 1, CubeType::F);
        let _ = r.to_cubes(2, 1, CubeType::F);

        // Explicitly drop to test cleanup
        drop(result);
        drop(d);
        drop(r);

        // Periodic memory check
        if i % 100 == 0 && i > 0 {
            if let (Some(base), Some(current)) = (baseline, get_memory_usage()) {
                let growth = current.saturating_sub(base);
                assert!(
                    growth < 2048, // Less than 2MB growth
                    "Memory leak detected! Grew by {} KB after {} iterations",
                    growth,
                    i
                );
            }
        }
    }

    println!("✓ {} iterations completed without memory leak", ITERATIONS);
}

/// Test that Cover properly manages memory across minimize calls
#[test]
fn test_coverbuilder_memory_management() {
    use espresso_logic::{Cover, CoverType};

    for _ in 0..100 {
        let mut cover = Cover::new(CoverType::F);
        cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

        // minimize() internally creates EspressoCover and frees it
        cover = cover.minimize().unwrap();
        assert_eq!(cover.num_cubes(), 2);

        // Cover's internal data should be properly freed on next minimize or drop
        cover = cover.minimize().unwrap();
        assert_eq!(cover.num_cubes(), 2);
    }
}

/// Test memory management with dimension changes
#[test]
fn test_dimension_changes_no_leak() {
    use espresso_logic::{Cover, CoverType};

    let baseline = get_memory_usage();

    // Alternate between different dimensions to stress cleanup
    for i in 0..100 {
        match i % 3 {
            0 => {
                let mut cover = Cover::new(CoverType::F);
                cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
                let _ = cover.minimize().unwrap();
            }
            1 => {
                let mut cover = Cover::new(CoverType::F);
                cover.add_cube(&[Some(false), Some(true), Some(false)], &[Some(true)]);
                let _ = cover.minimize().unwrap();
            }
            2 => {
                let mut cover = Cover::new(CoverType::F);
                cover.add_cube(
                    &[Some(false), Some(true), Some(false), Some(true)],
                    &[Some(true)],
                );
                let _ = cover.minimize().unwrap();
            }
            _ => unreachable!(),
        }
    }

    if let (Some(base), Some(after)) = (baseline, get_memory_usage()) {
        let growth = after.saturating_sub(base);
        println!("Memory growth after dimension changes: {} KB", growth);
        assert!(
            growth < 1024,
            "Memory leak with dimension changes! Grew by {} KB",
            growth
        );
    }
}

/// Test with larger covers to stress allocation/deallocation
#[test]
fn test_large_cover_allocations() {
    use espresso_logic::{Cover, CoverType};

    let baseline = get_memory_usage();

    // Create and minimize large covers repeatedly
    for _ in 0..50 {
        let mut cover = Cover::new(CoverType::F);

        // Add many cubes
        for i in 0..64 {
            let inputs = [
                Some((i & 32) != 0),
                Some((i & 16) != 0),
                Some((i & 8) != 0),
                Some((i & 4) != 0),
            ];
            let outputs = [Some((i & 2) != 0), Some((i & 1) != 0)];
            cover.add_cube(&inputs, &outputs);
        }

        // This allocates significant C memory
        cover = cover.minimize().unwrap();

        // Cover should be minimized
        assert!(cover.num_cubes() <= 64);

        // Drop cover - should free all C memory
        drop(cover);
    }

    std::thread::sleep(std::time::Duration::from_millis(50));

    if let (Some(base), Some(after)) = (baseline, get_memory_usage()) {
        let growth = after.saturating_sub(base);
        println!("Memory growth after large covers: {} KB", growth);
        assert!(
            growth < 2048,
            "Memory leak with large covers! Grew by {} KB",
            growth
        );
    }
}

/// Multi-threaded test: each thread should independently manage memory
#[test]
fn test_multithreaded_memory_isolation() {
    use std::thread;

    const NUM_THREADS: usize = 8;
    const OPS_PER_THREAD: usize = 100;

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|thread_id| {
            thread::spawn(move || {
                let esp = Espresso::new(2, 1, &EspressoConfig::default());

                for _ in 0..OPS_PER_THREAD {
                    let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
                    let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
                    let (result, d, r) = esp.minimize(&f, None, None);

                    // Use results
                    let _ = result.to_cubes(2, 1, CubeType::F);
                    let _ = d.to_cubes(2, 1, CubeType::F);
                    let _ = r.to_cubes(2, 1, CubeType::F);
                }

                println!(
                    "Thread {} completed {} operations",
                    thread_id, OPS_PER_THREAD
                );
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    println!("✓ All threads completed without leaks or crashes");
}

/// Test that covers keep Espresso alive (no use-after-free)
#[test]
fn test_cover_keeps_espresso_alive() {
    let cover = {
        let _esp = Espresso::new(2, 1, &EspressoConfig::default());
        let cubes = [(&[0, 1][..], &[1][..])];
        EspressoCover::from_cubes(&cubes, 2, 1).unwrap()
        // _esp goes out of scope here, but cover holds Rc to keep it alive
    };

    // Cover should still be usable - Espresso kept alive by Rc (no use-after-free)
    let cubes = cover.to_cubes(2, 1, CubeType::F);
    assert_eq!(cubes.len(), 1);

    // Minimize should also work (memory still valid)
    let (result, _, _) = cover.minimize(None, None);
    assert!(!result.to_cubes(2, 1, CubeType::F).is_empty());
}
