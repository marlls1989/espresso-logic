//! Example: Concurrent execution with transparent process isolation

use espresso_logic::{Cover, CoverBuilder};
use std::sync::Arc;
use std::thread;

fn main() -> std::io::Result<()> {
    println!("=== Concurrent Transparent API Test ===\n");

    // Note: With Cover::new(), each thread creates its own cover
    // No shared state needed!
    let handles: Vec<_> = (0..4)
        .map(|i| {
            thread::spawn(move || {
                // Each thread creates its own cover
                let mut cover = CoverBuilder::<2, 1>::new();
                cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
                cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

                // Minimize in isolated process
                cover.minimize().expect("Minimization failed");

                let num_cubes = cover.num_cubes();
                println!("Thread {} completed with {} cubes", i, num_cubes);
                num_cubes
            })
        })
        .collect();

    // Wait for all threads
    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    println!("\n✓ All {} threads completed successfully!", results.len());
    println!("Results: {:?}", results);

    Ok(())
}
