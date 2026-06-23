//! Basic minimisation example

use espresso_logic::Anonymous;
use espresso_logic::{Cover, CoverType, Cube, CubeType, Minimizable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Boolean Function Minimization Example\n");

    println!("Creating a 3-input, 1-output Boolean function");
    println!("Function: F = A'B'C + A'BC + AB'C + ABC");
    println!("(output is 1 when an odd number of inputs are 1)\n");

    // Build the ON-set (truth table where output is 1)
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);

    // A'B'C (001)
    cover.push(Cube::anonymous(
        &[Some(false), Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));

    // A'BC (011)
    cover.push(Cube::anonymous(
        &[Some(false), Some(true), Some(true)],
        &[true],
        CubeType::F,
    ));

    // AB'C (101)
    cover.push(Cube::anonymous(
        &[Some(true), Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));

    // ABC (111)
    cover.push(Cube::anonymous(
        &[Some(true), Some(true), Some(true)],
        &[true],
        CubeType::F,
    ));

    // Minimize the function - all C code runs in isolated process
    println!("Minimizing using Espresso algorithm...");
    cover = cover.minimize()?;

    println!("\nMinimized to {} cubes", cover.num_cubes());
    println!("\nThe minimized function should be equivalent but with fewer cubes.");

    Ok(())
}
