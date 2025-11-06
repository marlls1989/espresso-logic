//! XOR function minimization example

use espresso_logic::{Cover, CoverBuilder};

fn main() -> std::io::Result<()> {
    println!("XOR Function Minimization\n");

    // Create a cover for 2 inputs, 1 output
    let mut cover = CoverBuilder::<2, 1>::new();

    // Add XOR truth table
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]); // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // 10 -> 1

    println!("Input: 2 cubes");
    println!("  01 -> 1");
    println!("  10 -> 1\n");

    // Minimize
    cover.minimize()?;

    println!("Output: {} cubes", cover.num_cubes());
    println!("\n✓ Minimization complete!");

    Ok(())
}
