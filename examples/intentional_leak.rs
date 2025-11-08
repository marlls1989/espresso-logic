//! Intentional memory leak example - FOR TESTING LEAK DETECTION ONLY
//!
//! ⚠️  WARNING: This program INTENTIONALLY leaks memory! ⚠️
//!
//! Purpose: Validate that our leak detection methodology actually works.
//! If your leak detection tools DON'T report leaks from this program, they're not working!
//!
//! Usage:
//!   cargo build --example intentional_leak --release
//!   
//! macOS:
//!   leaks --atExit -- ./target/release/examples/intentional_leak
//!   (Should report leaks from C malloc)
//!   
//! Linux:
//!   valgrind --leak-check=full ./target/release/examples/intentional_leak
//!   (Should report definitely lost bytes)
//!
//! Expected result: LEAK DETECTED (this proves leak detection works)

use espresso_logic::espresso::{CubeType, Espresso, EspressoCover};
use espresso_logic::EspressoConfig;
use std::mem;

extern "C" {
    fn malloc(size: usize) -> *mut u8;
}

fn main() {
    println!("=== INTENTIONAL MEMORY LEAK TEST ===");
    println!("⚠️  This program INTENTIONALLY leaks memory to validate leak detection! ⚠️");
    println!();

    // Leak 1: Direct C malloc without free (leak detectors MUST catch this)
    println!("Creating intentional leak 1: C malloc without free");
    unsafe {
        for _ in 0..100 {
            let ptr = malloc(1024); // Allocate 1KB
            if !ptr.is_null() {
                // Write to it to ensure it's not optimized away
                *ptr = 42;
            }
            // Intentionally not calling free()
        }
    }
    println!("  Leaked: ~100KB via C malloc (NO free)");
    println!("  ✓ This MUST be detected by leak tools");

    // Leak 2: Forget Rust-allocated memory (may or may not be detected)
    println!();
    println!("Creating intentional leak 2: Forgetting heap-allocated Vec");
    let leak1 = vec![1u8; 10000]; // 10KB
    mem::forget(leak1);
    println!("  Leaked: ~10KB via std::mem::forget");
    println!("  ⚠️  May not be detected (known limitation)");

    // Leak 3: Box::into_raw without freeing (may or may not be detected)
    println!();
    println!("Creating intentional leak 3: Box::into_raw without free");
    for _ in 0..100 {
        let leak2 = Box::new([0u64; 100]); // ~800 bytes each
        let _raw = Box::into_raw(leak2);
        // Intentionally not calling Box::from_raw to free
    }
    println!("  Leaked: ~80KB via Box::into_raw");
    println!("  ⚠️  May not be detected (known limitation)");

    // Leak 4: Leak through our library (EspressoCover with C memory)
    println!();
    println!("Creating intentional leak 4: Forgetting EspressoCover (C memory)");
    let _esp = Espresso::new(2, 1, &EspressoConfig::default());
    let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
    let cover = EspressoCover::from_cubes(cubes, 2, 1).unwrap();

    // Use it first to prove it's valid
    let _cubes = cover.to_cubes(2, 1, CubeType::F);

    // Now leak it
    mem::forget(cover);
    println!("  Leaked: C-allocated memory via mem::forget(EspressoCover)");
    println!("  ⚠️  May not be detected (known limitation)");

    println!();
    println!("=== LEAKS CREATED ===");
    println!("Total intentional leaks: ~200KB+");
    println!();
    println!("VALIDATION CRITERIA:");
    println!("✅ MUST detect: Leak 1 (C malloc without free) - ~100KB");
    println!("⚠️  MAY detect: Leaks 2-4 (mem::forget patterns) - ~90KB");
    println!();
    println!("If leak 1 is detected, leak detection is working!");
    println!("If NO leaks are detected at all, the tool is not working.");
    println!();
}
