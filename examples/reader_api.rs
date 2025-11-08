//! Example: Using the Reader-Based PLA Parsing API
//!
//! This example demonstrates how to use the new `from_pla_reader` method that reads
//! directly from any `BufRead` implementation for efficient parsing.

use espresso_logic::{Cover, PLACover};
use std::io::{self, BufReader, Cursor};

fn main() -> io::Result<()> {
    println!("=== Reader-Based PLA Parsing Example ===\n");

    let pla_content = r#".i 3
.o 2
.p 4
000 11
001 10
010 01
111 11
.e
"#;

    println!("--- Method 1: Parse from string (via from_pla_content) ---");
    let cover1 = PLACover::from_pla_content(pla_content)?;
    println!("Inputs:  {}", cover1.num_inputs());
    println!("Outputs: {}", cover1.num_outputs());
    println!("Cubes:   {}", cover1.num_cubes());

    println!("\n--- Method 2: Parse from Cursor (via from_pla_reader) ---");
    let cursor = Cursor::new(pla_content.as_bytes());
    let cover2 = PLACover::from_pla_reader(cursor)?;
    println!("Inputs:  {}", cover2.num_inputs());
    println!("Outputs: {}", cover2.num_outputs());
    println!("Cubes:   {}", cover2.num_cubes());

    println!("\n--- Method 3: Parse from BufReader of bytes ---");
    let bytes = pla_content.as_bytes();
    let buf_reader = BufReader::new(bytes);
    let cover3 = PLACover::from_pla_reader(buf_reader)?;
    println!("Inputs:  {}", cover3.num_inputs());
    println!("Outputs: {}", cover3.num_outputs());
    println!("Cubes:   {}", cover3.num_cubes());

    println!("\n--- Method 4: Parse from file (via from_pla_file) ---");
    // Create a temporary file
    use std::io::Write;
    use tempfile::NamedTempFile;

    let mut temp_file = NamedTempFile::new()?;
    temp_file.write_all(pla_content.as_bytes())?;
    temp_file.flush()?;

    // from_pla_file now uses from_pla_reader internally with BufReader
    let cover4 = PLACover::from_pla_file(temp_file.path())?;
    println!("Inputs:  {}", cover4.num_inputs());
    println!("Outputs: {}", cover4.num_outputs());
    println!("Cubes:   {}", cover4.num_cubes());

    println!("\n--- Method 5: Custom reader implementation ---");
    // Example: Reading from a network stream, compressed file, etc.
    struct LoggingReader<R> {
        inner: R,
        bytes_read: usize,
    }

    impl<R: io::Read> io::Read for LoggingReader<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.inner.read(buf)?;
            self.bytes_read += n;
            Ok(n)
        }
    }

    let cursor = Cursor::new(pla_content.as_bytes());
    let mut logging_reader = LoggingReader {
        inner: cursor,
        bytes_read: 0,
    };
    let buf_reader = BufReader::new(&mut logging_reader);
    let cover5 = PLACover::from_pla_reader(buf_reader)?;
    println!(
        "Parsed cover with {} inputs, {} outputs, {} cubes",
        cover5.num_inputs(),
        cover5.num_outputs(),
        cover5.num_cubes()
    );
    println!("Total bytes read: {}", logging_reader.bytes_read);

    println!("\n✓ All parsing methods work correctly!");
    println!("\nBenefits of from_pla_reader:");
    println!("  - No need to load entire file into memory first");
    println!("  - Can read from any BufRead source (files, network, stdin, etc.)");
    println!("  - Better memory efficiency for large PLA files");
    println!("  - Composable with other readers (decompression, decryption, etc.)");
    println!("  - from_pla_file now uses buffered reading internally");

    Ok(())
}
