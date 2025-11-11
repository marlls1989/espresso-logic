//! Tests for the expression module

use super::*;
use crate::expr;

#[test]
fn test_collect_variables() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let expr = a.and(&b).or(&c);
    let vars = expr.collect_variables();

    assert_eq!(vars.len(), 3);
    let var_names: Vec<String> = vars.iter().map(|s| s.to_string()).collect();
    assert_eq!(var_names, vec!["a", "b", "c"]); // Should be alphabetical
}

// ========== Display and Parsing Round-trip Tests ==========
// These tests verify correct expression formatting with focus on:
// - Operator precedence
// - Correct parenthesis placement
// - Complex nesting
// Uses round-trip validation: display → parse → equivalency check

#[test]
fn test_precedence_and_over_or() {
    // AND has higher precedence than OR
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr = a.and(&b).or(&c);

    let display = format!("{}", expr);
    assert_eq!(display, "a * b + c"); // No parens needed

    let parsed = BoolExpr::parse(&display).unwrap();
    assert!(expr.equivalent_to(&parsed));
}

#[test]
fn test_precedence_or_in_and_needs_parens() {
    // OR has lower precedence, needs parentheses when inside AND
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr = a.or(&b).and(&c);

    let display = format!("{}", expr);
    assert_eq!(display, "(a + b) * c"); // Parens required

    let parsed = BoolExpr::parse(&display).unwrap();
    assert!(expr.equivalent_to(&parsed));
}

#[test]
fn test_not_of_compound_requires_parens() {
    // NOT of compound expression needs parentheses
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let expr_and = a.and(&b).not();
    let display_and = format!("{}", expr_and);
    assert_eq!(display_and, "~(a * b)");
    let parsed_and = BoolExpr::parse(&display_and).unwrap();
    assert!(expr_and.equivalent_to(&parsed_and));

    let expr_or = a.or(&b).not();
    let display_or = format!("{}", expr_or);
    assert_eq!(display_or, "~(a + b)");
    let parsed_or = BoolExpr::parse(&display_or).unwrap();
    assert!(expr_or.equivalent_to(&parsed_or));
}

#[test]
fn test_complex_nested_parentheses() {
    // Complex nested: (a + b) * (c + d)
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");
    let expr = a.or(&b).and(&c.or(&d));

    let display = format!("{}", expr);
    assert_eq!(display, "(a + b) * (c + d)");

    let parsed = BoolExpr::parse(&display).unwrap();
    assert!(expr.equivalent_to(&parsed));
}

#[test]
fn test_deeply_nested_expressions() {
    // ((a + b) * c) + d - minimal parens
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");
    let expr = a.or(&b).and(&c).or(&d);

    let display = format!("{}", expr);
    assert_eq!(display, "(a + b) * c + d");

    let parsed = BoolExpr::parse(&display).unwrap();
    assert!(expr.equivalent_to(&parsed));
}

#[test]
fn test_not_precedence() {
    // NOT has highest precedence, no extra parens needed for ~a * b
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.not().and(&b);

    let display = format!("{}", expr);
    assert_eq!(display, "~a * b");

    let parsed = BoolExpr::parse(&display).unwrap();
    assert!(expr.equivalent_to(&parsed));
}

#[test]
fn test_xor_pattern_formatting() {
    // XOR-like pattern: a*~b + ~a*b
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&(!&b)).or(&(!&a).and(&b));

    let display = format!("{}", expr);
    assert_eq!(display, "a * ~b + ~a * b");

    let parsed = BoolExpr::parse(&display).unwrap();
    assert!(expr.equivalent_to(&parsed));
}

#[test]
fn test_majority_function_formatting() {
    // Majority: a*b + b*c + a*c - clean formatting
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr = a.and(&b).or(&b.and(&c)).or(&a.and(&c));

    let display = format!("{}", expr);
    assert_eq!(display, "a * b + b * c + a * c");

    let parsed = BoolExpr::parse(&display).unwrap();
    assert!(expr.equivalent_to(&parsed));
}

#[test]
fn test_constants_formatting() {
    let a = BoolExpr::variable("a");
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);

    assert_eq!(format!("{}", t), "1");
    assert_eq!(format!("{}", f), "0");
    assert_eq!(format!("{}", a.and(&t)), "a * 1");
    assert_eq!(format!("{}", a.or(&f)), "a + 0");
}

// ========== Operator Overloading Tests ==========

#[test]
fn test_operator_overloading_basic() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Test basic operators
    let manual_and = a.and(&b);
    let with_ops_and = &a * &b;
    assert_eq!(manual_and, with_ops_and);

    let manual_or = a.or(&b);
    let with_ops_or = &a + &b;
    assert_eq!(manual_or, with_ops_or);

    let manual_not = a.not();
    let with_ops_not = !&a;
    assert_eq!(manual_not, with_ops_not);
}

