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
    let mut cover = Cover::new(CoverType::F);
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.num_cubes(), 1);
}

#[test]
fn test_dynamic_growth() {
    let mut cover = Cover::new(CoverType::F);
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);

    // Add larger cube
    cover.add_cube(
        &[Some(true), Some(false), Some(true)],
        &[Some(true), Some(false)],
    );
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.num_outputs(), 2);

    // Labels should NOT be auto-generated
    assert_eq!(cover.input_labels().len(), 0);
    assert_eq!(cover.output_labels().len(), 0);
}

#[test]
fn test_minimize() {
    let mut cover = Cover::new(CoverType::F);
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    cover = cover.minimize().unwrap();
    // XOR cannot be minimized
    assert_eq!(cover.num_cubes(), 2);
}

// ===== Dynamic Growth Tests =====

#[test]
fn test_dynamic_growth_inputs_only() {
    let mut cover = Cover::new(CoverType::F);

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
        assert_eq!(cube.inputs().len(), 5);
        assert_eq!(cube.outputs().len(), 1);
    }
}

#[test]
fn test_dynamic_growth_outputs_only() {
    let mut cover = Cover::new(CoverType::F);

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
        assert_eq!(cube.inputs().len(), 2);
        assert_eq!(cube.outputs().len(), 3);
    }
}

#[test]
fn test_dynamic_growth_both_dimensions() {
    let mut cover = Cover::new(CoverType::F);

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
        assert_eq!(cube.inputs().len(), 4);
        assert_eq!(cube.outputs().len(), 3);
    }
}

#[test]
fn test_dynamic_growth_preserves_existing_cubes() {
    let mut cover = Cover::new(CoverType::F);

    // Add first cube
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

    // Get the first cube's data before growth
    let first_cube_inputs: Vec<_> = cover.cubes().next().unwrap().inputs().to_vec();
    assert_eq!(first_cube_inputs, vec![Some(true), Some(false)]);

    // Grow dimensions
    cover.add_cube(&[Some(true), Some(false), Some(true)], &[Some(true)]);

    // First cube should be padded with None
    let first_cube_after: Vec<_> = cover.cubes().next().unwrap().inputs().to_vec();
    assert_eq!(first_cube_after, vec![Some(true), Some(false), None]);
}

// ===== Auto-Generated Label Tests =====

#[test]
fn test_auto_generated_input_labels() {
    let mut cover = Cover::new(CoverType::F);

    // Add cube with 5 inputs
    cover.add_cube(
        &[Some(true), Some(false), None, Some(true), Some(false)],
        &[Some(true)],
    );

    // Labels should NOT be auto-generated when adding cubes
    assert_eq!(cover.input_labels().len(), 0);

    // But when converting to expressions, default labels should be used
    let expr = cover.to_expr_by_index(0).unwrap();
    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 4); // 4 non-don't-care inputs

    // Variable names should be x0, x1, x3, x4 (x2 is don't care so not in expr)
    let var_names: Vec<&str> = vars.iter().map(|v| v.as_ref()).collect();
    assert!(var_names.contains(&"x0"));
    assert!(var_names.contains(&"x1"));
    assert!(var_names.contains(&"x3"));
    assert!(var_names.contains(&"x4"));
}

#[test]
fn test_auto_generated_output_labels() {
    let mut cover = Cover::new(CoverType::F);

    // Add cube with 4 outputs
    cover.add_cube(
        &[Some(true), Some(false)],
        &[Some(true), Some(false), Some(true), Some(false)],
    );

    // Labels should NOT be auto-generated when adding cubes
    assert_eq!(cover.output_labels().len(), 0);

    // But when using to_exprs iterator, default output names should be generated
    let exprs: Vec<_> = cover.to_exprs().collect();
    assert_eq!(exprs.len(), 4);
    assert_eq!(exprs[0].0.as_ref(), "y0");
    assert_eq!(exprs[1].0.as_ref(), "y1");
    assert_eq!(exprs[2].0.as_ref(), "y2");
    assert_eq!(exprs[3].0.as_ref(), "y3");
}

