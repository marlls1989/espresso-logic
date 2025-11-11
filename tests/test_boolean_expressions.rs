//! Comprehensive tests for boolean expression functionality

use espresso_logic::{expr, BoolExpr, Cover, CoverType, Minimizable, PLAWriter};
use std::sync::Arc;

#[test]
fn test_cover_trait_basics() {
    use std::collections::HashMap;

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = expr!(a * b);

    // Should be able to use Cover with expressions
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.num_cubes(), 1); // a*b = 1 cube

    // Verify the cover can be converted back and evaluated correctly
    let retrieved = cover.to_expr("out").unwrap();
    let mut assignment = HashMap::new();

    // Test: a=1,b=1 → 1
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), true);
    assert!(retrieved.evaluate(&assignment));

    // Test: a=1,b=0 → 0
    assignment.insert(Arc::from("b"), false);
    assert!(!retrieved.evaluate(&assignment));
}

#[test]
fn test_xor_expression() {
    use std::collections::HashMap;

    // XOR: a*~b + ~a*b
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let xor = expr!(a * !b + !a * b);

    // Verify XOR truth table
    let mut assignment = HashMap::new();

    // 0 XOR 0 = 0
    assignment.insert(Arc::from("a"), false);
    assignment.insert(Arc::from("b"), false);
    assert!(!xor.evaluate(&assignment));

    // 0 XOR 1 = 1
    assignment.insert(Arc::from("b"), true);
    assert!(xor.evaluate(&assignment));

    // 1 XOR 0 = 1
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), false);
    assert!(xor.evaluate(&assignment));

    // 1 XOR 1 = 0
    assignment.insert(Arc::from("b"), true);
    assert!(!xor.evaluate(&assignment));
}

#[test]
fn test_xnor_expression() {
    use std::collections::HashMap;

    // XNOR: a*b + ~a*~b
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let xnor = expr!(a * b + !a * !b);

    // Verify XNOR truth table
    let mut assignment = HashMap::new();

    // 0 XNOR 0 = 1
    assignment.insert(Arc::from("a"), false);
    assignment.insert(Arc::from("b"), false);
    assert!(xnor.evaluate(&assignment));

    // 0 XNOR 1 = 0
    assignment.insert(Arc::from("b"), true);
    assert!(!xnor.evaluate(&assignment));

    // 1 XNOR 0 = 0
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), false);
    assert!(!xnor.evaluate(&assignment));

    // 1 XNOR 1 = 1
    assignment.insert(Arc::from("b"), true);
    assert!(xnor.evaluate(&assignment));
}

#[test]
fn test_minimization() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;

    // Create a redundant expression: a*b + a*b*c
    // Should minimize to just a*b
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let expr = expr!(a * b + a * b * c);

    // Test that minimization runs without error
    let minimized = expr.minimize()?;

    // After minimization, should have only a and b (c is absorbed)
    let vars = minimized.collect_variables();
    assert_eq!(vars.len(), 2);
    assert!(vars.contains(&Arc::from("a")));
    assert!(vars.contains(&Arc::from("b")));

    // Verify the minimized expression behaves like a*b
    let mut assignment = HashMap::new();

    // a=1,b=1,c=X → should be 1 (X is don't care)
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), true);
    assignment.insert(Arc::from("c"), false);
    assert!(minimized.evaluate(&assignment));
    assignment.insert(Arc::from("c"), true);
    assert!(minimized.evaluate(&assignment));

    // a=1,b=0,c=X → should be 0
    assignment.insert(Arc::from("b"), false);
    assignment.insert(Arc::from("c"), false);
    assert!(!minimized.evaluate(&assignment));
    assignment.insert(Arc::from("c"), true);
    assert!(!minimized.evaluate(&assignment));

    Ok(())
}

