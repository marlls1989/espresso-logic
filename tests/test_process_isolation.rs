//! Tests for process isolation feature
//!
//! These tests verify that the transparent API works correctly
//! with process isolation.

use espresso_logic::{Cover, CoverBuilder};
use std::thread;
use std::time::Duration;

#[test]
fn test_basic_process_isolation() {
    // Create a cover and add cubes
    let mut cover = CoverBuilder::<2, 1>::new();
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]); // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // 10 -> 1

    // Minimize using transparent process isolation
    cover.minimize().expect("Minimization failed");

    // Verify result
    assert!(cover.num_cubes() > 0, "Result should have cubes");
}

#[test]
fn test_concurrent_execution() {
    // Each thread creates its own cover - no shared state!
    let handles: Vec<_> = (0..4)
        .map(|i| {
            thread::spawn(move || {
                // Create cover (pure Rust)
                let mut cover = CoverBuilder::<2, 1>::new();
                cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
                cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

                // This works concurrently without conflicts!
                cover.minimize().expect("Minimization failed");

                let num_cubes = cover.num_cubes();
                println!("Thread {} completed with {} cubes", i, num_cubes);
                num_cubes
            })
        })
        .collect();

    // Wait for all threads to complete
    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    // All should succeed
    assert_eq!(results.len(), 4);
    for num_cubes in results {
        assert!(num_cubes > 0);
    }
}

#[test]
fn test_consistent_results() {
    // Test that multiple executions produce consistent results

    // First execution
    let mut cover1 = CoverBuilder::<3, 1>::new();
    cover1.add_cube(&[Some(false), Some(false), Some(true)], &[Some(true)]);
    cover1.add_cube(&[Some(false), Some(true), Some(false)], &[Some(true)]);
    cover1.add_cube(&[Some(true), Some(false), Some(false)], &[Some(true)]);
    cover1.minimize().unwrap();

    // Second execution
    let mut cover2 = CoverBuilder::<3, 1>::new();
    cover2.add_cube(&[Some(false), Some(false), Some(true)], &[Some(true)]);
    cover2.add_cube(&[Some(false), Some(true), Some(false)], &[Some(true)]);
    cover2.add_cube(&[Some(true), Some(false), Some(false)], &[Some(true)]);
    cover2.minimize().unwrap();

    // Results should be consistent
    assert_eq!(
        cover1.num_cubes(),
        cover2.num_cubes(),
        "Results should have same number of cubes"
    );
}

#[test]
fn test_multiple_sizes() {
    // Test that we can create instances with different sizes
    let mut cover1 = CoverBuilder::<2, 1>::new();
    cover1.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    cover1.minimize().unwrap();

    let mut cover2 = CoverBuilder::<3, 2>::new();
    cover2.add_cube(
        &[Some(true), Some(false), Some(true)],
        &[Some(true), Some(false)],
    );
    cover2.minimize().unwrap();

    assert!(cover1.num_cubes() > 0);
    assert!(cover2.num_cubes() > 0);
}

#[test]
fn test_stress_concurrent() {
    // Stress test with many concurrent operations
    let handles: Vec<_> = (0..10)
        .map(|i| {
            thread::spawn(move || {
                // Each thread performs multiple operations
                for j in 0..3 {
                    let mut cover = CoverBuilder::<2, 1>::new();
                    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
                    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

                    match cover.minimize() {
                        Ok(_) => {
                            assert!(cover.num_cubes() > 0);
                        }
                        Err(e) => {
                            eprintln!("Thread {}-{} failed: {}", i, j, e);
                            panic!("Minimization failed");
                        }
                    }

                    // Small delay to create more overlap
                    thread::sleep(Duration::from_millis(10));
                }
            })
        })
        .collect();

    // All threads should complete successfully
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}
