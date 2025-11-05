//! Tests for PLA API unsafe code paths

use espresso_logic::{PLAType, PLA};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_pla_from_file_basic() {
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
        .expect("Failed to write");
    temp.flush().expect("Failed to flush");

    let pla = PLA::from_file(temp.path()).expect("Failed to read PLA");

    let stats = pla.stats();
    assert_eq!(stats.num_cubes_f, 2);
}

#[test]
fn test_pla_minimize() {
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

    let mut temp = NamedTempFile::new().expect("Failed to create temp file");
    temp.write_all(pla_content.as_bytes())
        .expect("Failed to write");
    temp.flush().expect("Failed to flush");

    let pla = PLA::from_file(temp.path()).expect("Failed to read PLA");
    let before = pla.stats();

    let minimized = pla.minimize();
    let after = minimized.stats();

    // Strict expectations: 8 cubes representing d'=0 for d in [0,7]
    // Should minimize to 1 cube: d'=0 (all d bits OFF)
    assert_eq!(before.num_cubes_f, 8, "Should start with exactly 8 cubes");
    assert_eq!(after.num_cubes_f, 1, "Should minimize to exactly 1 cube");
}

#[test]
fn test_pla_to_file() {
    let pla_content = r#".i 2
.o 1
.ilb a b
.ob f
.p 2
01 1
10 1
.e
"#;

    let mut temp_in = NamedTempFile::new().expect("Failed to create temp file");
    temp_in
        .write_all(pla_content.as_bytes())
        .expect("Failed to write");
    temp_in.flush().expect("Failed to flush");

    let pla = PLA::from_file(temp_in.path()).expect("Failed to read PLA");

    let temp_out = NamedTempFile::new().expect("Failed to create output file");
    pla.to_file(temp_out.path(), PLAType::F)
        .expect("Failed to write PLA");

    // Read it back
    let pla2 = PLA::from_file(temp_out.path()).expect("Failed to read output PLA");

    let stats1 = pla.stats();
    let stats2 = pla2.stats();
    assert_eq!(stats1.num_cubes_f, stats2.num_cubes_f);
}

#[test]
fn test_pla_from_string() {
    let pla_content = r#".i 2
.o 1
.p 2
01 1
10 1
.e
"#;

    let pla = PLA::from_string(pla_content).expect("Failed to parse PLA string");
    let stats = pla.stats();
    assert_eq!(stats.num_cubes_f, 2);
}

#[test]
fn test_pla_clone_via_minimize() {
    // PLA doesn't implement Clone, but minimize creates a new PLA
    let pla_content = r#".i 2
.o 1
.p 2
01 1
10 1
.e
"#;

    let mut temp = NamedTempFile::new().expect("Failed to create temp file");
    temp.write_all(pla_content.as_bytes())
        .expect("Failed to write");
    temp.flush().expect("Failed to flush");

    let pla = PLA::from_file(temp.path()).expect("Failed to read PLA");
    let stats1 = pla.stats();

    let minimized = pla.minimize();
    let stats2 = minimized.stats();

    // Original should still be valid
    let stats1_again = pla.stats();
    assert_eq!(stats1.num_cubes_f, stats1_again.num_cubes_f);

    // Minimized should also be valid
    let stats2_again = minimized.stats();
    assert_eq!(stats2.num_cubes_f, stats2_again.num_cubes_f);
}

#[test]
fn test_pla_debug_format() {
    let pla_content = r#".i 2
.o 1
.p 2
01 1
10 1
.e
"#;

    let mut temp = NamedTempFile::new().expect("Failed to create temp file");
    temp.write_all(pla_content.as_bytes())
        .expect("Failed to write");
    temp.flush().expect("Failed to flush");

    let pla = PLA::from_file(temp.path()).expect("Failed to read PLA");

    let debug_str = format!("{:?}", pla);
    assert!(debug_str.contains("PLA"));
    assert!(debug_str.contains("cubes_f"));
}

#[test]
fn test_pla_write_to_stdout() {
    let pla_content = r#".i 2
.o 1
.p 2
01 1
10 1
.e
"#;

    let mut temp = NamedTempFile::new().expect("Failed to create temp file");
    temp.write_all(pla_content.as_bytes())
        .expect("Failed to write");
    temp.flush().expect("Failed to flush");

    let pla = PLA::from_file(temp.path()).expect("Failed to read PLA");

    // This tests the unsafe fd duplication code
    pla.write_to_stdout(PLAType::F)
        .expect("Failed to write to stdout");
}

#[test]
fn test_pla_output_formats() {
    let pla_content = r#".i 2
.o 1
.p 2
01 1
10 1
.e
"#;

    let mut temp_in = NamedTempFile::new().expect("Failed to create temp file");
    temp_in
        .write_all(pla_content.as_bytes())
        .expect("Failed to write");
    temp_in.flush().expect("Failed to flush");

    let pla = PLA::from_file(temp_in.path()).expect("Failed to read PLA");

    // Test all output formats
    let formats = vec![PLAType::F, PLAType::FD, PLAType::FR, PLAType::FDR];

    for format in formats {
        let temp_out = NamedTempFile::new().expect("Failed to create output file");
        pla.to_file(temp_out.path(), format)
            .expect("Failed to write PLA");
    }
}

#[test]
fn test_pla_empty() {
    // Test that empty PLA (0 products) is rejected by C library
    let pla_content = r#".i 2
.o 1
.p 0
.e
"#;

    let mut temp = NamedTempFile::new().expect("Failed to create temp file");
    temp.write_all(pla_content.as_bytes())
        .expect("Failed to write");
    temp.flush().expect("Failed to flush");

    // The C library returns EOF for empty PLA
    let result = PLA::from_file(temp.path());
    assert!(result.is_err(), "Empty PLA should fail to parse");
}

#[test]
fn test_pla_large() {
    // Test with a larger PLA
    let mut pla_content = String::from(".i 8\n.o 1\n.p 256\n");

    // Add all possible 8-bit patterns
    for i in 0..256 {
        let binary = format!("{:08b}", i);
        pla_content.push_str(&format!("{} 1\n", binary));
    }
    pla_content.push_str(".e\n");

    let mut temp = NamedTempFile::new().expect("Failed to create temp file");
    temp.write_all(pla_content.as_bytes())
        .expect("Failed to write");
    temp.flush().expect("Failed to flush");

    let pla = PLA::from_file(temp.path()).expect("Failed to read PLA");
    let before = pla.stats();
    assert_eq!(before.num_cubes_f, 256, "Should have exactly 256 cubes");

    // All 256 patterns map to 1, should minimize to 1 cube (always 1)
    let minimized = pla.minimize();
    let after = minimized.stats();
    assert_eq!(after.num_cubes_f, 1, "Should minimize to exactly 1 cube");
}

#[test]
fn test_pla_sequential_operations() {
    // Test that we can perform multiple operations sequentially
    let pla_content = r#".i 2
.o 1
.p 4
00 1
01 1
10 1
11 1
.e
"#;

    let mut temp = NamedTempFile::new().expect("Failed to create temp file");
    temp.write_all(pla_content.as_bytes())
        .expect("Failed to write");
    temp.flush().expect("Failed to flush");

    // Read and minimize multiple times
    for _ in 0..3 {
        let pla = PLA::from_file(temp.path()).expect("Failed to read PLA");
        let minimized = pla.minimize();
        let stats = minimized.stats();
        assert_eq!(stats.num_cubes_f, 1);
    }
}