#[test]
fn test_de_morgan_laws() {
    use std::collections::HashMap;

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // ~(a * b) = ~a + ~b (De Morgan's law)
    let expr1 = expr!(!(a * b));

    let mut assignment = HashMap::new();

    // Test ~(a*b): a=1,b=1 → ~(1*1) = ~1 = 0
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), true);
    assert!(!expr1.evaluate(&assignment));

    // a=1,b=0 → ~(1*0) = ~0 = 1
    assignment.insert(Arc::from("b"), false);
    assert!(expr1.evaluate(&assignment));

    // a=0,b=1 → ~(0*1) = ~0 = 1
    assignment.insert(Arc::from("a"), false);
    assignment.insert(Arc::from("b"), true);
    assert!(expr1.evaluate(&assignment));

    // ~(a + b) = ~a * ~b (De Morgan's law)
    let expr2 = expr!(!(a + b));

    // Test ~(a+b): a=0,b=0 → ~(0+0) = ~0 = 1
    assignment.insert(Arc::from("a"), false);
    assignment.insert(Arc::from("b"), false);
    assert!(expr2.evaluate(&assignment));

    // a=1,b=0 → ~(1+0) = ~1 = 0
    assignment.insert(Arc::from("a"), true);
    assert!(!expr2.evaluate(&assignment));

    // a=0,b=1 → ~(0+1) = ~1 = 0
    assignment.insert(Arc::from("a"), false);
    assignment.insert(Arc::from("b"), true);
    assert!(!expr2.evaluate(&assignment));
}

#[test]
fn test_cube_iteration() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = expr!(a * b);

    // Should be able to iterate cubes via Cover
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr, "out").unwrap();
        cover
    };
    let cubes: Vec<_> = cover.cubes_iter().collect();

    // a*b should produce exactly 1 cube
    assert_eq!(cubes.len(), 1);

    // Verify the cube represents a=1, b=1, out=1
    let (inputs, outputs) = &cubes[0];
    assert_eq!(inputs.len(), 2);
    assert_eq!(outputs.len(), 1);
    assert_eq!(inputs, &vec![Some(true), Some(true)]);
    assert_eq!(outputs, &vec![Some(true)]);
}

#[test]
fn test_to_pla_string() -> Result<(), Box<dyn std::error::Error>> {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = expr!(a * b);

    // Should be able to convert to PLA string via Cover
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr, "out").unwrap();
        cover
    };
    let pla = cover.to_pla_string(espresso_logic::CoverType::F)?;

    // Should contain basic PLA structure
    assert!(pla.contains(".i"));
    assert!(pla.contains(".o"));
    assert!(pla.contains(".e"));

    Ok(())
}

#[test]
fn test_minimize_absorption() -> Result<(), Box<dyn std::error::Error>> {
    // Test absorption law: a + a*b should minimize to a
    let expr = BoolExpr::parse("a + a * b").unwrap();
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr, "out").unwrap();
        cover
    };
    // BDD automatically applies absorption during conversion: a + a*b = a
    assert_eq!(cover.num_cubes(), 1);

    let cover = cover.minimize()?;

    // After Espresso minimization: still 1 cube (BDD already optimized it)
    assert_eq!(cover.num_cubes(), 1);
    let minimized = cover.to_expr("out").unwrap();
    let vars = minimized.collect_variables();
    assert_eq!(vars.len(), 1);
    assert!(vars.contains(&Arc::from("a")));

    Ok(())
}

#[test]
fn test_minimize_consensus() -> Result<(), Box<dyn std::error::Error>> {
    // Consensus theorem: a*b + ~a*c + b*c should minimize to a*b + ~a*c
    let expr = BoolExpr::parse("a * b + ~a * c + b * c").unwrap();
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr, "out").unwrap();
        cover
    };
    // BDD automatically applies consensus during conversion: a*b + ~a*c + b*c → a*b + ~a*c
    assert_eq!(cover.num_cubes(), 2);

    let cover = cover.minimize()?;

    // After Espresso minimization: still 2 cubes (BDD already optimized it)
    assert_eq!(cover.num_cubes(), 2);
    let minimized = cover.to_expr("out").unwrap();
    let vars = minimized.collect_variables();
    assert_eq!(vars.len(), 3);

    Ok(())
}

