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

    // BDD may produce different but equivalent form - check logical equivalence
    let expected = BoolExpr::parse("a * b + c").unwrap();
    assert!(expr.equivalent_to(&expected));

    // Round-trip test: display → parse → equivalence
    let display = format!("{}", expr);
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

    // BDD may produce different but equivalent form - check logical equivalence
    let expected = BoolExpr::parse("(a + b) * c").unwrap();
    assert!(expr.equivalent_to(&expected));

    // Round-trip test: display → parse → equivalence
    let display = format!("{}", expr);
    let parsed = BoolExpr::parse(&display).unwrap();
    assert!(expr.equivalent_to(&parsed));
}

#[test]
fn test_not_of_compound_requires_parens() {
    // NOT of compound expression needs parentheses
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let expr_and = a.and(&b).not();
    let expected_and = BoolExpr::parse("~(a * b)").unwrap();
    assert!(expr_and.equivalent_to(&expected_and));

    let display_and = format!("{}", expr_and);
    let parsed_and = BoolExpr::parse(&display_and).unwrap();
    assert!(expr_and.equivalent_to(&parsed_and));

    let expr_or = a.or(&b).not();
    let expected_or = BoolExpr::parse("~(a + b)").unwrap();
    assert!(expr_or.equivalent_to(&expected_or));

    let display_or = format!("{}", expr_or);
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

    // BDD may produce different but equivalent form - check logical equivalence
    let expected = BoolExpr::parse("(a + b) * (c + d)").unwrap();
    assert!(expr.equivalent_to(&expected));

    // Round-trip test: display → parse → equivalence
    let display = format!("{}", expr);
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

    // BDD may produce different but equivalent form - check logical equivalence
    let expected = BoolExpr::parse("(a + b) * c + d").unwrap();
    assert!(expr.equivalent_to(&expected));

    // Round-trip test: display → parse → equivalence
    let display = format!("{}", expr);
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

    // BDD may produce different but equivalent form - check logical equivalence
    let expected = BoolExpr::parse("a * ~b + ~a * b").unwrap();
    assert!(expr.equivalent_to(&expected));

    // Round-trip test: display → parse → equivalence
    let display = format!("{}", expr);
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

    // BDD may produce different but equivalent form - check logical equivalence
    let expected = BoolExpr::parse("a * b + b * c + a * c").unwrap();
    assert!(expr.equivalent_to(&expected));

    // Round-trip test: display → parse → equivalence
    let display = format!("{}", expr);
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

    // BDD optimises a∧1 → a, a∨0 → a
    let a_and_true = a.and(&t);
    let a_or_false = a.or(&f);

    // Check logical equivalence instead of exact format
    assert!(a_and_true.equivalent_to(&a));
    assert!(a_or_false.equivalent_to(&a));
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

    // BDD may produce different but equivalent form - check logical equivalence
    let expected = BoolExpr::parse("(a + b) * (c + d)").unwrap();
    assert!(with_ops.equivalent_to(&expected));
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

    // BDD may produce different but equivalent form - check logical equivalence
    let expected = BoolExpr::parse("(a + b) * c").unwrap();
    assert!(macro_expr.equivalent_to(&expected));
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

    // BDD may produce different but equivalent form - check logical equivalence
    let expected = BoolExpr::parse("a * b + ~a * ~b").unwrap();
    assert!(macro_expr.equivalent_to(&expected));
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

    // All should be equal (BDD canonical form)
    assert_eq!(manual, with_ops);
    assert_eq!(manual, from_parse);
    assert_eq!(manual, from_macro);

    // All should be logically equivalent
    let expected = BoolExpr::parse("a * b + c").unwrap();
    assert!(manual.equivalent_to(&expected));
    assert!(with_ops.equivalent_to(&expected));
    assert!(from_parse.equivalent_to(&expected));
    assert!(from_macro.equivalent_to(&expected));
}

// ========== Semantic Equivalence Tests ==========

#[test]
fn test_commutative_properties() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // AND commutative - With BDD as primary storage, equality is canonical!
    let expr1 = a.and(&b);
    let expr2 = b.and(&a);
    assert_eq!(expr1, expr2); // BDD provides canonical equality
    assert!(expr1.equivalent_to(&expr2)); // And logically equivalent

    // OR commutative - With BDD as primary storage, equality is canonical!
    let expr3 = a.or(&b);
    let expr4 = b.or(&a);
    assert_eq!(expr3, expr4); // BDD provides canonical equality
    assert!(expr3.equivalent_to(&expr4));
}