#[test]
fn test_operator_overloading_complex() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");

    // Complex: (a + b) * (c + d)
    let manual = a.or(&b).and(&c.or(&d));
    let with_ops = (&a + &b) * (&c + &d);
    assert_eq!(manual, with_ops);
    assert_eq!(format!("{}", with_ops), "(a + b) * (c + d)");
}

#[test]
fn test_operator_overloading_xor_pattern() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let manual = a.and(&b).or(&a.not().and(&b.not()));
    let with_ops = &a * &b + &(!&a) * &(!&b);
    assert_eq!(manual, with_ops);
}

// ========== Procedural Macro Tests (expr!) ==========

#[test]
fn test_expr_macro_basic_operators() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    assert_eq!(expr!(a * b), a.and(&b));
    assert_eq!(expr!(a + b), a.or(&b));
    assert_eq!(expr!(!a), a.not());
}

#[test]
fn test_expr_macro_precedence() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // NOT > AND > OR
    let macro_expr = expr!(~a * b + c);
    let manual = a.not().and(&b).or(&c);
    assert_eq!(macro_expr, manual);
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
fn test_expr_macro_complex_expression() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");

    let macro_expr = expr!((a + b) * (c + d));
    let manual = a.or(&b).and(&c.or(&d));
    assert_eq!(macro_expr, manual);
}

#[test]
fn test_expr_macro_with_sub_expressions() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Build sub-expressions and compose them
    let sub1 = expr!(a * b);
    let sub2 = expr!(c + !a);
    let combined = expr!(sub1 + sub2);

    let manual = a.and(&b).or(&c.or(&a.not()));
    assert!(combined.equivalent_to(&manual));
}

// ========== String Literal Tests ==========

#[test]
fn test_expr_macro_string_literals() {
    // String literals create variables automatically
    let macro_expr = expr!("a" * "b" + !"a" * !"b");

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let manual = a.and(&b).or(&a.not().and(&b.not()));

    assert_eq!(macro_expr, manual);
    assert_eq!(format!("{}", macro_expr), "a * b + ~a * ~b");
}

#[test]
fn test_expr_macro_mixed_variables_and_strings() {
    // Can mix existing variables with string notation
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let macro_expr = expr!(a * "c" + b);

    let c = BoolExpr::variable("c");
    let manual = a.and(&c).or(&b);
    assert_eq!(macro_expr, manual);
}

// ========== Parser and Macro Feature Parity ==========

#[test]
fn test_parse_display_operator_macro_equivalence() {
    // All construction methods should produce equivalent results
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    let manual = a.and(&b).or(&c);
    let with_ops = (&a * &b).or(&c);
    let from_parse = BoolExpr::parse("a * b + c").unwrap();
    let from_macro = expr!(a * b + c);

    // All should be structurally equal
    assert_eq!(manual, with_ops);
    assert_eq!(manual, from_parse);
    assert_eq!(manual, from_macro);

    // All should display the same
    assert_eq!(format!("{}", manual), "a * b + c");
    assert_eq!(format!("{}", with_ops), "a * b + c");
    assert_eq!(format!("{}", from_parse), "a * b + c");
    assert_eq!(format!("{}", from_macro), "a * b + c");
}

// ========== Semantic Equivalence Tests ==========

#[test]
fn test_commutative_properties() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // AND commutative
    let expr1 = a.and(&b);
    let expr2 = b.and(&a);
    assert_ne!(expr1, expr2); // Structurally different
    assert!(expr1.equivalent_to(&expr2)); // But logically equivalent

    // OR commutative
    let expr3 = a.or(&b);
    let expr4 = b.or(&a);
    assert_ne!(expr3, expr4);
    assert!(expr3.equivalent_to(&expr4));
}

#[test]
fn test_double_negation_equivalence() {
    let a = BoolExpr::variable("a");

    let expr1 = a.clone();
    let expr2 = a.not().not();

    assert_ne!(expr1, expr2); // Structurally different
    assert!(expr1.equivalent_to(&expr2)); // But logically equivalent
}

#[test]
fn test_not_equivalent_expressions() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let and_expr = a.and(&b);
    let or_expr = a.or(&b);

    assert_ne!(and_expr, or_expr);
    assert!(!and_expr.equivalent_to(&or_expr));
}

// ========== BDD Caching Tests ==========

#[test]
fn test_bdd_caching() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&b);

    // First call computes and caches BDD
    let bdd1 = expr.to_bdd();
    // Second call returns cached BDD
    let bdd2 = expr.to_bdd();

    assert_eq!(bdd1, bdd2);
    assert_eq!(bdd1.node_count(), bdd2.node_count());

    // Repeated calls should be essentially free
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
    let ab_bdd = ab.to_bdd(); // Gets cached

    // Use subexpression in larger expression
    let expr = expr!(ab + !ab); // (a*b) + ~(a*b) = always true

    // Should reuse cached BDD
    let expr_bdd = expr.to_bdd();
    assert!(expr_bdd.is_true());

    // Subexpression cache still works
    let ab_bdd2 = ab.to_bdd();
    assert_eq!(ab_bdd2, ab_bdd);
}