#[test]
fn test_label_uniqueness_on_growth() {
    let mut cover = Cover::new(CoverType::F);

    // Add cubes causing growth
    cover.add_cube(&[Some(true)], &[Some(true)]);
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    cover.add_cube(&[Some(true), Some(false), None], &[Some(true)]);

    // Labels should NOT be auto-generated
    assert_eq!(cover.input_labels().len(), 0);

    // When converting to expression, default labels should be used
    let expr = cover.to_expr_by_index(0).unwrap();
    let vars = expr.collect_variables();
    // BDD canonicalises: x0=1, x0=1∧x1=0, x0=1∧x1=0∧x2=- → x0 (absorption law)
    assert_eq!(vars.len(), 1); // Only x0 after BDD optimisation
}

#[test]
fn test_mixed_labels_and_growth() {
    // Start with labeled cover
    let mut cover = Cover::with_labels(CoverType::F, &["a", "b"], &["out1"]);
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);

    // Grow inputs - labels SHOULD be auto-added since cover is already labeled
    cover.add_cube(&[Some(true), Some(false), None, Some(true)], &[Some(true)]);
    assert_eq!(cover.num_inputs(), 4);
    // All 4 input labels should exist: a, b, x2, x3
    assert_eq!(cover.input_labels().len(), 4);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");
    assert_eq!(cover.input_labels()[2].as_ref(), "x2"); // Auto-generated
    assert_eq!(cover.input_labels()[3].as_ref(), "x3"); // Auto-generated

    // Grow outputs - labels SHOULD be auto-added since cover is already labeled
    cover.add_cube(
        &[Some(true), Some(false)],
        &[Some(true), Some(false), Some(true)],
    );
    assert_eq!(cover.num_outputs(), 3);
    // All 3 output labels should exist: out1, y1, y2
    assert_eq!(cover.output_labels().len(), 3);
    assert_eq!(cover.output_labels()[0].as_ref(), "out1");
    assert_eq!(cover.output_labels()[1].as_ref(), "y1"); // Auto-generated
    assert_eq!(cover.output_labels()[2].as_ref(), "y2"); // Auto-generated

    // Verify labels are properly used in expressions
    let expr = cover.to_expr_by_index(0).unwrap();
    let vars = expr.collect_variables();
    // Should have some variables from the cover
    assert!(!vars.is_empty());
}

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

#[test]
fn test_add_expr_with_existing_cubes() {
    let mut cover = Cover::new(CoverType::F);

    // Add a manual cube first - no labels are generated
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.input_labels().len(), 0); // No labels yet
    assert_eq!(cover.output_labels().len(), 0); // No labels yet
    let initial_cubes = cover.num_cubes();

    // Add an expression with variables x0, x1 - this backfills labels
    let x0 = crate::BoolExpr::variable("x0");
    let x1 = crate::BoolExpr::variable("x1");

    // Try to add to output y0 - should FAIL because y0 was backfilled
    let result = cover.add_expr(&x0.or(&x1), "y0");
    assert!(result.is_err()); // y0 already exists after backfilling

    // Add to a different output name - should succeed
    cover.add_expr(&x0.and(&x1), "y1").unwrap();
    assert_eq!(cover.num_outputs(), 2);
    assert_eq!(cover.output_labels().len(), 2);
    assert_eq!(cover.output_labels()[0].as_ref(), "y0"); // Backfilled
    assert_eq!(cover.output_labels()[1].as_ref(), "y1"); // New
    assert!(cover.num_cubes() > initial_cubes);
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
    let mut cover = Cover::new(CoverType::F);

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
    let mut cover = Cover::new(CoverType::FD);

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
    let mut cover = Cover::new(CoverType::FR);

    // FR type accepts Some(true) and Some(false)
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // F cube
    cover.add_cube(&[Some(false), Some(true)], &[Some(false)]); // R cube

    // For FR type, num_cubes() counts all cubes
    assert_eq!(cover.num_cubes(), 2);
}

#[test]
fn test_fdr_type_cover() {
    let mut cover = Cover::new(CoverType::FDR);

    // FDR type accepts all: Some(true), Some(false), None
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]); // F cube
    cover.add_cube(&[Some(false), Some(true)], &[Some(false)]); // R cube
    cover.add_cube(&[Some(true), Some(true)], &[None]); // D cube

    // For FDR type, num_cubes() counts all cubes
    assert_eq!(cover.num_cubes(), 3);
}

