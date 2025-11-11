//! Tests for the expression module

use super::*;
use crate::expr;

#[test]
fn test_variable_creation() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let a2 = BoolExpr::variable("a");

    // Variables are compared by structure
    assert_eq!(a, a2);
    assert_ne!(a, b);
}

#[test]
fn test_constant_creation() {
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);

    assert_eq!(t, BoolExpr::constant(true));
    assert_ne!(t, f);
}

#[test]
fn test_collect_variables() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Using method API
    let expr = a.and(&b).or(&c);
    let vars = expr.collect_variables();

    assert_eq!(vars.len(), 3);
    let var_names: Vec<String> = vars.iter().map(|s| s.to_string()).collect();
    assert_eq!(var_names, vec!["a", "b", "c"]); // Should be alphabetical
}

// ========== Display Formatting Tests ==========

#[test]
fn test_display_simple_variable() {
    let a = BoolExpr::variable("a");
    assert_eq!(format!("{}", a), "a");
    assert_eq!(format!("{:?}", a), "a");
}

#[test]
fn test_display_constants() {
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);
    assert_eq!(format!("{}", t), "1");
    assert_eq!(format!("{}", f), "0");
}

#[test]
fn test_display_simple_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&b);

    // Simple AND should have no parentheses
    assert_eq!(format!("{}", expr), "a * b");
}

#[test]
fn test_display_simple_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.or(&b);

    // Simple OR should have no parentheses
    assert_eq!(format!("{}", expr), "a + b");
}

#[test]
fn test_display_simple_not() {
    let a = BoolExpr::variable("a");
    let expr = a.not();

    // NOT of variable should have no parentheses
    assert_eq!(format!("{}", expr), "~a");
}

#[test]
fn test_display_and_then_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr = a.and(&b).or(&c);

    // AND has higher precedence than OR, so no parentheses needed
    assert_eq!(format!("{}", expr), "a * b + c");
}

#[test]
fn test_display_or_then_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr = a.or(&b).and(&c);

    // OR has lower precedence, needs parentheses when inside AND
    assert_eq!(format!("{}", expr), "(a + b) * c");
}

#[test]
fn test_display_multiple_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr = a.and(&b).and(&c);

    // Chained AND operations, no parentheses needed
    assert_eq!(format!("{}", expr), "a * b * c");
}

#[test]
fn test_display_multiple_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr = a.or(&b).or(&c);

    // Chained OR operations, no parentheses needed
    assert_eq!(format!("{}", expr), "a + b + c");
}

#[test]
fn test_display_not_of_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&b).not();

    // NOT of compound expression needs parentheses
    assert_eq!(format!("{}", expr), "~(a * b)");
}

#[test]
fn test_display_not_of_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.or(&b).not();

    // NOT of compound expression needs parentheses
    assert_eq!(format!("{}", expr), "~(a + b)");
}

#[test]
fn test_display_xor_like() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    // XOR-like: a*b + ~a*~b
    let expr = a.and(&b).or(&a.not().and(&b.not()));

    // No unnecessary parentheses
    assert_eq!(format!("{}", expr), "a * b + ~a * ~b");
}

#[test]
fn test_display_xnor_like() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    // XOR-like (not XNOR): a*~b + ~a*b
    // Build using reference NOT operator
    let expr = a.and(&(!&b)).or(&(!&a).and(&b));

    // No unnecessary parentheses
    assert_eq!(format!("{}", expr), "a * ~b + ~a * b");
}

#[test]
fn test_display_complex_nested() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");

    // (a + b) * (c + d)
    let expr = a.or(&b).and(&c.or(&d));

    // Both ORs need parentheses when inside AND
    assert_eq!(format!("{}", expr), "(a + b) * (c + d)");
}

#[test]
fn test_display_nested_or_in_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // a * (b + c)
    let expr = a.and(&b.or(&c));

    // OR needs parentheses when inside AND
    assert_eq!(format!("{}", expr), "a * (b + c)");
}

#[test]
fn test_display_nested_and_in_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // a + b * c
    let expr = a.or(&b.and(&c));

    // AND has higher precedence, no parentheses needed
    assert_eq!(format!("{}", expr), "a + b * c");
}

