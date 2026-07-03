//! Integration tests for the Espresso Rust wrapper
//!
//! These tests verify end-to-end functionality including file I/O,
//! PLA format handling, and complete minimization workflows.

use espresso_logic::Anonymous;
use espresso_logic::{Minimizable, *};
use std::io::Write;
use tempfile::NamedTempFile;

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

    // Test with new Cover API — unconditionally assert the cube count so a parse failure can't
    // slip past as a vacuously-true conditional.
    let cover = PlaCover::<Symbol>::from_pla_file(temp.path()).expect("Failed to parse PLA file");
    assert_eq!(cover.num_cubes(), 2); // 2 cubes in the PLA
}

#[test]
fn test_create_cover_from_pla() {
    // Create PLA content programmatically for XOR function
    let pla_str = ".i 2\n.o 1\n.p 2\n01 1\n10 1\n.e\n";

    let cover = PlaCover::<Symbol>::from_pla_string(pla_str).expect("Failed to parse PLA");
    assert_eq!(cover.num_cubes(), 2);

    let cover = cover.minimize().unwrap();

    // XOR cannot be minimized
    assert_eq!(cover.num_cubes(), 2);
}

#[test]
fn test_pla_roundtrip() {
    // Create a cover programmatically
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    )); // 01 -> 1
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    )); // 10 -> 1

    // PLA serialisation is string-labelled; give the anonymous cover real input/output names.
    let cover: Cover<Symbol, Symbol> = cover
        .rename(["x0", "x1"], ["y0"])
        .expect("relabel arity matches");

    // Convert to PLA format using the trait
    let pla_str = cover
        .to_pla_string(CoverType::F)
        .expect("Failed to serialize");

    // Parse it back using Cover
    let parsed_cover = PlaCover::<Symbol>::from_pla_string(&pla_str).expect("Failed to parse");
    assert_eq!(parsed_cover.num_cubes(), 2);

    // Minimize and verify XOR cannot be reduced
    let parsed_cover = parsed_cover.minimize().unwrap();
    assert_eq!(parsed_cover.num_cubes(), 2);
}

#[test]
fn test_pla_roundtrip_empty_literal() {
    // A `?` input field is Espresso's empty literal: the cube denotes the empty set. Confirm it
    // survives the PLA round-trip byte-for-byte, parses to `InputField::Empty`/a vacuous minterm,
    // and is dropped by minimisation.
    let pla_str = ".i 3\n.o 1\n.p 1\n1?0 1\n.e\n";

    let parsed_cover = PlaCover::<Symbol>::from_pla_string(pla_str).expect("Failed to parse PLA");
    assert_eq!(parsed_cover.num_cubes(), 1);

    // Writing the parsed cover back out at the same cover type reproduces the `?` verbatim.
    let written = parsed_cover
        .to_pla_string(CoverType::F)
        .expect("Failed to serialize");
    assert_eq!(written, pla_str);

    // The `?` column is `InputField::Empty`, and the whole input minterm is vacuous as a result.
    let cover = match &parsed_cover {
        PlaCover::Positional(cover) => cover,
        other => panic!("a PLA with no .ilb/.ob sections must parse to Positional, got {other:?}"),
    };
    let cube = cover.cubes().next().expect("one cube");
    assert_eq!(cube.inputs().field_at(1), InputField::Empty);
    assert!(cube.inputs().is_vacuous());

    // Minimisation's pre-pass drops vacuous cubes outright, so the vacuous-only cover minimises to
    // nothing.
    let minimized_cover = parsed_cover.minimize().unwrap();
    assert_eq!(minimized_cover.num_cubes(), 0);
}
