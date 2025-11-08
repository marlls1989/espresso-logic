//! Leak detection example - run with valgrind/leaks/instruments
//!
//! Usage:
//!   cargo build --example leak_check --release
//!   
//! macOS:
//!   leaks --atExit -- ./target/release/examples/leak_check
//!   
//! Linux:
//!   valgrind --leak-check=full ./target/release/examples/leak_check

use espresso_logic::espresso::{CubeType, Espresso, EspressoCover};
use espresso_logic::EspressoConfig;

fn main() {
    println!("Running leak detection test...");
    println!("Iterations: 10,000");

    let esp = Espresso::new(2, 1, &EspressoConfig::default());

    for i in 0..10000 {
        let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
        let f = EspressoCover::from_cubes(cubes, 2, 1).unwrap();
        let (result, d, r) = esp.minimize(f, None, None);

        let _ = result.to_cubes(2, 1, CubeType::F);
        let _ = d.to_cubes(2, 1, CubeType::F);
        let _ = r.to_cubes(2, 1, CubeType::F);

        if i % 1000 == 0 {
            println!("  Completed {} iterations", i);
        }
    }

    println!("Done. Check for leaks.");
}
