//! Comprehensive tests for boolean expression functionality

use espresso_logic::{expr, BoolExpr, Cover, CoverType, PLAWriter};
use std::sync::Arc;

#[test]
fn test_parse_simple_variable() {
    let expr = BoolExpr::parse("a").unwrap();
    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 1);
}

#[test]
fn test_parse_and() {
    let expr = BoolExpr::parse("a * b").unwrap();
    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 2);
}

#[test]
fn test_parse_or() {
    let expr = BoolExpr::parse("a + b").unwrap();
    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 2);
}

#[test]
fn test_parse_not() {
    let expr1 = BoolExpr::parse("~a").unwrap();
    let expr2 = BoolExpr::parse("!a").unwrap();

    // Both should have same variable
    assert_eq!(expr1.collect_variables().len(), 1);
    assert_eq!(expr2.collect_variables().len(), 1);
}

#[test]
fn test_parse_parentheses() {
    let expr = BoolExpr::parse("(a + b) * c").unwrap();
    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 3);
}

#[test]
fn test_parse_complex() {
    let expr = BoolExpr::parse("(a * b) + (~a * ~b)").unwrap();
    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 2);
}

#[test]
fn test_parse_constants() {
    let t1 = BoolExpr::parse("1").unwrap();
    let t2 = BoolExpr::parse("true").unwrap();
    let f1 = BoolExpr::parse("0").unwrap();
    let f2 = BoolExpr::parse("false").unwrap();

    assert_eq!(t1.collect_variables().len(), 0);
    assert_eq!(t2.collect_variables().len(), 0);
    assert_eq!(f1.collect_variables().len(), 0);
    assert_eq!(f2.collect_variables().len(), 0);
}

#[test]
fn test_parse_multi_char_variables() {
    let expr = BoolExpr::parse("input_a * input_b + output_c").unwrap();
    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 3);

    let var_names: Vec<String> = vars.iter().map(|s| s.to_string()).collect();
    assert_eq!(var_names, vec!["input_a", "input_b", "output_c"]);
}

#[test]
fn test_precedence() {
    // NOT > AND > OR precedence
    let expr1 = BoolExpr::parse("~a * b + c").unwrap();
    let expr2 = BoolExpr::parse("((~a) * b) + c").unwrap();

    // Both should parse the same way
    assert_eq!(expr1.collect_variables(), expr2.collect_variables());
}

#[test]
fn test_method_api_integration() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Create expression using method API
    let expr1 = a.and(&b).or(&a.not().and(&b.not()));

    // Parse equivalent expression
    let expr2 = BoolExpr::parse("a * b + ~a * ~b").unwrap();

    // Should have same variables
    assert_eq!(expr1.collect_variables(), expr2.collect_variables());
}

#[test]
fn test_macro_vs_method_api() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Same expression built two ways
    let via_macro = expr!(a * b + !a * !b);
    let via_method = a.and(&b).or(&a.not().and(&b.not()));

    // Should have identical variables
    assert_eq!(
        via_macro.collect_variables(),
        via_method.collect_variables()
    );

    // Both should convert to covers with same properties
    let cover_macro = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(via_macro, "out").unwrap();
        cover
    };
    let cover_method = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(via_method, "out").unwrap();
        cover
    };
    assert_eq!(cover_macro.num_inputs(), cover_method.num_inputs());
    assert_eq!(cover_macro.num_outputs(), cover_method.num_outputs());
}

#[test]
fn test_macro_vs_parser() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Same expression via macro and parser
    let via_macro = expr!(a * b + !a * !b);
    let via_parser = BoolExpr::parse("a * b + ~a * ~b").unwrap();

    // Should have identical structure
    assert_eq!(
        via_macro.collect_variables(),
        via_parser.collect_variables()
    );
}

#[test]
fn test_cover_trait_basics() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = expr!(a * b);

    // Should be able to use Cover with expressions
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);
}

#[test]
fn test_xor_expression() {
    // XOR: a*~b + ~a*b
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let xor = expr!(a * !b + !a * b);

    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(xor, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);
}

#[test]
fn test_xnor_expression() {
    // XNOR: a*b + ~a*~b
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let xnor = expr!(a * b + !a * !b);

    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(xnor, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);
}

#[test]
fn test_minimization() -> Result<(), Box<dyn std::error::Error>> {
    // Create a redundant expression: a*b + a*b*c
    // Should minimize to just a*b
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let expr = expr!(a * b + a * b * c);

    // Test that minimization runs without error using convenience method
    let minimized = expr.minimize()?;

    // After minimization, should still have the same variables
    let vars = minimized.collect_variables();
    assert!(vars.len() >= 2); // At least a and b should remain

    Ok(())
}

#[test]
fn test_de_morgan_laws() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // ~(a * b) should expand using De Morgan's law
    let expr1 = expr!(!(a * b));

    // ~(a + b) should expand using De Morgan's law
    let expr2 = expr!(!(a + b));

    let cover1 = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr1, "out").unwrap();
        cover
    };
    let cover2 = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr2, "out").unwrap();
        cover
    };
    assert_eq!(cover1.num_inputs(), 2);
    assert_eq!(cover2.num_inputs(), 2);
}

#[test]
fn test_constant_propagation() {
    let a = BoolExpr::variable("a");
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);

    // a * true should still have variable a
    let expr1 = expr!(a * t);
    assert!(!expr1.collect_variables().is_empty());

    // a * false should have variable a (even though it could simplify to false)
    let expr2 = expr!(a * f);
    assert!(!expr2.collect_variables().is_empty());

    // a + true should have variable a
    let one = BoolExpr::constant(true);
    let expr3 = expr!(a + one);
    assert!(!expr3.collect_variables().is_empty());
}