#[test]
fn test_minimize_idempotence() -> Result<(), Box<dyn std::error::Error>> {
    // a + a should minimize to a
    let expr = BoolExpr::parse("a + a").unwrap();
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr, "out").unwrap();
        cover
    };
    // BDD canonicalizes identical expressions: a + a = a
    assert_eq!(cover.num_cubes(), 1);

    let cover = cover.minimize()?;

    // After Espresso minimization: still 1 cube (BDD already optimized it)
    assert_eq!(cover.num_cubes(), 1);
    let minimized = cover.to_expr("out").unwrap();
    let vars = minimized.collect_variables();
    assert_eq!(vars.len(), 1);
    assert!(vars.contains(&Arc::from("a")));

    Ok(())
}

#[test]
fn test_complex_parentheses() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Test with expr! macro
    let expr1 = expr!((a + b) * c);
    let cover1 = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr1, "out").unwrap();
        cover
    };
    assert_eq!(cover1.num_inputs(), 3);

    // Parser version
    let expr2 = BoolExpr::parse("(a + b) * c").unwrap();
    let mut cover2 = Cover::new(CoverType::F);
    cover2.add_expr(&expr2, "out").unwrap();
    assert_eq!(cover2.num_inputs(), 3);

    // Both should have same variables
    assert_eq!(expr1.collect_variables(), expr2.collect_variables());
}

#[test]
fn test_nested_parentheses_macro() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");

    // Test macro with nested parens
    let expr1 = expr!(a * (b + c));
    let cover1 = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr1, "out").unwrap();
        cover
    };
    assert_eq!(cover1.num_inputs(), 3);

    // Compare with parser
    let expr2 = BoolExpr::parse("a * (b + c)").unwrap();
    assert_eq!(expr1.collect_variables(), expr2.collect_variables());

    // More complex: (a + b) * (c + d)
    let expr3 = expr!((a + b) * (c + d));
    let expr4 = BoolExpr::parse("(a + b) * (c + d)").unwrap();
    assert_eq!(expr3.collect_variables(), expr4.collect_variables());
}

#[test]
fn test_parentheses_precedence() {
    // (a + b) * c is different from a + b * c
    let expr1 = BoolExpr::parse("(a + b) * c").unwrap();
    let expr2 = BoolExpr::parse("a + b * c").unwrap();

    // Both have same variables
    assert_eq!(expr1.collect_variables(), expr2.collect_variables());

    // But different structure - verify by converting to PLA via Cover
    let cover1 = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr1, "out").unwrap();
        cover
    };
    let cover2 = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr2, "out").unwrap();
        cover
    };
    let pla1 = cover1.to_pla_string(espresso_logic::CoverType::F).unwrap();
    let pla2 = cover2.to_pla_string(espresso_logic::CoverType::F).unwrap();

    // Different number of cubes means different logic
    assert_ne!(pla1, pla2);
}

#[test]
fn test_minimize_distributive() -> Result<(), Box<dyn std::error::Error>> {
    // a*(b+c) expands to a*b + a*c (already minimal)
    let expr = BoolExpr::parse("a * (b + c)").unwrap();
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_cubes(), 2); // Expands to 2 cubes

    let cover = cover.minimize()?;

    // After Espresso minimization: stays at 2 cubes (already minimal)
    assert_eq!(cover.num_cubes(), 2);
    let minimized = cover.to_expr("out").unwrap();
    let vars = minimized.collect_variables();
    assert_eq!(vars.len(), 3);

    Ok(())
}

#[test]
fn test_complex_minimize_real_world() -> Result<(), Box<dyn std::error::Error>> {
    // Real-world example: a*b + a*c + b*c*d + a*b*d
    let expr = BoolExpr::parse("a * b + a * c + b * c * d + a * b * d").unwrap();
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr, "out").unwrap();
        cover
    };
    // BDD performs some optimization, but not all
    let cubes_before = cover.num_cubes();

    let cover = cover.minimize()?;

    // After Espresso minimization: should reduce further
    assert!(cover.num_cubes() <= cubes_before);
    let minimized = cover.to_expr("out").unwrap();
    let vars = minimized.collect_variables();
    assert_eq!(vars.len(), 4);

    Ok(())
}

