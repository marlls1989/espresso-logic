//! Example demonstrating the direct Espresso API
//!
//! This shows how to use the low-level espresso module for direct access
//! to the Espresso algorithm with maximum control and performance.

use espresso_logic::espresso::EspressoCover;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Direct Espresso API Example ===\n");

    // Build a cover for XOR function: 01->1, 10->1
    // The Espresso instance is created automatically
    let cubes = [
        (&[0, 1][..], &[1][..]), // Input: 01, Output: 1
        (&[1, 0][..], &[1][..]), // Input: 10, Output: 1
    ];

    println!("Input cover (XOR function):");
    println!("  01 -> 1");
    println!("  10 -> 1");
    println!();

    let f = EspressoCover::from_cubes(&cubes, 2, 1)?;

    // Minimize directly on the cover
    let (minimized, _d, _r) = f.minimize(None, None);

    // Extract and display results
    let result_cubes = minimized.to_cubes(2, 1, espresso_logic::espresso::CubeType::F);

    println!("Minimized cover ({} cubes):", result_cubes.len());
    for cube in &result_cubes {
        print!("  ");
        for input in cube.inputs() {
            match input {
                Some(false) => print!("0"),
                Some(true) => print!("1"),
                None => print!("-"),
            }
        }
        print!(" -> ");
        for output in cube.outputs() {
            print!("{}", if *output { "1" } else { "0" });
        }
        println!();
    }

    println!("\n=== Multi-threaded Example ===\n");

    // Demonstrate thread safety - each thread gets its own Espresso instance automatically
    use std::thread;

    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            thread::spawn(
                move || -> Result<(usize, usize), Box<dyn std::error::Error + Send + Sync>> {
                    // No need to create Espresso instance - it's automatic!
                    let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
                    let f = EspressoCover::from_cubes(&cubes, 2, 1)?;
                    let (result, _, _) = f.minimize(None, None);
                    let result_cubes = result.to_cubes(2, 1, espresso_logic::espresso::CubeType::F);
                    Ok((thread_id, result_cubes.len()))
                },
            )
        })
        .collect();

    for handle in handles {
        let (thread_id, count) = handle
            .join()
            .unwrap()
            .map_err(|e| -> Box<dyn std::error::Error> { e })?;
        println!("Thread {} minimized to {} cubes", thread_id, count);
    }

    Ok(())
}