#[test]
fn test_parse_error_handling() {
    // Test various invalid inputs
    assert!(BoolExpr::parse("").is_err());
    assert!(BoolExpr::parse("a +").is_err());
    assert!(BoolExpr::parse("* b").is_err());
    assert!(BoolExpr::parse("(a + b").is_err()); // Missing closing paren
    assert!(BoolExpr::parse("a b").is_err()); // Missing operator
}

#[test]
fn test_variable_ordering() {
    // Variables should be in alphabetical order
    let expr = BoolExpr::parse("z + a + m").unwrap();
    let vars = expr.collect_variables();
    let var_names: Vec<String> = vars.iter().map(|s| s.to_string()).collect();

    // Should be sorted
    let mut sorted = var_names.clone();
    sorted.sort();
    assert_eq!(var_names, sorted);
}

#[test]
fn test_large_expression() {
    // Test with many variables
    let expr = BoolExpr::parse("(a * b * c) + (d * e * f) + (g * h * i) + (j * k * l)").unwrap();

    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 12);
}

#[test]
fn test_deeply_nested() {
    // Test deeply nested expression
    let expr = BoolExpr::parse("((((a * b) + c) * d) + e)").unwrap();

    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 5);
}

#[test]
fn test_cube_iteration() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = expr!(a * b);

    // Should be able to iterate cubes via Cover
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr, "out").unwrap();
        cover
    };
    let cubes: Vec<_> = cover.cubes_iter().collect();
    assert!(!cubes.is_empty());
}

#[test]
fn test_to_pla_string() -> Result<(), Box<dyn std::error::Error>> {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = expr!(a * b);

    // Should be able to convert to PLA string via Cover
    let cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr, "out").unwrap();
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
fn test_clone_semantics() {
    let a = BoolExpr::variable("a");
    let b = a.clone();

    // Cloning should be cheap and share the same Arc
    assert_eq!(a, b);

    // After cloning, both should still work independently
    let x = BoolExpr::variable("x");
    let y = BoolExpr::variable("y");
    let expr1 = expr!(a + x);
    let expr2 = expr!(b + y);

    assert_ne!(expr1.collect_variables(), expr2.collect_variables());
}

#[test]
fn test_minimize_absorption() -> Result<(), Box<dyn std::error::Error>> {
    // Test absorption law: a + a*b should minimize to a
    let expr = BoolExpr::parse("a + a * b").unwrap();
    let mut cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_cubes(), 2); // Before: 2 cubes

    cover.minimize()?;

    // After Espresso minimization: should reduce to 1 cube (just 'a')
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
    let mut cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_cubes(), 3); // Before: 3 cubes

    cover.minimize()?;

    // After Espresso minimization: should reduce to 2 cubes (b*c is redundant)
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
    let mut cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_cubes(), 2); // Before: 2 identical cubes

    cover.minimize()?;

    // After Espresso minimization: should reduce to 1 cube
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
        cover.add_expr(expr1.clone(), "out").unwrap();
        cover
    };
    assert_eq!(cover1.num_inputs(), 3);

    // Parser version
    let expr2 = BoolExpr::parse("(a + b) * c").unwrap();
    let mut cover2 = Cover::new(CoverType::F);
    cover2.add_expr(expr2.clone(), "out").unwrap();
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
        cover.add_expr(expr1.clone(), "out").unwrap();
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
fn test_deeply_nested_parentheses() {
    // Test very complex nested expression
    let expr = BoolExpr::parse("((a + b) * (c + d)) + ((e + f) * (g + h))").unwrap();

    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 8);

    let var_names: Vec<String> = vars.iter().map(|s| s.to_string()).collect();
    assert_eq!(var_names, vec!["a", "b", "c", "d", "e", "f", "g", "h"]);
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
        cover.add_expr(expr1, "out").unwrap();
        cover
    };
    let cover2 = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr2, "out").unwrap();
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
    let mut cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_cubes(), 2); // Expands to 2 cubes

    cover.minimize()?;

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
    let mut cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_cubes(), 4); // Before: 4 cubes

    cover.minimize()?;

    // After Espresso minimization: should reduce to 3 cubes (a*b*d covered by a*b)
    assert_eq!(cover.num_cubes(), 3);
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
    let mut cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_cubes(), 4); // Before: 4 cubes

    cover.minimize()?;

    // Espresso minimizes to single cube: ~a
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
        cover.add_expr(expr1, "out").unwrap();
        cover
    };
    let cover2 = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr2, "out").unwrap();
        cover
    };
    assert_eq!(cover1.num_inputs(), 2);
    assert_eq!(cover2.num_inputs(), 2);
}

#[test]
fn test_nested_parentheses_minimize() -> Result<(), Box<dyn std::error::Error>> {
    // (a + b) * (a + c) expands to a + a*c + a*b + b*c = a + b*c
    let expr = BoolExpr::parse("(a + b) * (a + c)").unwrap();
    let mut cover = {
        let mut cover = Cover::new(CoverType::F);
        cover.add_expr(expr, "out").unwrap();
        cover
    };
    assert_eq!(cover.num_cubes(), 4); // Expands to 4 products

    cover.minimize()?;

    // After Espresso minimization: should reduce to 2 cubes (a + b*c)
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
    cover.minimize()?;

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
