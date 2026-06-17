//! Example: Concurrent execution with transparent process isolation

use espresso_logic::{Cover, CoverType, Cube, CubeType, Minimizable};
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Concurrent Transparent API Test ===\n");

    // Note: With Cover::new(), each thread creates its own cover
    // No shared state needed!
    let handles: Vec<_> = (0..4)
        .map(|i| {
            thread::spawn(move || {
                // Each thread creates its own cover
                let mut cover = Cover::<(), ()>::anonymous(CoverType::F);
                cover.push(Cube::anonymous(
                    &[Some(false), Some(true)],
                    &[true],
                    CubeType::F,
                ));
                cover.push(Cube::anonymous(
                    &[Some(true), Some(false)],
                    &[true],
                    CubeType::F,
                ));

                // Minimize in isolated process
                cover = cover.minimize().expect("Minimization failed");

                let num_cubes = cover.num_cubes();
                println!("Thread {} completed with {} cubes", i, num_cubes);
                num_cubes
            })
        })
        .collect();

    // Wait for all threads
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    println!("\n✓ All {} threads completed successfully!", results.len());
    println!("Results: {:?}", results);

    Ok(())
}