#[test]
fn test_minimize_adjacent_minterms() -> Result<(), Box<dyn std::error::Error>> {
    // Test Espresso minimization on adjacent minterms
    // f(a,b,c) = m0 + m1 + m2 + m3 (all combinations where a=0)
    let expr = BoolExpr::parse("~a * ~b * ~c + ~a * ~b * c + ~a * b * ~c + ~a * b * c").unwrap();
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr, "out").unwrap();
        cover
    };
    // BDD canonicalizes adjacent minterms: ~a * ~b * ~c + ~a * ~b * c + ~a * b * ~c + ~a * b * c → ~a
    assert_eq!(cover.num_cubes(), 1);

    let cover = cover.minimize()?;

    // After Espresso minimization: still 1 cube (BDD already optimized it to ~a)
    assert_eq!(cover.num_cubes(), 1);
    let minimized = cover.to_expr("out").unwrap();
    let vars = minimized.collect_variables();
    assert_eq!(vars.len(), 1);
    assert!(vars.contains(&Arc::from("a")));

    Ok(())
}

#[test]
fn test_parentheses_with_negation() {
    // Test negation of parenthesized expressions
    let expr1 = BoolExpr::parse("~(a * b)").unwrap();
    let expr2 = BoolExpr::parse("~(a + b)").unwrap();

    // Should apply De Morgan's laws during DNF conversion
    let cover1 = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr1, "out").unwrap();
        cover
    };
    let cover2 = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr2, "out").unwrap();
        cover
    };
    assert_eq!(cover1.num_inputs(), 2);
    assert_eq!(cover2.num_inputs(), 2);
}

#[test]
fn test_nested_parentheses_minimize() -> Result<(), Box<dyn std::error::Error>> {
    // (a + b) * (a + c) expands to a + a*c + a*b + b*c = a + b*c
    let expr = BoolExpr::parse("(a + b) * (a + c)").unwrap();
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(&expr, "out").unwrap();
        cover
    };
    // BDD optimizes: (a + b) * (a + c) = a + b*c
    assert_eq!(cover.num_cubes(), 2);

    let cover = cover.minimize()?;

    // After Espresso minimization: still 2 cubes (BDD already optimized it)
    assert_eq!(cover.num_cubes(), 2);
    let minimized = cover.to_expr("out").unwrap();
    let vars = minimized.collect_variables();
    assert_eq!(vars.len(), 3);

    Ok(())
}

#[test]
fn test_cover_without_labels_to_expr() {
    // Test the edge case: Create a cover without explicit labels,
    // add cubes manually, and convert back to expressions.
    // Labels should NOT be generated until needed, and expressions should use default names.

    let mut cover = Cover::new(CoverType::F);

    // Verify cover starts empty with no labels
    assert_eq!(cover.num_inputs(), 0);
    assert_eq!(cover.num_outputs(), 0);
    assert_eq!(cover.input_labels().len(), 0);
    assert_eq!(cover.output_labels().len(), 0);

    // Add cubes manually (not via add_expr) - labels should NOT be auto-generated
    // Represent: x0 * x1 + ~x0 * ~x1 (XOR pattern for output y0)
    cover.add_cube(&[Some(true), Some(true)], &[Some(true)]); // x0 * x1 -> y0
    cover.add_cube(&[Some(false), Some(false)], &[Some(true)]); // ~x0 * ~x1 -> y0

    // Verify dimensions grew but labels were NOT auto-generated
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.input_labels().len(), 0); // No labels yet!
    assert_eq!(cover.output_labels().len(), 0); // No labels yet!

    // Convert to expression using index (can't use name since there are no labels)
    let expr = cover.to_expr_by_index(0).unwrap();
    let expr_str = format!("{}", expr);

    // The expression should use default generated variable names (x0, x1)
    assert!(expr_str.contains("x0"));
    assert!(expr_str.contains("x1"));

    // Verify the variables in the expression
    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 2);
    let var_names: Vec<&str> = vars.iter().map(|v| v.as_ref()).collect();
    assert!(var_names.contains(&"x0"));
    assert!(var_names.contains(&"x1"));

    // The labels in the cover should still be empty even after conversion
    assert_eq!(cover.input_labels().len(), 0);
    assert_eq!(cover.output_labels().len(), 0);

    // Try to get out-of-bounds output by index - should fail
    let result = cover.to_expr_by_index(1);
    assert!(result.is_err());
}

