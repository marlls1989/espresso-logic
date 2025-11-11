//! Tests for the bdd module

use super::*;

#[test]
fn test_terminal_nodes() {
    let t = Bdd::constant(true);
    let f = Bdd::constant(false);

    assert!(t.is_true());
    assert!(!t.is_false());
    assert!(f.is_false());
    assert!(!f.is_true());
    assert!(t.is_terminal());
    assert!(f.is_terminal());
}

#[test]
fn test_variable_creation() {
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");

    assert!(!a.is_terminal());
    assert!(!b.is_terminal());
    assert_ne!(a, b);
}

#[test]
fn test_ite_terminal_cases() {
    let t = Bdd::constant(true);
    let f = Bdd::constant(false);
    let a = Bdd::variable("a");

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
    let t = Bdd::constant(true);
    assert_eq!(t.node_count(), 1);

    let a = Bdd::variable("a");
    // Variable node: 1 decision node + 2 terminal nodes
    assert_eq!(a.node_count(), 3);
}

#[test]
fn test_var_count() {
    let t = Bdd::constant(true);
    assert_eq!(t.var_count(), 0);

    let a = Bdd::variable("a");
    assert_eq!(a.var_count(), 1);
}

#[test]
fn test_hash_consing() {
    let a1 = Bdd::variable("a");
    let a2 = Bdd::variable("a");

    // Same variable should produce same node (hash consing)
    assert_eq!(a1, a2);
}

#[test]
fn test_and_operation() {
    let t = Bdd::constant(true);
    let f = Bdd::constant(false);
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");

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
fn test_or_operation() {
    let t = Bdd::constant(true);
    let f = Bdd::constant(false);
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");

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
fn test_not_operation() {
    let t = Bdd::constant(true);
    let f = Bdd::constant(false);
    let a = Bdd::variable("a");

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
fn test_and_or_combination() {
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");

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
fn test_de_morgans_laws() {
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");

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
fn test_commutativity() {
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");

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
fn test_associativity() {
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");
    let c = Bdd::variable("c");

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
fn test_distributivity() {
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");
    let c = Bdd::variable("c");

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
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");

    // a AND b should produce one cube: {a: true, b: true}
    let ab = a.and(&b);
    let cubes = ab.to_cubes();
    assert_eq!(cubes.len(), 1);
    assert_eq!(cubes[0].get(&Arc::from("a")), Some(&true));
    assert_eq!(cubes[0].get(&Arc::from("b")), Some(&true));
}

#[test]
fn test_to_cubes_or() {
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");

    // a OR b should produce two cubes
    let a_or_b = a.or(&b);
    let cubes = a_or_b.to_cubes();
    assert_eq!(cubes.len(), 2);
}

#[test]
fn test_to_cubes_constant() {
    let t = Bdd::constant(true);
    let f = Bdd::constant(false);

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
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");
    let c = Bdd::variable("c");

    // (a AND b) OR (b AND c) OR (a AND c) - majority function
    let ab = a.and(&b);
    let bc = b.and(&c);
    let ac = a.and(&c);
    let majority = ab.or(&bc).or(&ac);

    let cubes = majority.to_cubes();
    // Should produce 3 cubes for the three products
    assert!(cubes.len() >= 2); // BDD may optimize this
    assert!(cubes.len() <= 3);
}

#[test]
fn test_roundtrip_bdd_expr() {
    use crate::expression::BoolExpr;

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&b);

    // Convert to BDD and back
    let bdd = expr.to_bdd();
    let expr2 = bdd.to_expr();

    // Should be logically equivalent
    assert!(expr.equivalent_to(&expr2));
}

#[test]
fn test_bdd_from_expr() {
    use crate::expression::BoolExpr;

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&b);

    // Test both conversion methods
    let bdd1 = expr.to_bdd();
    let bdd2 = Bdd::from_expr(&expr);

    // Both should produce equivalent BDDs
    assert_eq!(bdd1.node_count(), bdd2.node_count());
}

#[test]
fn test_bdd_consensus_theorem() {
    use crate::expression::BoolExpr;

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Consensus theorem: a*b + ~a*c + b*c
    // The b*c term is redundant
    let expr = a.and(&b).or(&a.not().and(&c)).or(&b.and(&c));
    let bdd = expr.to_bdd();
    let cubes = bdd.to_cubes();

    // BDD should recognize that b*c is redundant and produce only 2 cubes
    assert_eq!(cubes.len(), 2);
}

#[test]
fn test_bdd_xor() {
    use crate::expression::BoolExpr;

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // XOR: a*~b + ~a*b
    let xor = a.and(&b.not()).or(&a.not().and(&b));
    let bdd = xor.to_bdd();
    let cubes = bdd.to_cubes();

    // Should produce 2 cubes
    assert_eq!(cubes.len(), 2);

    // Convert back and verify equivalence
    let expr2 = bdd.to_expr();
    assert!(xor.equivalent_to(&expr2));
}

#[test]
fn test_global_manager_sharing() {
    use crate::expression::BoolExpr;

    // Create multiple BDDs
    let a1 = BoolExpr::variable("a");
    let a2 = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let bdd1 = a1.to_bdd();
    let bdd2 = a2.to_bdd();
    let bdd3 = b.to_bdd();

    // All BDDs should share the same manager (Arc pointer equality)
    assert!(Arc::ptr_eq(&bdd1.manager, &bdd2.manager));
    assert!(Arc::ptr_eq(&bdd1.manager, &bdd3.manager));

    // Same expressions should produce identical BDDs (hash consing works globally)
    assert_eq!(bdd1, bdd2);
}