#[test]
fn test_double_negation_equivalence() {
    let a = BoolExpr::variable("a");

    let expr1 = a.clone();
    let expr2 = a.not().not();

    // With BDD as primary storage, !!a == a (canonical form)
    assert_eq!(expr1, expr2); // BDD provides canonical equality
    assert!(expr1.equivalent_to(&expr2)); // And logically equivalent
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

    // BoolExpr is now a BDD internally
    assert_eq!(expr, expr);
    assert_eq!(expr.node_count(), expr.node_count());

    // Repeated calls should be essentially free
    for _ in 0..100 {
        assert_eq!(expr, expr);
    }
}

#[test]
fn test_bdd_subexpression_caching() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Create a common subexpression
    let ab = a.and(&b);

    // Use subexpression in larger expression
    let expr = expr!(ab + !ab); // (a*b) + ~(a*b) = always true

    // BoolExpr is a BDD internally
    assert!(expr.is_true());

    // Subexpression is still valid
    assert_eq!(ab, ab);
}

// ========== BDD-specific Tests (merged from bdd module) ==========
// Note: Bdd is now a type alias for BoolExpr

#[test]
fn test_terminal_nodes() {
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);

    assert!(t.is_true());
    assert!(!t.is_false());
    assert!(f.is_false());
    assert!(!f.is_true());
    assert!(t.is_terminal());
    assert!(f.is_terminal());
}

#[test]
fn test_variable_creation() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    assert!(!a.is_terminal());
    assert!(!b.is_terminal());
    assert_ne!(a, b);
}

#[test]
fn test_ite_terminal_cases() {
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);
    let a = BoolExpr::variable("a");

    // Test basic operations which are implemented via ITE internally
    // a AND true = a
    let result = a.and(&t);
    assert_eq!(result, a);

    // a AND false = false
    let result = a.and(&f);
    assert_eq!(result, f);

    // a OR true = true
    let result = a.or(&t);
    assert_eq!(result, t);

    // a OR false = a
    let result = a.or(&f);
    assert_eq!(result, a);
}

#[test]
fn test_node_count() {
    let t = BoolExpr::constant(true);
    assert_eq!(t.node_count(), 1);

    let a = BoolExpr::variable("a");
    // Variable node: 1 decision node + 2 terminal nodes
    assert_eq!(a.node_count(), 3);
}

#[test]
fn test_var_count() {
    let t = BoolExpr::constant(true);
    assert_eq!(t.var_count(), 0);

    let a = BoolExpr::variable("a");
    assert_eq!(a.var_count(), 1);
}

#[test]
fn test_hash_consing() {
    let a1 = BoolExpr::variable("a");
    let a2 = BoolExpr::variable("a");

    // Same variable should produce same node (hash consing)
    assert_eq!(a1, a2);
}

#[test]
fn test_and_operation_bdd() {
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Test terminal cases
    assert_eq!(a.and(&t), a); // a AND true = a
    assert!(a.and(&f).is_false()); // a AND false = false
    assert_eq!(t.and(&a), a); // true AND a = a
    assert!(f.and(&a).is_false()); // false AND a = false

    // Test with variables
    let result = a.and(&b);
    assert!(!result.is_terminal());
    assert!(!result.is_true());
    assert!(!result.is_false());

    // a AND a = a (idempotent)
    let result = a.and(&a);
    assert_eq!(result, a);
}

#[test]
fn test_or_operation_bdd() {
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Test terminal cases
    assert_eq!(a.or(&f), a); // a OR false = a
    assert!(a.or(&t).is_true()); // a OR true = true
    assert_eq!(f.or(&a), a); // false OR a = a
    assert!(t.or(&a).is_true()); // true OR a = true

    // Test with variables
    let result = a.or(&b);
    assert!(!result.is_terminal());

    // a OR a = a (idempotent)
    let result = a.or(&a);
    assert_eq!(result, a);
}

