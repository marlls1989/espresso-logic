//! Test that Cover works with the transparent API

use espresso_logic::{Cover, CoverBuilder};

#[test]
fn test_cover_populates_cubes() {
    // Create cover and add cubes
    let mut cover = CoverBuilder::<2, 1>::new();

    // Add two cubes (XOR function)
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]); // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // 10 -> 1

    // Minimize
    cover.minimize().unwrap();

    // XOR cannot be minimized - should still have 2 cubes
    assert_eq!(cover.num_cubes(), 2, "XOR should have exactly 2 cubes after minimization");
}

#[test]
fn test_cover_many_cubes() {
    // Create cover
    let mut cover = CoverBuilder::<3, 1>::new();

    // Add 4 cubes: all have input[2]=1, so this should minimize to just --1 -> 1
    cover.add_cube(&[Some(false), Some(false), Some(true)], &[Some(true)]);  // 001 -> 1
    cover.add_cube(&[Some(false), Some(true), Some(true)], &[Some(true)]);   // 011 -> 1
    cover.add_cube(&[Some(true), Some(false), Some(true)], &[Some(true)]);   // 101 -> 1
    cover.add_cube(&[Some(true), Some(true), Some(true)], &[Some(true)]);    // 111 -> 1

    // Minimize
    cover.minimize().unwrap();

    // Should minimize to 1 cube: --1 (whenever input[2]=1, output=1)
    assert_eq!(cover.num_cubes(), 1, "Should minimize to 1 cube: --1 -> 1");
}
