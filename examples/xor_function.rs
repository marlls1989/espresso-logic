//! XOR function minimisation example

use espresso_logic::Anonymous;
use espresso_logic::{Cover, CoverType, Cube, CubeType, Minimizable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("XOR Function Minimization\n");

    // Create a cover (dimensions grow automatically)
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);

    // Add XOR truth table
    cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    )); // 01 -> 1
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    )); // 10 -> 1

    println!("Input: 2 cubes");
    println!("  01 -> 1");
    println!("  10 -> 1\n");

    // Minimize
    cover = cover.minimize()?;

    println!("Output: {} cubes", cover.num_cubes());
    println!("\n✓ Minimization complete!");

    Ok(())
}
