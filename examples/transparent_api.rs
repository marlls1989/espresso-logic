//! Example: Transparent Process Isolation API
//!
//! This demonstrates the modern API where ALL C code
//! runs in isolated processes - nothing in the parent process.

use espresso_logic::{Cover, CoverBuilder};

fn main() -> std::io::Result<()> {
    println!("=== Transparent Process Isolation API ===\n");

    println!("✓ No global state in parent process");
    println!("✓ All C code runs in isolated workers");
    println!("✓ Thread-safe by default\n");

    // Create a cover for 2 inputs, 1 output - NO C code executed!
    println!("1. Creating Cover (no C code yet)");
    let mut cover = CoverBuilder::<2, 1>::new();
    println!("   ✓ Cover created (pure Rust data structure)\n");

    // Add cubes (XOR function)
    println!("2. Adding cubes to cover");
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]); // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);  // 10 -> 1
    println!("   ✓ Cubes added (pure Rust, no C code)\n");

    // Minimize - THIS is when the worker process runs
    println!("3. Minimizing (spawns worker process)");
    println!("   - Worker forks");
    println!("   - Worker initializes C cube structure");
    println!("   - Worker builds Cover from cube data");
    println!("   - Worker runs minimization");
    println!("   - Worker returns result via shared memory");
    println!("   - Worker terminates");
    
    cover.minimize()?;
    println!("   ✓ Success! Result has {} cubes\n", cover.num_cubes());

    println!("=== Key Benefits ===");
    println!("• Parent process never touches C code");
    println!("• No global state in parent");
    println!("• Thread-safe without any synchronization");
    println!("• Simple, clean API with const generics\n");

    println!("✓ Complete process isolation achieved!");
    Ok(())
}
