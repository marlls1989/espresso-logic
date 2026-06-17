//! Tests for the cover module

use super::pla::{PLAReader, PLAWriter};
use super::*;

#[test]
fn test_cover_creation() {
    let cover = Cover::new(CoverType::F);
    assert_eq!(cover.num_inputs(), 0);
    assert_eq!(cover.num_outputs(), 0);
    assert_eq!(cover.num_cubes(), 0);
}

#[test]
fn test_cover_with_labels() {
    let cover = Cover::with_labels(CoverType::F, &["a", "b", "c"], &["out"]);
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");
    assert_eq!(cover.input_labels()[2].as_ref(), "c");
    assert_eq!(cover.output_labels()[0].as_ref(), "out");
}

#[test]
fn test_add_cube() {
    let mut cover = Cover::<(), ()>::anonymous(CoverType::F);
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.num_cubes(), 1);
}

#[test]
fn test_minimize() {
    let mut cover = Cover::<(), ()>::anonymous(CoverType::F);
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    cover = cover.minimize().unwrap();
    // XOR cannot be minimized
    assert_eq!(cover.num_cubes(), 2);
}

// ===== Dynamic Growth Tests =====

#[test]
fn test_dynamic_growth_inputs_only() {
    let mut cover = Cover::<(), ()>::anonymous(CoverType::F);

    // Start with 2 inputs
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);

    // Grow to 5 inputs
    cover.add_cube(
        &[Some(true), None, Some(false), None, Some(true)],
        &[Some(true)],
    );
    assert_eq!(cover.num_inputs(), 5);
    assert_eq!(cover.num_outputs(), 1);

    // Verify all cubes have consistent dimensions
    for cube in cover.cubes() {
        assert_eq!(cube.inputs().num_vars(), 5);
        assert_eq!(cube.outputs().num_vars(), 1);
    }
}

#[test]
fn test_dynamic_growth_outputs_only() {
    let mut cover = Cover::<(), ()>::anonymous(CoverType::F);

    // Start with 1 output
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);

    // Grow to 3 outputs
    cover.add_cube(&[Some(true), None], &[Some(true), Some(false), Some(true)]);
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 3);

    // Verify all cubes have consistent dimensions
    for cube in cover.cubes() {
        assert_eq!(cube.inputs().num_vars(), 2);
        assert_eq!(cube.outputs().num_vars(), 3);
    }
}

#[test]
fn test_dynamic_growth_both_dimensions() {
    let mut cover = Cover::<(), ()>::anonymous(CoverType::F);

    // Start small
    cover.add_cube(&[Some(true)], &[Some(true)]);
    assert_eq!(cover.num_inputs(), 1);
    assert_eq!(cover.num_outputs(), 1);

    // Grow both dimensions
    cover.add_cube(&[Some(true), Some(false), None], &[Some(true), Some(false)]);
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.num_outputs(), 2);

    // Add another with even more dimensions
    cover.add_cube(
        &[Some(true), Some(false), None, Some(true)],
        &[Some(true), Some(false), Some(true)],
    );
    assert_eq!(cover.num_inputs(), 4);
    assert_eq!(cover.num_outputs(), 3);

    // All cubes should have been padded
    assert_eq!(cover.num_cubes(), 3);
    for cube in cover.cubes() {
        assert_eq!(cube.inputs().num_vars(), 4);
        assert_eq!(cube.outputs().num_vars(), 3);
    }
}

#[test]
fn test_dynamic_growth_preserves_existing_cubes() {
    let mut cover = Cover::<(), ()>::anonymous(CoverType::F);

    // Add first cube
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

    // Get the first cube's data before growth
    let first_cube_inputs: Vec<_> = cover
        .cubes()
        .next()
        .unwrap()
        .inputs()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(first_cube_inputs, vec![Some(true), Some(false)]);

    // Grow dimensions
    cover.add_cube(&[Some(true), Some(false), Some(true)], &[Some(true)]);

    // First cube should be padded with None
    let first_cube_after: Vec<_> = cover
        .cubes()
        .next()
        .unwrap()
        .inputs()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(first_cube_after, vec![Some(true), Some(false), None]);
}

// ===== Auto-Generated Label Tests =====

// ===== Expression Addition Tests =====

#[test]
fn test_add_expr_basic() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let expr = a.and(&b);

    cover.add_expr(&expr, "output").unwrap();

    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");
    assert_eq!(cover.output_labels()[0].as_ref(), "output");
    assert!(cover.num_cubes() > 0);
}

