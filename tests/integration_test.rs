//! Integration tests for the Espresso Rust wrapper

use espresso_logic::*;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_cover_new() {
    let cover = CoverBuilder::<2, 1>::new();
    // Just verify it doesn't panic
    drop(cover);
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

    // Test with new Cover API
    let result = espresso_logic::PLACover::from_pla_file(temp.path());

    // Should successfully parse the PLA file
    assert!(result.is_ok());
    if let Ok(cover) = result {
        assert_eq!(cover.num_cubes(), 2); // 2 cubes in the PLA
    }
}

#[test]
fn test_cover_builder() {
    // Create cover
    let mut cover = CoverBuilder::<2, 1>::new();
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    
    // Minimize
    cover.minimize().unwrap();
    assert!(cover.num_cubes() > 0);
}

#[test]
fn test_pla_type_values() {
    assert_eq!(PLAType::F as i32, 1);
    assert_eq!(PLAType::FD as i32, 3);
    assert_eq!(PLAType::FR as i32, 5);
    assert_eq!(PLAType::FDR as i32, 7);
}