// ===== Mixed Operations Tests =====

#[test]
fn test_add_cubes_then_expressions() {
    let mut cover = Cover::new(CoverType::F);

    // Add manual cubes first - no labels generated
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.input_labels().len(), 0); // No labels yet
    assert_eq!(cover.output_labels().len(), 0); // No labels yet

    // Now add expression with named variables - this backfills labels for existing dimensions
    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    cover.add_expr(&a.and(&b), "y1").unwrap();

    // Should have 4 inputs now: 2 from cube (x0, x1) + 2 from expression (a, b)
    assert_eq!(cover.num_inputs(), 4);
    // All 4 should have labels: x0, x1 (backfilled), a, b (from expression)
    assert_eq!(cover.input_labels().len(), 4);
    assert_eq!(cover.input_labels()[0].as_ref(), "x0");
    assert_eq!(cover.input_labels()[1].as_ref(), "x1");
    assert_eq!(cover.input_labels()[2].as_ref(), "a");
    assert_eq!(cover.input_labels()[3].as_ref(), "b");

    // Should have 2 outputs with labels
    assert_eq!(cover.num_outputs(), 2);
    assert_eq!(cover.output_labels().len(), 2);
    assert_eq!(cover.output_labels()[0].as_ref(), "y0"); // Backfilled
    assert_eq!(cover.output_labels()[1].as_ref(), "y1"); // From expression
}

#[test]
fn test_add_expressions_then_cubes() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    // Add expression first - no backfilling needed since cover is empty
    cover.add_expr(&a.and(&b), "result").unwrap();
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");

    // Add manual cube with more inputs - should auto-extend labels since cover is in labeled mode
    cover.add_cube(
        &[Some(true), Some(false), Some(true)],
        &[Some(true), Some(false)],
    );

    // Should grow to 3 inputs, 2 outputs
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.num_outputs(), 2);

    // Original labels preserved, and new labels auto-generated
    assert_eq!(cover.input_labels().len(), 3);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");
    assert_eq!(cover.input_labels()[2].as_ref(), "x2"); // Auto-generated

    // Output labels should also be extended
    assert_eq!(cover.output_labels().len(), 2);
    assert_eq!(cover.output_labels()[0].as_ref(), "result");
    assert_eq!(cover.output_labels()[1].as_ref(), "y1"); // Auto-generated
}

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
fn test_dynamic_naming_no_collision() {
    let mut cover = Cover::new(CoverType::F);

    // Add cubes - no labels are auto-generated
    cover.add_cube(&[Some(true), Some(false), None], &[Some(true)]);
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.input_labels().len(), 0); // No labels yet

    // Now add expression with variables "x1" and "other"
    // This backfills x0, x1, x2 for existing dimensions, then x1 matches existing x1
    let x1 = crate::BoolExpr::variable("x1");
    let other = crate::BoolExpr::variable("other");

    cover.add_expr(&x1.and(&other), "y1").unwrap();

    // Should have 4 inputs: 3 from cube (x0, x1, x2) + 1 new (other)
    // x1 from expression matches the backfilled x1
    assert_eq!(cover.num_inputs(), 4);

    // All 4 should have labels: x0, x1, x2 (backfilled), other (from expression)
    assert_eq!(cover.input_labels().len(), 4);
    assert_eq!(cover.input_labels()[0].as_ref(), "x0");
    assert_eq!(cover.input_labels()[1].as_ref(), "x1");
    assert_eq!(cover.input_labels()[2].as_ref(), "x2");
    assert_eq!(cover.input_labels()[3].as_ref(), "other");
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

#[test]
fn test_unlabeled_cover_to_expr_uses_auto_names() {
    let mut cover = Cover::new(CoverType::F);

    // Add cube without any labels
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);

    // Convert to expression
    let expr = cover.to_expr_by_index(0).unwrap();

    // Expression should use auto-generated names x0, x1
    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 2);
    assert!(vars.contains(&std::sync::Arc::from("x0")));
    assert!(vars.contains(&std::sync::Arc::from("x1")));
}