#[test]
fn test_add_expr_variable_matching() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let c = crate::BoolExpr::variable("c");

    // Add first expression with variables a and b
    cover.add_expr(&a.and(&b), "out1").unwrap();
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");

    // Add second expression with variables b and c (b should match, c appended)
    cover.add_expr(&b.and(&c), "out2").unwrap();
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");
    assert_eq!(cover.input_labels()[2].as_ref(), "c");

    assert_eq!(cover.num_outputs(), 2);
}

#[test]
fn test_add_expr_duplicate_output_error() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    // Add first expression
    cover.add_expr(&a, "result").unwrap();

    // Try to add another expression with same output name - should fail
    let result = cover.add_expr(&b, "result");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn test_add_expr_to_different_cover_types() {
    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    // F type
    let mut f_cover = Cover::new(CoverType::F);
    f_cover.add_expr(&a.and(&b), "out").unwrap();
    assert_eq!(f_cover.cover_type(), CoverType::F);

    // FD type
    let mut fd_cover = Cover::new(CoverType::FD);
    fd_cover.add_expr(&a.or(&b), "out").unwrap();
    assert_eq!(fd_cover.cover_type(), CoverType::FD);

    // FR type
    let mut fr_cover = Cover::new(CoverType::FR);
    fr_cover.add_expr(&a, "out").unwrap();
    assert_eq!(fr_cover.cover_type(), CoverType::FR);

    // FDR type
    let mut fdr_cover = Cover::new(CoverType::FDR);
    fdr_cover.add_expr(&a.not(), "out").unwrap();
    assert_eq!(fdr_cover.cover_type(), CoverType::FDR);
}

#[test]
fn test_add_expr_multiple_outputs() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let c = crate::BoolExpr::variable("c");

    // Add three different expressions
    cover.add_expr(&a.and(&b), "and_result").unwrap();
    cover.add_expr(&a.or(&c), "or_result").unwrap();
    cover.add_expr(&b.not(), "not_result").unwrap();

    assert_eq!(cover.num_outputs(), 3);
    assert_eq!(cover.output_labels()[0].as_ref(), "and_result");
    assert_eq!(cover.output_labels()[1].as_ref(), "or_result");
    assert_eq!(cover.output_labels()[2].as_ref(), "not_result");

    // All three variables should be present
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");
    assert_eq!(cover.input_labels()[2].as_ref(), "c");
}

#[test]
fn test_add_expr_variable_ordering_preserved() {
    let mut cover = Cover::new(CoverType::F);

    let z = crate::BoolExpr::variable("z");
    let a = crate::BoolExpr::variable("a");
    let m = crate::BoolExpr::variable("m");

    // Add expression with variables in non-alphabetical order
    // Variables in BoolExpr are sorted alphabetically internally
    cover.add_expr(&z.and(&a).and(&m), "out").unwrap();

    // Variables should be in alphabetical order (a, m, z)
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "m");
    assert_eq!(cover.input_labels()[2].as_ref(), "z");
}

// ===== Expression Conversion Tests =====

#[test]
fn test_to_expr_basic() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    cover.add_expr(&a.and(&b), "result").unwrap();

    let retrieved = cover.to_expr("result").unwrap();

    // Should be able to collect variables
    let vars = retrieved.collect_variables();
    assert_eq!(vars.len(), 2);
    assert!(vars.contains(&std::sync::Arc::from("a")));
    assert!(vars.contains(&std::sync::Arc::from("b")));
}

#[test]
fn test_to_expr_by_index() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");

    cover.add_expr(&a, "out0").unwrap();
    cover.add_expr(&a.not(), "out1").unwrap();

    let expr0 = cover.to_expr_by_index(0).unwrap();
    let expr1 = cover.to_expr_by_index(1).unwrap();

    assert_eq!(expr0.collect_variables().len(), 1);
    assert_eq!(expr1.collect_variables().len(), 1);
}

#[test]
fn test_to_expr_nonexistent() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    cover.add_expr(&a, "exists").unwrap();

    // Try to get non-existent output
    let result = cover.to_expr("doesnt_exist");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_to_expr_index_out_of_bounds() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    cover.add_expr(&a, "out").unwrap();

    // Try to get out of bounds index
    let result = cover.to_expr_by_index(1);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("out of bounds"));
}

