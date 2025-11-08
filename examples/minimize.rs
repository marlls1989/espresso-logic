//! Basic minimization example

use espresso_logic::{Cover, CoverType};

fn main() -> std::io::Result<()> {
    println!("Boolean Function Minimization Example\n");

    println!("Creating a 3-input, 1-output Boolean function");
    println!("Function: F = A'B'C + A'BC + AB'C + ABC");
    println!("(output is 1 when an odd number of inputs are 1)\n");

    // Build the ON-set (truth table where output is 1)
    let mut cover = Cover::new(CoverType::F);

    // A'B'C (001)
    cover.add_cube(&[Some(false), Some(false), Some(true)], &[Some(true)]);

    // A'BC (011)
    cover.add_cube(&[Some(false), Some(true), Some(true)], &[Some(true)]);

    // AB'C (101)
    cover.add_cube(&[Some(true), Some(false), Some(true)], &[Some(true)]);

    // ABC (111)
    cover.add_cube(&[Some(true), Some(true), Some(true)], &[Some(true)]);

    // Minimize the function - all C code runs in isolated process
    println!("Minimizing using Espresso algorithm...");
    cover.minimize()?;

    println!("\nMinimized to {} cubes", cover.num_cubes());
    println!("\nThe minimized function should be equivalent but with fewer cubes.");

    Ok(())
}
