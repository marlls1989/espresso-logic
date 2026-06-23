//! Example: Inspect cubes after minimization

use espresso_logic::Anonymous;
use espresso_logic::{Cover, CoverType, Cube, CubeType, Minimizable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cube Inspection Example ===\n");

    // Create a cover with a simple function
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);

    // Add 4 cubes that can be minimized
    println!("Adding 4 input cubes:");
    cover.push(Cube::anonymous(
        &[Some(false), Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));
    println!("  001 -> 1");
    cover.push(Cube::anonymous(
        &[Some(false), Some(true), Some(true)],
        &[true],
        CubeType::F,
    ));
    println!("  011 -> 1");
    cover.push(Cube::anonymous(
        &[Some(true), Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));
    println!("  101 -> 1");
    cover.push(Cube::anonymous(
        &[Some(true), Some(true), Some(true)],
        &[true],
        CubeType::F,
    ));
    println!("  111 -> 1");

    println!("\nBefore minimization:");
    println!("  Number of cubes: {}", cover.num_cubes());

    // Minimize
    cover = cover.minimize()?;

    println!("\nAfter minimization:");
    println!("  Number of cubes: {}", cover.num_cubes());

    // Inspect the minimized cubes
    println!("\nMinimized cubes:");
    for (i, cube) in cover.cubes().enumerate() {
        let inputs: Vec<Option<bool>> = cube.inputs().iter().collect();
        let outputs: Vec<bool> = cube.outputs().iter().collect();
        print!("  Cube {}: ", i + 1);
        for input in inputs {
            match input {
                Some(false) => print!("0"),
                Some(true) => print!("1"),
                None => print!("-"),
            }
        }
        print!(" -> ");
        for output in outputs {
            print!("{}", if output { "1" } else { "0" });
        }
        println!();
    }

    println!("\n✓ Cube inspection complete!");
    Ok(())
}