#[test]
fn test_display_double_negation() {
    let a = BoolExpr::variable("a");
    let expr = a.not().not();

    // Double negation
    assert_eq!(format!("{}", expr), "~~a");
}

#[test]
fn test_display_not_in_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.not().and(&b);

    // NOT has highest precedence, no extra parens
    assert_eq!(format!("{}", expr), "~a * b");
}

#[test]
fn test_display_not_in_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.not().or(&b);

    // NOT has highest precedence, no extra parens
    assert_eq!(format!("{}", expr), "~a + b");
}

#[test]
fn test_display_majority_function() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Majority: a*b + b*c + a*c
    let expr = a.and(&b).or(&b.and(&c)).or(&a.and(&c));

    // Clean formatting with no unnecessary parentheses
    assert_eq!(format!("{}", expr), "a * b + b * c + a * c");
}

#[test]
fn test_display_with_constants() {
    let a = BoolExpr::variable("a");
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);

    assert_eq!(format!("{}", a.and(&t)), "a * 1");
    assert_eq!(format!("{}", a.or(&f)), "a + 0");
    assert_eq!(format!("{}", t.not()), "~1");
}

#[test]
fn test_display_deeply_nested() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");

    // ((a + b) * c) + d - should minimize parens
    let expr = a.or(&b).and(&c).or(&d);
    assert_eq!(format!("{}", expr), "(a + b) * c + d");

    // a * ((b + c) * d)
    let expr2 = a.and(&b.or(&c).and(&d));
    assert_eq!(format!("{}", expr2), "a * (b + c) * d");
}

#[test]
fn test_display_not_of_complex() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // ~(a*b + c)
    let expr = a.and(&b).or(&c).not();
    assert_eq!(format!("{}", expr), "~(a * b + c)");

    // ~((a + b) * c)
    let expr2 = a.or(&b).and(&c).not();
    assert_eq!(format!("{}", expr2), "~((a + b) * c)");
}

// ========== Roundtrip Tests (Display -> Parse -> Display) ==========

#[test]
fn test_roundtrip_simple_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&b);

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "a * b");
    assert_eq!(display, display2);
}

#[test]
fn test_roundtrip_simple_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.or(&b);

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "a + b");
    assert_eq!(display, display2);
}

#[test]
fn test_roundtrip_and_then_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr = a.and(&b).or(&c);

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "a * b + c");
    assert_eq!(display, display2);
}

#[test]
fn test_roundtrip_or_then_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr = a.or(&b).and(&c);

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "(a + b) * c");
    assert_eq!(display, display2);
}

#[test]
fn test_roundtrip_not_of_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&b).not();

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "~(a * b)");
    assert_eq!(display, display2);
}

#[test]
fn test_roundtrip_not_of_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.or(&b).not();

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "~(a + b)");
    assert_eq!(display, display2);
}

#[test]
fn test_roundtrip_xor_like() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&b).or(&a.not().and(&b.not()));

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "a * b + ~a * ~b");
    assert_eq!(display, display2);
}

#[test]
fn test_roundtrip_complex_nested() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");
    let expr = a.or(&b).and(&c.or(&d));

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "(a + b) * (c + d)");
    assert_eq!(display, display2);
}

#[test]
fn test_roundtrip_majority() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr = a.and(&b).or(&b.and(&c)).or(&a.and(&c));

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "a * b + b * c + a * c");
    assert_eq!(display, display2);
}

#[test]
fn test_roundtrip_double_negation() {
    let a = BoolExpr::variable("a");
    let expr = a.not().not();

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "~~a");
    assert_eq!(display, display2);
}

#[test]
fn test_roundtrip_deeply_nested() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");
    let expr = a.or(&b).and(&c).or(&d);

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "(a + b) * c + d");
    assert_eq!(display, display2);
}

#[test]
fn test_roundtrip_with_constants() {
    let a = BoolExpr::variable("a");
    let t = BoolExpr::constant(true);
    let expr = a.and(&t);

    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    assert_eq!(display, "a * 1");
    assert_eq!(display, display2);
}

