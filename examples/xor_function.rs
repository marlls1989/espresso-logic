//! XOR function minimization example

use espresso_logic::{Cover, CoverType};

fn main() -> std::io::Result<()> {
    println!("XOR Function Minimization\n");

    // Create a cover (dimensions grow automatically)
    let mut cover = Cover::new(CoverType::F);

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
