//! Test that CoverBuilder actually works

use espresso_logic::CoverBuilder;

#[test]
fn test_cover_builder_populates_cubes() {
    let mut builder = CoverBuilder::new(2, 1);

    // Add two cubes (XOR function)
    builder.add_cube(&[0, 1], &[1]); // 01 -> 1
    builder.add_cube(&[1, 0], &[1]); // 10 -> 1

    let cover = builder.build();

    // Verify the cover actually contains the cubes we added
    assert_eq!(
        cover.count(),
        2,
        "Cover should contain 2 cubes, but has {}",
        cover.count()
    );
}

#[test]
fn test_cover_builder_empty() {
    let builder = CoverBuilder::new(2, 1);
    let cover = builder.build();

    // Empty builder should produce empty cover
    assert_eq!(cover.count(), 0);
}

#[test]
fn test_cover_builder_many_cubes() {
    let mut builder = CoverBuilder::new(3, 1);

    // Add 4 cubes
    builder.add_cube(&[0, 0, 1], &[1]);
    builder.add_cube(&[0, 1, 1], &[1]);
    builder.add_cube(&[1, 0, 1], &[1]);
    builder.add_cube(&[1, 1, 1], &[1]);

    let cover = builder.build();

    assert_eq!(cover.count(), 4, "Cover should contain 4 cubes");
}