// ========== Macro Tests (expr! macro) ==========

#[test]
fn test_operator_overloading_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let manual = a.and(&b);
    let with_ops = &a * &b;

    assert_eq!(manual, with_ops);
    assert_eq!(format!("{}", with_ops), "a * b");
}

#[test]
fn test_operator_overloading_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let manual = a.or(&b);
    let with_ops = &a + &b;

    assert_eq!(manual, with_ops);
    assert_eq!(format!("{}", with_ops), "a + b");
}

#[test]
fn test_operator_overloading_not() {
    let a = BoolExpr::variable("a");

    let manual = a.not();

    let a2 = BoolExpr::variable("a");
    let with_ops = !&a2;

    assert_eq!(manual, with_ops);
    assert_eq!(format!("{}", with_ops), "~a");
}

#[test]
fn test_operator_overloading_and_then_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let manual = a.and(&b).or(&c);
    let with_ops = (&a * &b).or(&c);

    assert_eq!(manual, with_ops);
    assert_eq!(format!("{}", with_ops), "a * b + c");
}

#[test]
fn test_operator_overloading_xor_pattern() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let manual = a.and(&b).or(&a.not().and(&b.not()));

    let a2 = BoolExpr::variable("a");
    let b2 = BoolExpr::variable("b");
    let with_ops = &a2 * &b2 + &(!&a2) * &(!&b2);

    assert_eq!(manual, with_ops);
    assert_eq!(format!("{}", with_ops), "a * b + ~a * ~b");
}

#[test]
fn test_operator_overloading_with_parens() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let manual = a.or(&b).and(&c);
    let with_ops = (&a + &b).and(&c);

    assert_eq!(manual, with_ops);
    assert_eq!(format!("{}", with_ops), "(a + b) * c");
}

#[test]
fn test_operator_overloading_not_of_expression() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let manual = a.and(&b).not();
    let with_ops = !(&a * &b);

    assert_eq!(manual, with_ops);
    assert_eq!(format!("{}", with_ops), "~(a * b)");
}

#[test]
fn test_operator_overloading_complex_nested() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");

    let manual = a.or(&b).and(&c.or(&d));
    let with_ops = (&a + &b) * (&c + &d);

    assert_eq!(manual, with_ops);
    assert_eq!(format!("{}", with_ops), "(a + b) * (c + d)");
}

#[test]
fn test_operator_overloading_multiple_not() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let manual = a.not().and(&b.not());

    let a2 = BoolExpr::variable("a");
    let b2 = BoolExpr::variable("b");
    let with_ops = (!&a2) * (!&b2);

    assert_eq!(manual, with_ops);
    assert_eq!(format!("{}", with_ops), "~a * ~b");
}

#[test]
fn test_operator_overloading_three_way_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let manual = a.and(&b).and(&c);
    let with_ops = (&a * &b).and(&c);

    assert_eq!(manual, with_ops);
    assert_eq!(format!("{}", with_ops), "a * b * c");
}

// ========== Combined Roundtrip + Operator Tests ==========

#[test]
fn test_operator_roundtrip_xor() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Build with operators
    let expr_built = (&a) * (&b) + (&(!&a)) * (&(!&b));
    let display = format!("{}", expr_built);

    // Parse it back
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    // Should be stable
    assert_eq!(display, "a * b + ~a * ~b");
    assert_eq!(display, display2);
}

#[test]
fn test_operator_roundtrip_complex() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");

    // Build with operators
    let expr_built = (&a + &b) * (&c + &d);
    let display = format!("{}", expr_built);

    // Parse it back
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    // Should be stable
    assert_eq!(display, "(a + b) * (c + d)");
    assert_eq!(display, display2);
}