#[test]
fn test_to_exprs_iterator() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let c = crate::BoolExpr::variable("c");

    cover.add_expr(&a, "out1").unwrap();
    cover.add_expr(&b, "out2").unwrap();
    cover.add_expr(&c, "out3").unwrap();

    let exprs: Vec<_> = cover.to_exprs().collect();
    assert_eq!(exprs.len(), 3);

    assert_eq!(exprs[0].0.as_ref(), "out1");
    assert_eq!(exprs[1].0.as_ref(), "out2");
    assert_eq!(exprs[2].0.as_ref(), "out3");

    // Each expression should have one variable
    assert_eq!(exprs[0].1.collect_variables().len(), 1);
    assert_eq!(exprs[1].1.collect_variables().len(), 1);
    assert_eq!(exprs[2].1.collect_variables().len(), 1);
}

#[test]
fn test_to_exprs_after_minimization() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let c = crate::BoolExpr::variable("c");

    // Add redundant expression: a*b + a*b*c
    let redundant = a.and(&b).or(&a.and(&b).and(&c));
    cover.add_expr(&redundant, "out").unwrap();

    let cubes_before = cover.num_cubes();
    cover = cover.minimize().unwrap();
    let cubes_after = cover.num_cubes();

    // Should minimize
    assert!(cubes_after <= cubes_before);

    // Should still be able to convert to expression
    let minimized = cover.to_expr("out").unwrap();
    let vars = minimized.collect_variables();
    assert!(vars.len() >= 2); // At least a and b
}

// ===== Cover Type Tests =====

#[test]
fn test_f_type_cover() {
    let mut cover = Cover::<(), ()>::anonymous(CoverType::F);

    // F type only accepts Some(true) for outputs
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    assert_eq!(cover.num_cubes(), 1);

    // Some(false) and None are ignored for F type
    cover.add_cube(&[Some(true), Some(true)], &[Some(false)]);
    cover.add_cube(&[Some(false), Some(false)], &[None]);

    // Should still have only 1 cube (F type)
    assert_eq!(cover.num_cubes(), 1);
}

#[test]
fn test_fd_type_cover() {
    let mut cover = Cover::<(), ()>::anonymous(CoverType::FD);

    // FD type accepts Some(true) and None
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // F cube
    cover.add_cube(&[Some(false), Some(true)], &[None]); // D cube

    // For FD type, num_cubes() only counts F cubes
    assert_eq!(cover.num_cubes(), 1);

    // But internal cubes should have both
    assert_eq!(cover.cubes.len(), 2);
}

#[test]
fn test_fr_type_cover() {
    let mut cover = Cover::<(), ()>::anonymous(CoverType::FR);

    // FR type accepts Some(true) and Some(false)
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // F cube
    cover.add_cube(&[Some(false), Some(true)], &[Some(false)]); // R cube

    // For FR type, num_cubes() counts all cubes
    assert_eq!(cover.num_cubes(), 2);
}

#[test]
fn test_fdr_type_cover() {
    let mut cover = Cover::<(), ()>::anonymous(CoverType::FDR);

    // FDR type accepts all: Some(true), Some(false), None
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // F cube
    cover.add_cube(&[Some(false), Some(true)], &[Some(false)]); // R cube
    cover.add_cube(&[Some(true), Some(true)], &[None]); // D cube

    // For FDR type, num_cubes() counts all cubes
    assert_eq!(cover.num_cubes(), 3);
}

// ===== Mixed Operations Tests =====

#[test]
fn test_complex_expression_with_minimization() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let c = crate::BoolExpr::variable("c");

    // Consensus theorem: a*b + ~a*c + b*c (b*c is redundant)
    let expr = a.and(&b).or(&a.not().and(&c)).or(&b.and(&c));
    cover.add_expr(&expr, "consensus").unwrap();

    // BDD automatically optimizes during conversion, so we get 2 cubes directly
    // (b*c is recognized as redundant by the canonical BDD representation)
    assert_eq!(cover.num_cubes(), 2);

    cover = cover.minimize().unwrap();

    // Should still have 2 cubes after minimization
    assert_eq!(cover.num_cubes(), 2);

    // Should still be able to convert back
    let minimized = cover.to_expr("consensus").unwrap();
    assert_eq!(minimized.collect_variables().len(), 3);
}

#[test]
fn test_empty_cover_to_expr() {
    let cover = Cover::new(CoverType::F);

    // Try to get expression from empty cover - should fail
    let result = cover.to_expr_by_index(0);
    assert!(result.is_err());
}

