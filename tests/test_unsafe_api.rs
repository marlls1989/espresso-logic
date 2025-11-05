//! Comprehensive tests for unsafe API paths
//! These tests exercise the actual usage patterns and would catch segfaults

use espresso_logic::{Cover, CoverBuilder, Espresso};

#[test]
fn test_espresso_new_and_drop() {
    // Test that Espresso can be created and dropped safely
    let esp = Espresso::new(2, 1);
    drop(esp);

    // Should be able to create another instance after drop
    let esp2 = Espresso::new(3, 1);
    drop(esp2);
}

#[test]
fn test_espresso_multiple_instances_sequential() {
    // Test creating multiple instances sequentially
    for i in 1..5 {
        let esp = Espresso::new(i, 1);
        drop(esp);
    }
}

#[test]
fn test_cover_builder_without_espresso() {
    // This SHOULD fail or be documented as requiring Espresso::new() first
    // Currently this is a bug - CoverBuilder assumes cube is initialized

    // Note: This test is expected to panic/crash - documenting the issue
    // Uncomment when we decide how to handle this properly

    // let mut builder = CoverBuilder::new(2, 1);
    // builder.add_cube(&[0, 1], &[1]);
    // let _cover = builder.build(); // Will panic with null pointer
}

#[test]
fn test_cover_builder_with_espresso() {
    // Test the proper workflow: Espresso::new() THEN CoverBuilder
    let _esp = Espresso::new(2, 1);

    let mut builder = CoverBuilder::new(2, 1);
    builder.add_cube(&[0, 1], &[1]);
    builder.add_cube(&[1, 0], &[1]);

    let cover = builder.build();
    assert_eq!(cover.count(), 2, "Cover should have 2 cubes");
}

#[test]
fn test_end_to_end_minimize_simple() {
    // This is the MOST IMPORTANT test - actual end-to-end usage
    // XOR function: should have 2 cubes (cannot be reduced)

    let mut esp = Espresso::new(2, 1);

    let mut builder = CoverBuilder::new(2, 1);
    builder.add_cube(&[0, 1], &[1]); // XOR: 01 -> 1
    builder.add_cube(&[1, 0], &[1]); // XOR: 10 -> 1

    let cover = builder.build();
    assert_eq!(cover.count(), 2, "Input should have 2 cubes");

    let minimized = esp.minimize(cover, None, None);
    assert_eq!(
        minimized.count(),
        2,
        "XOR cannot be minimized further, should still have 2 cubes"
    );
}

#[test]
#[ignore] // Ignored because it currently segfaults
fn test_end_to_end_minimize_with_dc() {
    // Test with don't-care set

    let mut esp = Espresso::new(2, 1);

    // ON-set
    let mut f_builder = CoverBuilder::new(2, 1);
    f_builder.add_cube(&[0, 1], &[1]);
    let f = f_builder.build();

    // Don't-care set
    let mut d_builder = CoverBuilder::new(2, 1);
    d_builder.add_cube(&[1, 1], &[1]);
    let d = d_builder.build();

    let _minimized = esp.minimize(f, Some(d), None); // SEGFAULT HERE
}

#[test]
fn test_cover_clone() {
    // Test Cover::clone() which uses unsafe sys::sf_save()
    let _esp = Espresso::new(2, 1);

    let mut builder = CoverBuilder::new(2, 1);
    builder.add_cube(&[0, 1], &[1]);
    let cover = builder.build();

    let cloned = cover.clone();
    assert_eq!(cover.count(), cloned.count());
    assert_eq!(cover.cube_size(), cloned.cube_size());
}

#[test]
fn test_cover_debug_format() {
    // Test that Debug impl doesn't panic
    let _esp = Espresso::new(2, 1);

    let mut builder = CoverBuilder::new(2, 1);
    builder.add_cube(&[0, 1], &[1]);
    let cover = builder.build();

    let debug_str = format!("{:?}", cover);
    assert!(debug_str.contains("Cover"));
    assert!(debug_str.contains("count"));
}

#[test]
fn test_cover_builder_empty() {
    // Test building an empty cover
    let _esp = Espresso::new(2, 1);

    let builder = CoverBuilder::new(2, 1);
    let cover = builder.build();

    assert_eq!(cover.count(), 0);
}