#[test]
fn test_parse_display_operator_equivalence() {
    // All three methods should produce equivalent results
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Manual construction
    let manual = a.and(&b).or(&c);

    // Operator construction
    let with_ops = (&a * &b).or(&c);

    // Parse from string
    let from_parse = BoolExpr::parse("a * b + c").unwrap();

    // Macro construction
    let from_macro = expr!(a * b + c);

    // All four should produce the same structure
    assert_eq!(manual, with_ops);
    assert_eq!(manual, from_parse);
    assert_eq!(manual, from_macro);
    assert_eq!(with_ops, from_parse);

    // All should display the same
    let display1 = format!("{}", manual);
    let display2 = format!("{}", with_ops);
    let display3 = format!("{}", from_parse);
    let display4 = format!("{}", from_macro);

    assert_eq!(display1, "a * b + c");
    assert_eq!(display1, display2);
    assert_eq!(display1, display3);
    assert_eq!(display1, display4);
}

// ========== Procedural Macro Tests (expr!) ==========

#[test]
fn test_expr_macro_simple_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let macro_expr = expr!(a * b);
    let manual = a.and(&b);

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a * b");
}

#[test]
fn test_expr_macro_simple_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let macro_expr = expr!(a + b);
    let manual = a.or(&b);

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a + b");
}

#[test]
fn test_expr_macro_simple_not() {
    let a = BoolExpr::variable("a");

    let macro_expr = expr!(!a);
    let manual = a.not();

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "~a");
}

#[test]
fn test_expr_macro_and_then_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let macro_expr = expr!(a * b + c);
    let manual = a.and(&b).or(&c);

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a * b + c");
}

#[test]
fn test_expr_macro_xor_pattern() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let macro_expr = expr!(a * b + !a * !b);
    let manual = a.and(&b).or(&a.not().and(&b.not()));

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a * b + ~a * ~b");
}

#[test]
fn test_expr_macro_with_parens() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let macro_expr = expr!((a + b) * c);
    let manual = a.or(&b).and(&c);

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "(a + b) * c");
}

#[test]
fn test_expr_macro_not_of_expression() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let macro_expr = expr!(!(a * b));
    let manual = a.and(&b).not();

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "~(a * b)");
}

#[test]
fn test_expr_macro_complex_nested() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");

    let macro_expr = expr!((a + b) * (c + d));
    let manual = a.or(&b).and(&c.or(&d));

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "(a + b) * (c + d)");
}

#[test]
fn test_expr_macro_multiple_not() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let macro_expr = expr!(!a * !b);
    let manual = a.not().and(&b.not());

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "~a * ~b");
}

#[test]
fn test_expr_macro_three_way_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let macro_expr = expr!(a * b * c);
    let manual = a.and(&b).and(&c);

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a * b * c");
}

#[test]
fn test_expr_macro_three_way_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let macro_expr = expr!(a + b + c);
    let manual = a.or(&b).or(&c);

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a + b + c");
}

#[test]
fn test_expr_macro_majority_function() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let macro_expr = expr!(a * b + b * c + a * c);
    let manual = a.and(&b).or(&b.and(&c)).or(&a.and(&c));

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a * b + b * c + a * c");
}

#[test]
fn test_expr_macro_double_negation() {
    let a = BoolExpr::variable("a");

    let macro_expr = expr!(!!a);
    let manual = a.not().not();

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "~~a");
}

#[test]
fn test_expr_macro_deeply_nested() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");

    let macro_expr = expr!((a + b) * c + d);
    let manual = a.or(&b).and(&c).or(&d);

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "(a + b) * c + d");
}

#[test]
fn test_expr_macro_equivalence_with_manual() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Macro version
    let macro_expr = expr!(a * b + !a * !b);

    // Manual version
    let manual_expr = a.and(&b).or(&a.not().and(&b.not()));

    // Should be structurally equal
    assert_eq!(macro_expr, manual_expr);
    assert_eq!(format!("{}", macro_expr), format!("{}", manual_expr));
}

#[test]
fn test_expr_macro_roundtrip() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let expr = expr!(a * b + !c);
    let display = format!("{}", expr);

    // Parse it back
    let parsed = BoolExpr::parse(&display).unwrap();
    let display2 = format!("{}", parsed);

    // Should be stable
    assert_eq!(display, display2);
    assert!(expr.equivalent_to(&parsed));
}

