//! Test Cover API with PLA format support

use espresso_logic::{Cover, CoverBuilder, PLACover, PLAType};

#[test]
fn test_create_cover_from_pla() {
    // Create PLA content programmatically for XOR function
    let pla_str = ".i 2\n.o 1\n.p 2\n01 1\n10 1\n.e\n";

    let mut cover = PLACover::from_pla_content(pla_str).expect("Failed to parse PLA");
    assert_eq!(cover.num_cubes(), 2);

    cover.minimize().unwrap();

    // XOR cannot be minimized
    assert_eq!(cover.num_cubes(), 2);
}

#[test]
fn test_pla_roundtrip() {
    // Create a cover programmatically
    let mut cover = CoverBuilder::<2, 1>::new();
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]); // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // 10 -> 1

    // Convert to PLA format using the trait
    let pla_str = <CoverBuilder<2, 1> as Cover>::to_pla_string(&cover, PLAType::F)
        .expect("Failed to serialize");

    println!("Generated PLA:\n{}", pla_str);

    // Parse it back using PLACover
    let mut parsed_cover = PLACover::from_pla_content(&pla_str).expect("Failed to parse");
    assert_eq!(parsed_cover.num_cubes(), 2);

    // Minimize and verify XOR cannot be reduced
    parsed_cover.minimize().unwrap();
    assert_eq!(parsed_cover.num_cubes(), 2);
}