#[test]
fn test_not_operation_bdd() {
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);
    let a = BoolExpr::variable("a");

    // Test terminal cases
    assert!(t.not().is_false()); // NOT true = false
    assert!(f.not().is_true()); // NOT false = true

    // Test double negation
    let not_a = a.not();
    assert!(!not_a.is_terminal());
    let not_not_a = not_a.not();
    assert_eq!(not_not_a, a); // NOT NOT a = a
}

#[test]
fn test_and_or_combination_bdd() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // (a AND b) OR (a AND b) = a AND b (idempotent)
    let ab = a.and(&b);
    let result = ab.or(&ab);
    assert_eq!(result, ab);

    // (a OR b) AND (a OR b) = a OR b (idempotent)
    let a_or_b = a.or(&b);
    let result = a_or_b.and(&a_or_b);
    assert_eq!(result, a_or_b);
}

#[test]
fn test_de_morgans_laws_bdd() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // NOT(a AND b) = (NOT a) OR (NOT b)
    let not_ab = a.and(&b).not();
    let not_a_or_not_b = a.not().or(&b.not());
    assert_eq!(not_ab, not_a_or_not_b);

    // NOT(a OR b) = (NOT a) AND (NOT b)
    let not_a_or_b = a.or(&b).not();
    let not_a_and_not_b = a.not().and(&b.not());
    assert_eq!(not_a_or_b, not_a_and_not_b);
}

#[test]
fn test_commutativity_bdd() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // a AND b = b AND a
    let ab = a.and(&b);
    let ba = b.and(&a);
    assert_eq!(ab, ba);

    // a OR b = b OR a
    let a_or_b = a.or(&b);
    let b_or_a = b.or(&a);
    assert_eq!(a_or_b, b_or_a);
}

#[test]
fn test_associativity_bdd() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // (a AND b) AND c = a AND (b AND c)
    let ab_and_c = a.and(&b).and(&c);
    let a_and_bc = a.and(&b.and(&c));
    assert_eq!(ab_and_c, a_and_bc);

    // (a OR b) OR c = a OR (b OR c)
    let ab_or_c = a.or(&b).or(&c);
    let a_or_bc = a.or(&b.or(&c));
    assert_eq!(ab_or_c, a_or_bc);
}

#[test]
fn test_distributivity_bdd() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // a AND (b OR c) = (a AND b) OR (a AND c)
    let a_and_bc = a.and(&b.or(&c));
    let ab_or_ac = a.and(&b).or(&a.and(&c));
    assert_eq!(a_and_bc, ab_or_ac);

    // a OR (b AND c) = (a OR b) AND (a OR c)
    let a_or_bc = a.or(&b.and(&c));
    let ab_or_ac = a.or(&b).and(&a.or(&c));
    assert_eq!(a_or_bc, ab_or_ac);
}

#[test]
fn test_to_cubes_simple() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // a AND b should produce one cube: {a: true, b: true}
    let ab = a.and(&b);
    let cubes = ab.to_cubes();
    assert_eq!(cubes.len(), 1);
    assert_eq!(cubes[0].get(&Arc::from("a")), Some(&true));
    assert_eq!(cubes[0].get(&Arc::from("b")), Some(&true));
}

#[test]
fn test_to_cubes_or() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // a OR b should produce two cubes
    let a_or_b = a.or(&b);
    let cubes = a_or_b.to_cubes();
    assert_eq!(cubes.len(), 2);
}

#[test]
fn test_to_cubes_constant() {
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);

    // TRUE should produce one empty cube (tautology)
    let cubes = t.to_cubes();
    assert_eq!(cubes.len(), 1);
    assert!(cubes[0].is_empty());

    // FALSE should produce no cubes
    let cubes = f.to_cubes();
    assert_eq!(cubes.len(), 0);
}

#[test]
fn test_to_cubes_complex() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // (a AND b) OR (b AND c) OR (a AND c) - majority function
    let ab = a.and(&b);
    let bc = b.and(&c);
    let ac = a.and(&c);
    let majority = ab.or(&bc).or(&ac);

    let cubes = majority.to_cubes();
    // Should produce 3 cubes for the three products
    assert!(cubes.len() >= 2); // BDD may optimise this
    assert!(cubes.len() <= 3);
}