#[test]
fn test_expression_with_constants() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let t = crate::BoolExpr::constant(true);

    // Expression with constant: a * true = a
    let expr = a.and(&t);
    cover.add_expr(&expr, "out").unwrap();

    // Should have one variable
    assert_eq!(cover.num_inputs(), 1);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
}

#[test]
fn test_pla_roundtrip_with_expressions() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    cover.add_expr(&a.and(&b), "output").unwrap();

    // Convert to PLA string
    let pla_string = cover.to_pla_string(CoverType::F).unwrap();

    // Parse it back
    let cover2 = Cover::from_pla_string(&pla_string).unwrap();

    // Should have same dimensions
    assert_eq!(cover2.num_inputs(), cover.num_inputs());
    assert_eq!(cover2.num_outputs(), cover.num_outputs());
    assert_eq!(cover2.num_cubes(), cover.num_cubes());

    // Labels should be preserved
    assert_eq!(cover2.input_labels()[0].as_ref(), "a");
    assert_eq!(cover2.input_labels()[1].as_ref(), "b");
    assert_eq!(cover2.output_labels()[0].as_ref(), "output");
}

#[test]
fn test_minimize_preserves_structure() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    cover.add_expr(&a.and(&b), "out1").unwrap();
    cover.add_expr(&a.or(&b), "out2").unwrap();

    let inputs_before = cover.num_inputs();
    let outputs_before = cover.num_outputs();

    cover = cover.minimize().unwrap();

    // Dimensions should be preserved
    assert_eq!(cover.num_inputs(), inputs_before);
    assert_eq!(cover.num_outputs(), outputs_before);

    // Should still be able to extract both expressions
    let expr1 = cover.to_expr("out1").unwrap();
    let expr2 = cover.to_expr("out2").unwrap();

    assert!(expr1.collect_variables().len() <= 2);
    assert!(expr2.collect_variables().len() <= 2);
}

// ===== Generic label type / anonymous covers (M3) =====

#[test]
fn anonymous_cover_minimizes() {
    // Pure positional cover, no labels (L = ()).
    let mut cover: Cover<(), ()> = Cover::anonymous(CoverType::F);
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]); // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // 10 -> 1
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_cubes(), 2);
    let min = cover.minimize().unwrap();
    assert_eq!(min.num_inputs(), 2);
    assert!(min.num_cubes() >= 1);
}

#[test]
fn custom_u32_labels_via_relabel() {
    let mut cover: Cover<(), ()> = Cover::anonymous(CoverType::F);
    cover.add_cube(&[Some(true), None, Some(false)], &[Some(true)]);
    // Explicitly relabel to a u32-labelled cover, position-for-position.
    let labeled: Cover<u32, u32> = cover.relabel(
        Symbols::new(vec![10u32, 20, 30].into()),
        Symbols::new(vec![1u32].into()),
    );
    assert_eq!(labeled.num_inputs(), 3);
    let first = labeled.cubes().next().unwrap();
    assert_eq!(first.inputs().value_of(&10u32), Some(true));
    assert_eq!(first.inputs().value_of(&20u32), None);
    assert_eq!(first.inputs().value_of(&30u32), Some(false));
    assert_eq!(first.inputs().value_of(&99u32), None); // absent variable
    let _ = labeled.minimize().unwrap();
}

#[test]
fn anonymize_drops_labels_preserving_values() {
    use std::sync::Arc;
    // Build positionally, label explicitly, then anonymise back — values preserved throughout.
    let mut anon = Cover::<(), ()>::anonymous(CoverType::F);
    anon.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    let labeled = anon.relabel(
        Symbols::new(vec![Arc::<str>::from("a"), Arc::from("b")].into()),
        Symbols::new(vec![Arc::<str>::from("out")].into()),
    );
    assert_eq!(labeled.num_inputs(), 2);

    let back: Cover<(), ()> = labeled.anonymize();
    assert_eq!(back.num_cubes(), 1);
    let cube = back.cubes().next().unwrap();
    assert_eq!(cube.inputs().value_at(0), Some(true));
    assert_eq!(cube.inputs().value_at(1), Some(false));
}

#[test]
fn cover_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Cover<(), ()>>();
    assert_send_sync::<Cover<u32, u32>>();
    assert_send_sync::<Cover<std::sync::Arc<str>>>();
}
