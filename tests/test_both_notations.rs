use espresso_logic::{expr, BoolExpr};
use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn test_parser_both_and_notations() {
    // Test with * notation
    let expr1 = BoolExpr::parse("a * b").unwrap();
    // Test with & notation
    let expr2 = BoolExpr::parse("a & b").unwrap();

    // Both should produce identical results
    let mut assignment = HashMap::new();
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), true);

    assert!(expr1.evaluate(&assignment));
    assert!(expr2.evaluate(&assignment));

    assignment.insert(Arc::from("b"), false);
    assert!(!expr1.evaluate(&assignment));
    assert!(!expr2.evaluate(&assignment));
}

#[test]
fn test_parser_both_or_notations() {
    // Test with + notation
    let expr1 = BoolExpr::parse("a + b").unwrap();
    // Test with | notation
    let expr2 = BoolExpr::parse("a | b").unwrap();

    // Both should produce identical results
    let mut assignment = HashMap::new();
    assignment.insert(Arc::from("a"), false);
    assignment.insert(Arc::from("b"), true);

    assert!(expr1.evaluate(&assignment));
    assert!(expr2.evaluate(&assignment));

    assignment.insert(Arc::from("b"), false);
    assert!(!expr1.evaluate(&assignment));
    assert!(!expr2.evaluate(&assignment));
}

#[test]
fn test_parser_mixed_notations() {
    // Test mixing * and & for AND
    let expr1 = BoolExpr::parse("a * b & c").unwrap();
    // Test mixing + and | for OR
    let expr2 = BoolExpr::parse("a + b | c").unwrap();

    let mut assignment = HashMap::new();
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), true);
    assignment.insert(Arc::from("c"), true);

    // a * b & c = (a * b) & c = true & true = true
    assert!(expr1.evaluate(&assignment));
    // a + b | c = (a + b) | c = true | true = true
    assert!(expr2.evaluate(&assignment));
}

#[test]
fn test_parser_complex_mixed_expression() {
    // Test complex expression with both notations
    let expr = BoolExpr::parse("(a & b) + (c * d) | !e").unwrap();

    let mut assignment = HashMap::new();
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), true);
    assignment.insert(Arc::from("c"), false);
    assignment.insert(Arc::from("d"), true);
    assignment.insert(Arc::from("e"), false);

    // (true & true) + (false * true) | !false
    // = true + false | true
    // = true | true
    // = true
    assert!(expr.evaluate(&assignment));
}

#[test]
fn test_expr_macro_both_and_notations() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Test with * notation
    let expr1 = expr!(a * b);
    // Test with & notation
    let expr2 = expr!(a & b);

    let mut assignment = HashMap::new();
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), true);

    assert!(expr1.evaluate(&assignment));
    assert!(expr2.evaluate(&assignment));

    assignment.insert(Arc::from("b"), false);
    assert!(!expr1.evaluate(&assignment));
    assert!(!expr2.evaluate(&assignment));
}

#[test]
fn test_expr_macro_both_or_notations() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Test with + notation
    let expr1 = expr!(a + b);
    // Test with | notation
    let expr2 = expr!(a | b);

    let mut assignment = HashMap::new();
    assignment.insert(Arc::from("a"), false);
    assignment.insert(Arc::from("b"), true);

    assert!(expr1.evaluate(&assignment));
    assert!(expr2.evaluate(&assignment));

    assignment.insert(Arc::from("b"), false);
    assert!(!expr1.evaluate(&assignment));
    assert!(!expr2.evaluate(&assignment));
}

#[test]
fn test_expr_macro_string_literals_both_notations() {
    // Test with string literals and & notation
    let expr1 = expr!("a" & "b");
    // Test with string literals and | notation
    let expr2 = expr!("a" | "b");

    let mut assignment = HashMap::new();
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), false);

    assert!(!expr1.evaluate(&assignment));
    assert!(expr2.evaluate(&assignment));
}

#[test]
fn test_expr_macro_mixed_notations() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Mix * and & for AND
    let expr1 = expr!(a * b & c);
    // Mix + and | for OR
    let expr2 = expr!(a + b | c);

    let mut assignment = HashMap::new();
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), true);
    assignment.insert(Arc::from("c"), true);

    assert!(expr1.evaluate(&assignment));
    assert!(expr2.evaluate(&assignment));
}

#[test]
fn test_expr_macro_complex_mixed() {
    // Complex expression with both notations and string literals
    let expr = expr!(("a" & "b") + ("c" * "d") | !"e");

    let mut assignment = HashMap::new();
    assignment.insert(Arc::from("a"), true);
    assignment.insert(Arc::from("b"), true);
    assignment.insert(Arc::from("c"), false);
    assignment.insert(Arc::from("d"), true);
    assignment.insert(Arc::from("e"), false);

    // (true & true) + (false * true) | !false
    // = true + false | true
    // = true | true
    // = true
    assert!(expr.evaluate(&assignment));
}

#[test]
fn test_precedence_with_both_notations() {
    // Test that precedence is correct with both notations
    // a | b & c should be a | (b & c)
    let expr1 = BoolExpr::parse("a | b & c").unwrap();
    let expr2 = BoolExpr::parse("a | (b & c)").unwrap();

    let mut assignment = HashMap::new();
    assignment.insert(Arc::from("a"), false);
    assignment.insert(Arc::from("b"), true);
    assignment.insert(Arc::from("c"), false);

    assert_eq!(expr1.evaluate(&assignment), expr2.evaluate(&assignment));

    // a + b * c should be a + (b * c)
    let expr3 = BoolExpr::parse("a + b * c").unwrap();
    let expr4 = BoolExpr::parse("a + (b * c)").unwrap();

    assert_eq!(expr3.evaluate(&assignment), expr4.evaluate(&assignment));
}

#[test]
fn test_xor_with_both_notations() {
    // XOR with & and | notation
    let xor1 = BoolExpr::parse("a & !b | !a & b").unwrap();
    // XOR with * and + notation
    let xor2 = BoolExpr::parse("a * !b + !a * b").unwrap();

    let mut assignment = HashMap::new();

    // Test all combinations
    for a in [false, true] {
        for b in [false, true] {
            assignment.insert(Arc::from("a"), a);
            assignment.insert(Arc::from("b"), b);

            let expected = a ^ b; // XOR
            assert_eq!(xor1.evaluate(&assignment), expected);
            assert_eq!(xor2.evaluate(&assignment), expected);

            // Both notations should give the same result
            assert_eq!(xor1.evaluate(&assignment), xor2.evaluate(&assignment));
        }
    }
}