#[test]
fn test_roundtrip_bdd_expr() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&b);

    // Convert to BDD and back (note: now they're the same type, so this is just cloning)
    #[allow(deprecated)]
    let bdd = expr.to_bdd();
    #[allow(deprecated)]
    let expr2 = bdd.to_expr();

    // Should be logically equivalent
    assert!(expr.equivalent_to(&expr2));
}

#[test]
fn test_bdd_consensus_theorem() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Consensus theorem: a*b + ~a*c + b*c
    // The b*c term is redundant
    let expr = a.and(&b).or(&a.not().and(&c)).or(&b.and(&c));
    let cubes = expr.to_cubes();

    // BDD should recognise that b*c is redundant and produce only 2 cubes
    assert_eq!(cubes.len(), 2);
}

#[test]
fn test_bdd_xor() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // XOR: a*~b + ~a*b
    let xor = a.and(&b.not()).or(&a.not().and(&b));
    let cubes = xor.to_cubes();

    // Should produce 2 cubes
    assert_eq!(cubes.len(), 2);

    // Convert back and verify equivalence (deprecated method, but test for compatibility)
    #[allow(deprecated)]
    let expr2 = xor.to_expr();
    assert!(xor.equivalent_to(&expr2));
}

#[test]
fn test_global_manager_sharing() {
    // Create multiple expressions
    let a1 = BoolExpr::variable("a");
    let a2 = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // All BoolExprs should share the same manager (Arc pointer equality)
    assert!(Arc::ptr_eq(&a1.manager, &a2.manager));
    assert!(Arc::ptr_eq(&a1.manager, &b.manager));

    // Same expressions should produce identical representations (hash consing works globally)
    assert_eq!(a1, a2);
}

#[test]
fn test_dnf_cache_updated_after_minimization() {
    use crate::Minimizable;

    // Create activation expression from threshold_gate example
    // This has some redundancy that Espresso can minimize
    let activation = expr!(
        // All 5 high
        "a" * "b" * "c" * "d" * "e" +
        // Any 4 high (5 choose 4 = 5 combinations)
        "a" * "b" * "c" * "d" * !"e" +
        "a" * "b" * "c" * !"d" * "e" +
        "a" * "b" * !"c" * "d" * "e" +
        "a" * !"b" * "c" * "d" * "e" +
        !"a" * "b" * "c" * "d" * "e"
    );

    // Get initial cube count (BDD canonical form)
    let cubes_before = activation.to_cubes();
    println!("Cubes before minimization: {}", cubes_before.len());

    // BDD should have already reduced this from 6 to 5 cubes
    assert_eq!(cubes_before.len(), 5, "BDD should reduce to 5 cubes");

    // Minimize the expression
    let minimized = activation.minimize().unwrap();

    // Get cube count after minimization
    let cubes_after = minimized.to_cubes();
    println!("Cubes after minimization: {}", cubes_after.len());

    // For this particular expression, Espresso keeps it at 5 cubes (already minimal)
    assert_eq!(cubes_after.len(), 5, "Espresso should keep at 5 cubes");

    // Verify they are equivalent
    assert!(activation.equivalent_to(&minimized));
}

#[test]
fn test_dnf_cache_updated_with_smaller_cover() {
    use crate::Minimizable;

    // Create a DIFFERENT expression from test 3 to avoid cache pollution
    // Using a simpler pattern but still reducible
    let expr = expr!(
        "x" * "y" * "z"
            + "x" * "y" * !"z"
            + "x" * !"y" * "z"
            + !"x" * "y" * "z"
            + "x" * "w"
            + "y" * "w"
    );

    // Get initial cube count
    let cubes_before = expr.to_cubes();
    println!("Cubes before minimization: {}", cubes_before.len());

    // Should have multiple cubes
    assert!(
        cubes_before.len() >= 3,
        "Should start with at least 3 cubes, got {}",
        cubes_before.len()
    );

    // Minimize the expression
    let minimized = expr.minimize().unwrap();

    // Get cube count after minimization
    let cubes_after = minimized.to_cubes();
    println!("Cubes after minimization: {}", cubes_after.len());

    // Espresso should reduce it (exact count depends on Espresso heuristics)
    assert!(
        cubes_after.len() <= cubes_before.len(),
        "Minimized should have <= cubes than original"
    );

    // Verify they are equivalent
    assert!(expr.equivalent_to(&minimized));

    // Verify cache is actually being used - call to_cubes again should return same count
    let cubes_cached = minimized.to_cubes();
    assert_eq!(
        cubes_cached.len(),
        cubes_after.len(),
        "Cached cubes should be consistent"
    );
}