#[test]
fn test_cover_without_labels_multiple_outputs() {
    // Test with multiple outputs, labels should NOT be generated automatically
    let mut cover = Cover::new(CoverType::F);

    // Add cubes with 2 inputs and 2 outputs
    // Output 0: x0 * x1
    cover.add_cube(&[Some(true), Some(true)], &[Some(true), None]);
    // Output 1: ~x0
    cover.add_cube(&[Some(false), None], &[None, Some(true)]);

    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 2);
    assert_eq!(cover.num_cubes(), 2);

    // Labels should NOT be auto-generated
    assert_eq!(cover.output_labels().len(), 0);
    assert_eq!(cover.input_labels().len(), 0);

    // Get expression for first output by index
    let expr0 = cover.to_expr_by_index(0).unwrap();
    let expr0_str = format!("{}", expr0);
    assert!(expr0_str.contains("x0"));
    assert!(expr0_str.contains("x1"));

    // Get expression for second output by index
    let expr1 = cover.to_expr_by_index(1).unwrap();
    let expr1_str = format!("{}", expr1);
    assert!(expr1_str.contains("x0"));

    // Test iterator over all expressions - should generate default names
    let exprs: Vec<(Arc<str>, BoolExpr)> = cover.to_exprs().collect();
    assert_eq!(exprs.len(), 2);
    assert_eq!(exprs[0].0.as_ref(), "y0"); // Generated on-the-fly
    assert_eq!(exprs[1].0.as_ref(), "y1"); // Generated on-the-fly

    // Labels in the cover should still be empty
    assert_eq!(cover.output_labels().len(), 0);
}

#[test]
fn test_cover_without_labels_minimize_and_convert() -> Result<(), Box<dyn std::error::Error>> {
    // Test minimization without labels, then convert to expressions
    let mut cover = Cover::new(CoverType::F);

    // Add redundant cubes: x0*x1 + x0*x1*x2 should minimize to x0*x1
    cover.add_cube(&[Some(true), Some(true), None], &[Some(true)]);
    cover.add_cube(&[Some(true), Some(true), Some(true)], &[Some(true)]);

    assert_eq!(cover.num_cubes(), 2);

    // Labels should not exist yet
    assert_eq!(cover.input_labels().len(), 0);
    assert_eq!(cover.output_labels().len(), 0);

    // Minimize
    cover = cover.minimize()?;

    // Should reduce to 1 cube
    assert_eq!(cover.num_cubes(), 1);

    // Labels should still not exist after minimization
    assert_eq!(cover.input_labels().len(), 0);
    assert_eq!(cover.output_labels().len(), 0);

    // Convert to expression by index - should use default generated names
    let expr = cover.to_expr_by_index(0)?;
    let vars = expr.collect_variables();

    // Should only have x0 and x1 after minimization (x2 becomes don't care)
    assert_eq!(vars.len(), 2);
    let var_names: Vec<&str> = vars.iter().map(|v| v.as_ref()).collect();
    assert!(var_names.contains(&"x0"));
    assert!(var_names.contains(&"x1"));

    // Labels in the cover should still be empty even after expression conversion
    assert_eq!(cover.input_labels().len(), 0);
    assert_eq!(cover.output_labels().len(), 0);

    Ok(())
}

