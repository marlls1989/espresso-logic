//! Tests for the safe, public API with const generics

use espresso_logic::{Cover, CoverBuilder};

#[test]
fn test_multiple_instances() {
    // Multiple covers can coexist without conflicts
    let _cover1 = CoverBuilder::<2, 1>::new();
    let _cover2 = CoverBuilder::<3, 1>::new();
    let _cover3 = CoverBuilder::<4, 1>::new();

    // All can coexist safely
}

#[test]
fn test_cover_basic() {
    let mut cover = CoverBuilder::<2, 1>::new();
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

    cover.minimize().unwrap();
    assert!(cover.num_cubes() > 0);
}

#[test]
fn test_clone() {
    let cover1 = CoverBuilder::<2, 1>::new();
    let _cover2 = cover1.clone();

    // Both should exist independently
}

#[test]
fn test_debug_output() {
    let cover = CoverBuilder::<2, 1>::new();
    let debug_str = format!("{:?}", cover);

    // Should contain useful information
    assert!(debug_str.contains("Cover"));
}

#[test]
fn test_add_cube_with_dont_care() {
    let mut cover = CoverBuilder::<3, 1>::new();

    // Use don't care (None) in inputs
    cover.add_cube(&[Some(true), None, Some(false)], &[Some(true)]);
    cover.add_cube(&[None, Some(true), Some(true)], &[Some(true)]);

    cover.minimize().unwrap();
    assert!(cover.num_cubes() > 0);
}

#[test]
fn test_num_cubes_before_minimize() {
    let mut cover = CoverBuilder::<2, 1>::new();

    assert_eq!(cover.num_cubes(), 0, "Empty cover should have 0 cubes");

    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
    assert_eq!(cover.num_cubes(), 1, "Should have 1 cube before minimize");

    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    assert_eq!(cover.num_cubes(), 2, "Should have 2 cubes before minimize");
}

#[test]
fn test_num_cubes_after_minimize() {
    let mut cover = CoverBuilder::<2, 1>::new();
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

    cover.minimize().unwrap();
    let after = cover.num_cubes();

    // After minimization, num_cubes should return the result count
    assert!(after > 0, "Should have cubes after minimization");
}