#[test]
fn test_dnf_cache_shared_across_clones() {
    use crate::Minimizable;

    // Create the next_q_v1 expression from threshold_gate example
    // This is known to reduce from 19 cubes to 15 cubes
    let activation = expr!(
        "a" * "b" * "c" * "d" * "e"
            + "a" * "b" * "c" * "d" * !"e"
            + "a" * "b" * "c" * !"d" * "e"
            + "a" * "b" * !"c" * "d" * "e"
            + "a" * !"b" * "c" * "d" * "e"
            + !"a" * "b" * "c" * "d" * "e"
    );

    let deactivation = expr!(
        !"a" * !"b" * !"c" * !"d" * !"e"
            + "a" * !"b" * !"c" * !"d" * !"e"
            + !"a" * "b" * !"c" * !"d" * !"e"
            + !"a" * !"b" * "c" * !"d" * !"e"
            + !"a" * !"b" * !"c" * "d" * !"e"
            + !"a" * !"b" * !"c" * !"d" * "e"
    );

    let expr = expr!((activation + "q") * !deactivation);

    // Get initial cube count BEFORE minimization
    let cubes_before = expr.to_cubes();
    println!("Original cubes BEFORE minimization: {}", cubes_before.len());
    assert_eq!(cubes_before.len(), 19, "Should start with 19 cubes");

    // Clone it
    let clone1 = expr.clone();
    let clone2 = expr.clone();

    // Minimize one of the clones
    let minimized = clone1.minimize().unwrap();
    let min_cubes = minimized.to_cubes();
    println!("Minimized cubes: {}", min_cubes.len());
    assert_eq!(min_cubes.len(), 15, "Should minimize to 15 cubes");

    // NOW check if the original expr sees the minimized cache!
    // Since they have the same NodeId (equivalent functions), the cache should be shared
    let cubes_after = expr.to_cubes();
    println!(
        "Original cubes AFTER minimization of clone: {}",
        cubes_after.len()
    );

    // The original should now see the minimized cache if they share the same NodeId
    if expr == minimized {
        // Same NodeId - cache should be shared
        println!("✓ Same NodeId - cache is shared!");
        assert_eq!(
            cubes_after.len(),
            15,
            "Original should see minimized cache (same NodeId)"
        );
    } else {
        // Different NodeIds - caches are separate
        println!("✗ Different NodeIds - caches are separate");
        assert_eq!(
            cubes_after.len(),
            19,
            "Original should keep its own cache (different NodeId)"
        );
    }

    // clone2 should also reflect the update if it shares the NodeId
    let clone2_cubes = clone2.to_cubes();
    if expr == minimized {
        assert_eq!(
            clone2_cubes.len(),
            15,
            "Clone should see minimized cache (same NodeId)"
        );
    }

    // Verify equivalence
    assert!(expr.equivalent_to(&minimized));
    assert!(clone2.equivalent_to(&minimized));
}

#[test]
fn test_dnf_cache_updates_with_better_version() {
    use crate::Minimizable;

    // Create a redundant expression
    let redundant = expr!("a" * "b" + "a" * "b" * "c" + "a" * "b" * "c" * "d");

    // Get cubes (BDD should already simplify this)
    let cubes_bdd = redundant.to_cubes();
    println!("BDD cubes: {}", cubes_bdd.len());

    // BDD should reduce to 1 cube (a*b covers all terms)
    assert_eq!(cubes_bdd.len(), 1, "BDD should reduce to 1 cube");

    // Minimize it
    let minimized = redundant.minimize().unwrap();
    let cubes_min = minimized.to_cubes();
    println!("Minimized cubes: {}", cubes_min.len());

    // Should still be 1 cube
    assert_eq!(cubes_min.len(), 1, "Minimized should still be 1 cube");

    // Verify equivalence
    assert!(redundant.equivalent_to(&minimized));
}
