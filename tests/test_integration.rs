//! Integration tests for the Espresso Rust wrapper
//!
//! These tests verify end-to-end functionality including file I/O,
//! PLA format handling, and complete minimization workflows.

use espresso_logic::*;
use std::io::Write;
use tempfile::NamedTempFile;

// Basic cover creation tests

#[test]
fn test_cover_new() {
    // Test that new covers are empty and functional
    let cover = Cover::new(CoverType::F);
    assert_eq!(cover.num_cubes(), 0, "New cover should start with 0 cubes");

    // Test with different dimensions
    let cover3x1 = Cover::new(CoverType::F);
    assert_eq!(
        cover3x1.num_cubes(),
        0,
        "New 3x1 cover should start with 0 cubes"
    );

    let cover2x2 = Cover::new(CoverType::F);
    assert_eq!(
        cover2x2.num_cubes(),
        0,
        "New 2x2 cover should start with 0 cubes"
    );

    // Verify they can be dropped without issues
    drop(cover);
    drop(cover3x1);
    drop(cover2x2);
}

#[test]
fn test_cover_builder() {
    // Create cover
    let mut cover = Cover::new(CoverType::F);
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

    // Minimize
    cover.minimize().unwrap();
    assert!(cover.num_cubes() > 0);
}

// PLA file I/O tests

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

    // Test with new Cover API
    let result = Cover::from_pla_file(temp.path());

    // Should successfully parse the PLA file
    assert!(result.is_ok());
    if let Ok(cover) = result {
        assert_eq!(cover.num_cubes(), 2); // 2 cubes in the PLA
    }
}

#[test]
fn test_create_cover_from_pla() {
    // Create PLA content programmatically for XOR function
    let pla_str = ".i 2\n.o 1\n.p 2\n01 1\n10 1\n.e\n";

    let mut cover = Cover::from_pla_string(pla_str).expect("Failed to parse PLA");
    assert_eq!(cover.num_cubes(), 2);

    cover.minimize().unwrap();

    // XOR cannot be minimized
    assert_eq!(cover.num_cubes(), 2);
}

#[test]
fn test_pla_roundtrip() {
    // Create a cover programmatically
    let mut cover = Cover::new(CoverType::F);
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]); // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // 10 -> 1

    // Convert to PLA format using the trait
    let pla_str =
        <Cover as PLAWriter>::to_pla_string(&cover, CoverType::F).expect("Failed to serialize");

    // Parse it back using Cover
    let mut parsed_cover = Cover::from_pla_string(&pla_str).expect("Failed to parse");
    assert_eq!(parsed_cover.num_cubes(), 2);

    // Minimize and verify XOR cannot be reduced
    parsed_cover.minimize().unwrap();
    assert_eq!(parsed_cover.num_cubes(), 2);
}

// PLA type enum tests

#[test]
fn test_pla_type_values() {
    assert_eq!(CoverType::F as i32, 1);
    assert_eq!(CoverType::FD as i32, 3);
    assert_eq!(CoverType::FR as i32, 5);
    assert_eq!(CoverType::FDR as i32, 7);
}
