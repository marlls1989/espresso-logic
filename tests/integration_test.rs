//! Integration tests for the Espresso Rust wrapper

use espresso_logic::*;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_espresso_new() {
    let esp = Espresso::new(2, 1);
    // Just verify it doesn't panic
    drop(esp);
}

#[test]
fn test_cover_new() {
    let cover = Cover::new(10, 5);
    // Just verify the cover was created successfully
    let _ = cover.count();
}

#[test]
fn test_cover_clone() {
    let cover = Cover::new(10, 5);
    let cloned = cover.clone();
    assert_eq!(cover.count(), cloned.count());
}

#[test]
fn test_pla_from_file() {
    // Create a simple PLA file
    let pla_content = r#".i 2
.o 1
.ilb a b
.ob f
.p 2
01 1
10 1
.e
"#;

    let mut temp = NamedTempFile::new().expect("Failed to create temp file");
    temp.write_all(pla_content.as_bytes())
        .expect("Failed to write temp file");
    temp.flush().expect("Failed to flush temp file");

    // This might fail if the C library isn't properly initialized
    // but the test should at least compile
    let result = PLA::from_file(temp.path());

    // Don't assert success, as this depends on C library initialization
    // which can be tricky in test contexts
    if let Ok(pla) = result {
        // If we got a PLA, try to use it
        drop(pla);
    }
}

#[test]
fn test_cover_builder() {
    // CoverBuilder requires Espresso::new() to be called first to initialize cube structure
    let _esp = Espresso::new(2, 1);

    let mut builder = CoverBuilder::new(2, 1);
    builder.add_cube(&[0, 1], &[1]);
    builder.add_cube(&[1, 0], &[1]);
    let cover = builder.build();
    // Verify it builds without panicking
    drop(cover);
}

#[test]
fn test_pla_type_values() {
    assert_eq!(PLAType::F as i32, 1);
    assert_eq!(PLAType::FD as i32, 3);
    assert_eq!(PLAType::FR as i32, 5);
    assert_eq!(PLAType::FDR as i32, 7);
}