#[test]
fn test_expr_macro_with_sub_expressions() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Build sub-expressions
    let sub1 = expr!(a * b);
    let sub2 = expr!(c + !a);

    // Combine them
    let combined = expr!(sub1 + sub2);

    // Should work correctly
    let manual = a.and(&b).or(&c.or(&a.not()));
    assert_eq!(combined, manual);
}

// ========== String Literal Tests (automatic variable creation) ==========

#[test]
fn test_expr_macro_string_simple_and() {
    let macro_expr = expr!("a" * "b");

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let manual = a.and(&b);

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a * b");
}

#[test]
fn test_expr_macro_string_simple_or() {
    let macro_expr = expr!("a" + "b");

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let manual = a.or(&b);

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a + b");
}

#[test]
fn test_expr_macro_string_xor() {
    let macro_expr = expr!("a" * "b" + !"a" * !"b");

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let manual = a.and(&b).or(&a.not().and(&b.not()));

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a * b + ~a * ~b");
}

#[test]
fn test_expr_macro_string_complex() {
    let macro_expr = expr!(("a" + "b") * ("c" + "d"));

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");
    let manual = a.or(&b).and(&c.or(&d));

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "(a + b) * (c + d)");
}

#[test]
fn test_expr_macro_mixed_string_and_var() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Mix existing variables with string literals
    let macro_expr = expr!(a * "c" + b);

    let c = BoolExpr::variable("c");
    let manual = a.and(&c).or(&b);

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a * c + b");
}

#[test]
fn test_expr_macro_string_no_variable_declaration() {
    // Most concise syntax - no variable declarations needed!
    let expr = expr!("x" * "y" + "z");

    assert_eq!(format!("{}", expr), "x * y + z");

    // Verify it works correctly
    let vars = expr.collect_variables();
    assert_eq!(vars.len(), 3);
}

// ========== Semantic Equivalence Tests ==========

#[test]
fn test_commutative_and() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let expr1 = a.and(&b);
    let expr2 = b.and(&a);

    // Structurally different
    assert_ne!(expr1, expr2);
    // But logically equivalent
    assert!(expr1.equivalent_to(&expr2));
}

#[test]
fn test_commutative_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let expr1 = a.or(&b);
    let expr2 = b.or(&a);

    // Structurally different
    assert_ne!(expr1, expr2);
    // But logically equivalent
    assert!(expr1.equivalent_to(&expr2));
}

#[test]
fn test_double_negation() {
    let a = BoolExpr::variable("a");

    let expr1 = a.clone();
    let expr2 = a.not().not();

    // Structurally different
    assert_ne!(expr1, expr2);
    // But logically equivalent
    assert!(expr1.equivalent_to(&expr2));
}

#[test]
fn test_not_equivalent() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let and_expr = a.and(&b);
    let or_expr = a.or(&b);

    // Different operations should not be equivalent
    assert_ne!(and_expr, or_expr);
    assert!(!and_expr.equivalent_to(&or_expr));
}

#[test]
fn test_bdd_caching() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&b);

    // First call computes and caches BDD
    let bdd1 = expr.to_bdd();

    // Second call should return cached BDD (same result)
    let bdd2 = expr.to_bdd();

    // Both should be identical
    assert_eq!(bdd1, bdd2);
    assert_eq!(bdd1.node_count(), bdd2.node_count());

    // Caching means repeated calls are essentially free
    for _ in 0..100 {
        let bdd = expr.to_bdd();
        assert_eq!(bdd, bdd1);
    }
}

#[test]
fn test_bdd_subexpression_caching() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Create a common subexpression
    let ab = a.and(&b);

    // Compute BDD for subexpression (gets cached)
    let ab_bdd = ab.to_bdd();

    // Use subexpression in larger expression using expr!
    let expr = expr!(ab + !ab); // (a*b) + ~(a*b) = always true

    // When expr.to_bdd() is called, it should reuse ab's cached BDD
    let expr_bdd = expr.to_bdd();

    // Verify the result is correct (should be TRUE)
    assert!(expr_bdd.is_true());

    // The subexpression cache was used, making this very efficient
    let ab_bdd2 = ab.to_bdd();
    assert_eq!(ab_bdd2, ab_bdd); // Still cached
}