#[test]
fn test_cover_empty_to_expr() {
    // Edge case: Cover with dimensions but no cubes
    let cover = Cover::with_labels(CoverType::F, &["a", "b"], &["out"]);

    // Try to get expression for output with no cubes - should return constant false
    let expr = cover.to_expr("out").unwrap();
    let expr_str = format!("{}", expr);
    assert_eq!(expr_str, "0");

    // Verify it's actually a constant false
    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 0);
}

// ========== Expression Composition Tests ==========

#[test]
fn test_composition_nested_sub_expressions() {
    // Build deeply nested composition: ((a*b) + (c+d)) * e
    let level1_a = expr!("a" * "b");
    let level1_b = expr!("c" + "d");
    let level2 = expr!(level1_a + level1_b);
    let level3 = expr!(level2 * "e");

    let expected = BoolExpr::parse("((a * b) + (c + d)) * e").unwrap();

    // Use exhaustive truth table check
    assert!(level3.equivalent_to(&expected));
}

#[test]
fn test_composition_with_cover_integration() {
    // Build expression through composition, then use with Cover
    let term1 = expr!("a" * "b");
    let term2 = expr!("c" + "d");
    let composed = expr!(term1 * term2); // (a*b) * (c+d) = a*b*c + a*b*d

    let mut cover = Cover::new(CoverType::F);
    cover.add_expr(&composed, "output").unwrap();

    assert_eq!(cover.num_inputs(), 4);
    assert_eq!(cover.num_outputs(), 1);
    // Should expand to 2 cubes: a*b*c and a*b*d
    assert_eq!(cover.num_cubes(), 2);

    // Verify the expression can be retrieved and is equivalent to what we put in
    let retrieved = cover.to_expr("output").unwrap();
    let expected = BoolExpr::parse("(a * b) * (c + d)").unwrap();

    // Use exhaustive truth table check
    assert!(retrieved.equivalent_to(&expected));
}

// ========== Negative Tests (Error Handling) ==========

#[test]
fn test_parser_error_empty_string() {
    assert!(BoolExpr::parse("").is_err());
}

#[test]
fn test_parser_error_invalid_syntax() {
    // Double operators
    assert!(BoolExpr::parse("a * * b").is_err());
    assert!(BoolExpr::parse("a + + b").is_err());
    assert!(BoolExpr::parse("* a").is_err());
    assert!(BoolExpr::parse("+ b").is_err());
}

#[test]
fn test_parser_error_unbalanced_parentheses() {
    assert!(BoolExpr::parse("(a + b").is_err());
    assert!(BoolExpr::parse("a + b)").is_err());
    assert!(BoolExpr::parse("((a + b)").is_err());
    assert!(BoolExpr::parse("(a + b))").is_err());
}

#[test]
fn test_parser_error_missing_operator() {
    assert!(BoolExpr::parse("a b").is_err());
    assert!(BoolExpr::parse("a b c").is_err());
    assert!(BoolExpr::parse("(a)(b)").is_err());
}

#[test]
fn test_parser_error_missing_operand() {
    assert!(BoolExpr::parse("a +").is_err());
    assert!(BoolExpr::parse("* b").is_err());
    assert!(BoolExpr::parse("a + * b").is_err());
}

#[test]
fn test_cover_error_nonexistent_output() {
    let mut cover = Cover::new(CoverType::F);
    let a = BoolExpr::variable("a");
    cover.add_expr(&a, "out1").unwrap();

    // Try to get non-existent output
    let result = cover.to_expr("nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_cover_error_duplicate_output() {
    let mut cover = Cover::new(CoverType::F);
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    cover.add_expr(&a, "out").unwrap();

    // Try to add to same output again
    let result = cover.add_expr(&b, "out");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn test_cover_error_output_index_out_of_bounds() {
    let mut cover = Cover::new(CoverType::F);
    let a = BoolExpr::variable("a");
    cover.add_expr(&a, "out").unwrap();

    // Try to access out of bounds index
    let result = cover.to_expr_by_index(1);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("out of bounds"));
}
