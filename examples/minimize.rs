//! Example: Basic minimization using the Espresso algorithm
//!
//! This example demonstrates how to create a simple Boolean function
//! and minimize it using Espresso.

use espresso_logic::{CoverBuilder, Espresso};

fn main() {
    println!("=== Espresso Logic Minimizer Example ===\n");

    // Create an Espresso instance for a 3-input, 1-output function
    let mut esp = Espresso::new(3, 1);

    println!("Creating a 3-input, 1-output Boolean function");
    println!("Function: F = A'B'C + A'BC + AB'C + ABC");
    println!("(output is 1 when an odd number of inputs are 1)\n");

    // Build the ON-set (truth table where output is 1)
    let mut builder = CoverBuilder::new(3, 1);

    // A'B'C (001)
    builder.add_cube(&[0, 0, 1], &[1]);

    // A'BC (011)
    builder.add_cube(&[0, 1, 1], &[1]);

    // AB'C (101)
    builder.add_cube(&[1, 0, 1], &[1]);

    // ABC (111)
    builder.add_cube(&[1, 1, 1], &[1]);

    let cover = builder.build();

    println!("Input cover: {:?}", cover);

    // Minimize the function
    println!("\nMinimizing using Espresso algorithm...");
    let minimized = esp.minimize(cover, None, None);

    println!("\nMinimized cover: {:?}", minimized);
    println!("\nThe minimized function should be equivalent but with fewer cubes.");
}
