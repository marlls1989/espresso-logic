//! Example: Inspect cubes after minimization

use espresso_logic::{Cover, CoverType};

fn main() -> std::io::Result<()> {
    println!("=== Cube Inspection Example ===\n");

    // Create a cover with a simple function
    let mut cover = Cover::new(CoverType::F);

    // Add 4 cubes that can be minimized
    println!("Adding 4 input cubes:");
    cover.add_cube(&[Some(false), Some(false), Some(true)], &[Some(true)]);
    println!("  001 -> 1");
    cover.add_cube(&[Some(false), Some(true), Some(true)], &[Some(true)]);
    println!("  011 -> 1");
    cover.add_cube(&[Some(true), Some(false), Some(true)], &[Some(true)]);
    println!("  101 -> 1");
    cover.add_cube(&[Some(true), Some(true), Some(true)], &[Some(true)]);
    println!("  111 -> 1");

    println!("\nBefore minimization:");
    println!("  Number of cubes: {}", cover.num_cubes());

    // Minimize
    cover.minimize()?;

    println!("\nAfter minimization:");
    println!("  Number of cubes: {}", cover.num_cubes());

    // Inspect the minimized cubes
    println!("\nMinimized cubes:");
    for (i, (inputs, outputs)) in cover.cubes_iter().enumerate() {
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
            print!(
                "{}",
                match output {
                    Some(true) => "1",
                    Some(false) => "0",
                    None => "-",
                }
            );
        }
        println!();
    }

    println!("\n✓ Cube inspection complete!");
    Ok(())
}
