//! Example: Minimizing an XOR function
//!
//! This example shows how to minimize a 2-input XOR function
//! which is a classic example in Boolean logic.

use espresso_logic::{CoverBuilder, Espresso};

fn main() {
    println!("=== XOR Function Minimization ===\n");

    // XOR truth table:
    // A B | F
    // ----+---
    // 0 0 | 0
    // 0 1 | 1  <- ON-set
    // 1 0 | 1  <- ON-set
    // 1 1 | 0

    let mut esp = Espresso::new(2, 1);

    println!("Minimizing 2-input XOR function: F = A ⊕ B");
    println!("Truth table:");
    println!("  A B | F");
    println!("  ----+---");
    println!("  0 0 | 0");
    println!("  0 1 | 1");
    println!("  1 0 | 1");
    println!("  1 1 | 0");
    println!();

    // Build the ON-set
    let mut builder = CoverBuilder::new(2, 1);

    // 01 -> 1
    builder.add_cube(&[0, 1], &[1]);

    // 10 -> 1
    builder.add_cube(&[1, 0], &[1]);

    let cover = builder.build();

    println!("Input cover has {} cubes", cover.count());

    // Minimize
    println!("\nMinimizing...");
    let minimized = esp.minimize(cover, None, None);

    println!("Minimized cover has {} cubes", minimized.count());
    println!("\nNote: XOR cannot be reduced to a single product term,");
    println!("so the minimized form should still have 2 cubes.");
}
