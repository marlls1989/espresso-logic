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
        let cubes = [(&[0, 1][..], &[1][..]), (&[1, 0][..], &[1][..])];
        let f = EspressoCover::from_cubes(&cubes, 2, 1).unwrap();
        let (result, d, r) = esp.minimize(&f, None, None);

        let _ = result.to_cubes(2, 1, CubeType::F);
        let _ = d.to_cubes(2, 1, CubeType::F);
        let _ = r.to_cubes(2, 1, CubeType::F);

        if i % 1000 == 0 {
            println!("  Completed {} iterations", i);
        }
    }

    // macOS attach-mode leak harness: park as a live process so it can be scanned with
    // `leaks <pid>`, then exit cleanly when released. (No-op without the env var, so a direct run or
    // Linux/valgrind still runs straight through.)
    park_for_leak_scan();

    println!("Done. Check for leaks.");
}

/// When `ESPRESSO_LEAK_PARK` is set, print a `READY <pid>` marker and block on stdin so the macOS
/// `scripts/check_leaks_macos.sh` harness can scan this *live* process (`leaks <pid>`) and then
/// release it. macOS 26's `leaks --atExit` cannot resume a restricted process after its exit-time
/// SIGSTOP, leaving it orphaned and wedging the run — attaching to a live process avoids that.
fn park_for_leak_scan() {
    use std::io::Write;
    if std::env::var_os("ESPRESSO_LEAK_PARK").is_none() {
        return;
    }
    println!("READY {}", std::process::id());
    let _ = std::io::stdout().flush();
    let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);
}