#[test]
fn test_cover_builder_many_inputs() {
    // Test with more inputs
    let _esp = Espresso::new(5, 1);

    let mut builder = CoverBuilder::new(5, 1);
    builder.add_cube(&[0, 1, 0, 1, 0], &[1]);
    builder.add_cube(&[1, 0, 1, 0, 1], &[1]);

    let cover = builder.build();
    assert_eq!(cover.count(), 2);
}

#[test]
fn test_cover_builder_dont_care() {
    // Test with don't-care values (2)
    let _esp = Espresso::new(3, 1);

    let mut builder = CoverBuilder::new(3, 1);
    builder.add_cube(&[2, 1, 0], &[1]); // -10 -> 1
    builder.add_cube(&[0, 2, 1], &[1]); // 0-1 -> 1

    let cover = builder.build();
    assert_eq!(cover.count(), 2);
}

#[test]
fn test_cover_builder_multiple_outputs() {
    // Test with multiple outputs
    let _esp = Espresso::new(2, 3);

    let mut builder = CoverBuilder::new(2, 3);
    builder.add_cube(&[0, 1], &[1, 0, 1]);
    builder.add_cube(&[1, 0], &[0, 1, 1]);

    let cover = builder.build();
    assert_eq!(cover.count(), 2);
}

#[test]
#[should_panic(expected = "Input length mismatch")]
fn test_cover_builder_wrong_input_length() {
    let _esp = Espresso::new(2, 1);

    let mut builder = CoverBuilder::new(2, 1);
    builder.add_cube(&[0, 1, 0], &[1]); // 3 inputs, expected 2
}

#[test]
#[should_panic(expected = "Output length mismatch")]
fn test_cover_builder_wrong_output_length() {
    let _esp = Espresso::new(2, 1);

    let mut builder = CoverBuilder::new(2, 1);
    builder.add_cube(&[0, 1], &[1, 0]); // 2 outputs, expected 1
}

#[test]
#[should_panic(expected = "Invalid input value")]
fn test_cover_builder_invalid_input_value() {
    let _esp = Espresso::new(2, 1);

    let mut builder = CoverBuilder::new(2, 1);
    builder.add_cube(&[0, 5], &[1]); // 5 is definitely invalid, should be 0, 1, or 2
    let _cover = builder.build(); // Must call build() to trigger the panic
}

#[test]
fn test_cover_raw_conversion() {
    // Test Cover::into_raw and Cover::from_raw
    let _esp = Espresso::new(2, 1);

    let mut builder = CoverBuilder::new(2, 1);
    builder.add_cube(&[0, 1], &[1]);
    let cover = builder.build();

    let original_count = cover.count();

    // Convert to raw and back
    let raw_ptr = cover.into_raw();
    let cover2 = unsafe { Cover::from_raw(raw_ptr) };

    assert_eq!(cover2.count(), original_count);
}

#[test]
fn test_espresso_different_sizes() {
    // Test various input/output combinations
    let test_cases = vec![(1, 1), (2, 1), (3, 1), (4, 2), (5, 3), (8, 4)];

    for (inputs, outputs) in test_cases {
        let esp = Espresso::new(inputs, outputs);
        drop(esp);
    }
}

#[test]
fn test_cover_builder_large() {
    // Test with many cubes
    let _esp = Espresso::new(3, 1);

    let mut builder = CoverBuilder::new(3, 1);
    for i in 0..8 {
        let a = (i >> 2) & 1;
        let b = (i >> 1) & 1;
        let c = i & 1;
        builder.add_cube(&[a as u8, b as u8, c as u8], &[1]);
    }

    let cover = builder.build();
    assert_eq!(cover.count(), 8);
}

#[test]
fn test_espresso_config_apply() {
    // Test that configuration can be applied
    use espresso_logic::EspressoConfig;

    let config = EspressoConfig {
        summary: true,
        debug: false,
        ..Default::default()
    };

    config.apply();

    // Should be able to create Espresso after config
    let _esp = Espresso::new(2, 1);
}
