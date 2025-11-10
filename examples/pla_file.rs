//! Example: Reading and minimizing a PLA file
//!
//! This example demonstrates how to read a PLA file, minimize it,
//! and write the result back to a file.

use espresso_logic::{Cover, CoverType, Minimizable, PLAReader, PLAWriter};
use std::env;
use std::io::Write;
use tempfile::NamedTempFile;

fn main() {
    println!("=== PLA File Minimization Example ===\n");

    // Create a sample PLA file
    let pla_content = r#".i 4
.o 1
.ilb a b c d
.ob f
.p 8
0000 1
0001 1
0010 1
0011 1
0100 1
0101 1
0110 1
0111 1
.e
"#;

    println!("Sample PLA content:");
    println!("{}", pla_content);

    // Write to a temporary file
    let mut temp_in = NamedTempFile::new().expect("Failed to create temp file");
    temp_in
        .write_all(pla_content.as_bytes())
        .expect("Failed to write temp file");
    temp_in.flush().expect("Failed to flush temp file");

    // Read the PLA file
    println!("\nReading PLA file...");
    let mut cover = match Cover::from_pla_file(temp_in.path()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading PLA: {}", e);
            return;
        }
    };

    println!("PLA loaded successfully!");
    println!("\nOriginal cover:");
    println!("  Inputs:  {}", cover.num_inputs());
    println!("  Outputs: {}", cover.num_outputs());
    println!("  Cubes:   {}", cover.num_cubes());

    // Minimize using the Cover trait
    println!("\nMinimizing using Espresso...");
    cover = match cover.minimize() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error minimizing: {}", e);
            return;
        }
    };

    println!("\nMinimized cover:");
    println!("  Inputs:  {}", cover.num_inputs());
    println!("  Outputs: {}", cover.num_outputs());
    println!("  Cubes:   {}", cover.num_cubes());

    // Show the minimized PLA
    match cover.to_pla_string(CoverType::F) {
        Ok(pla_str) => {
            println!("\nMinimized PLA:");
            println!("{}", pla_str);
        }
        Err(e) => eprintln!("Error generating PLA: {}", e),
    }

    // Write to output file if requested
    if let Some(output_path) = env::args().nth(1) {
        println!("Writing minimized PLA to: {}", output_path);
        match cover.to_pla_file(&output_path, CoverType::F) {
            Ok(_) => println!("Successfully wrote output file!"),
            Err(e) => eprintln!("Error writing output: {}", e),
        }
    } else {
        println!("\nTo write output to a file, run:");
        println!("cargo run --example pla_file output.pla");
    }
}
