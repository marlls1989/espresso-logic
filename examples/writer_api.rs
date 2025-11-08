//! Example: Using the Writer-Based PLA Serialization API
//!
//! This example demonstrates how to use the new `write_pla` method that writes
//! directly to any `Write` implementation for efficient serialization.

use espresso_logic::{Cover, PLACover};
use std::io::{self, Write};

fn main() -> io::Result<()> {
    println!("=== Writer-Based PLA Serialization Example ===\n");

    // Parse from PLA content
    let pla_content = r#".i 3
.o 2
.p 4
000 11
001 10
010 01
111 11
.e
"#;

    let cover = PLACover::from_pla_content(pla_content)?;

    println!("Original cover:");
    println!("  Inputs:  {}", cover.num_inputs());
    println!("  Outputs: {}", cover.num_outputs());
    println!("  Cubes:   {}", cover.num_cubes());

    println!("\n--- Method 1: Write to stdout directly ---");
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    cover.write_pla(&mut handle, espresso_logic::PLAType::F)?;
    drop(handle); // Release the lock

    println!("\n--- Method 2: Write to a Vec<u8> buffer ---");
    let mut buffer = Vec::new();
    cover.write_pla(&mut buffer, espresso_logic::PLAType::F)?;
    println!("Buffer size: {} bytes", buffer.len());
    println!("Buffer content:\n{}", String::from_utf8_lossy(&buffer));

    println!("--- Method 3: Write to a custom writer ---");
    struct CountingWriter {
        count: usize,
    }

    impl Write for CountingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.count += buf.len();
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut counter = CountingWriter { count: 0 };
    cover.write_pla(&mut counter, espresso_logic::PLAType::F)?;
    println!("Total bytes written: {}", counter.count);

    println!("\n--- Comparison with to_pla_string() ---");
    let string_result = cover.to_pla_string(espresso_logic::PLAType::F)?;
    println!("String method length: {} bytes", string_result.len());
    println!("Writer buffer length: {} bytes", buffer.len());
    println!(
        "Results match: {}",
        string_result.as_bytes() == buffer.as_slice()
    );

    println!("\n✓ All methods work correctly!");
    println!("\nBenefits of write_pla:");
    println!("  - No intermediate string allocation for file writing");
    println!("  - Can write to any Write implementation (files, network, etc.)");
    println!("  - Better memory efficiency for large covers");
    println!("  - Composable with other writers (compression, encryption, etc.)");

    Ok(())
}
