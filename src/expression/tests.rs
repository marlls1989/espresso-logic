//! Unit tests for the owned, syntactic [`BoolExpr`].
//!
//! These cover construction, structural equality/hashing, the bitwise operators, evaluation, parsing,
//! display round-tripping, and the bridge to the canonical BDD layer. Semantic equivalence is checked
//! either by evaluating over a full truth table (because `BoolExpr` equality is *syntactic*) or via a
//! `BddContext` (whose `equivalent_to` is O(1) canonical).

use super::BoolExpr;
use crate::Symbol;
use std::collections::{HashMap, HashSet};

/// All `2^n` assignments over `vars`, as `Symbol -> bool` maps.
fn all_assignments(vars: &[&str]) -> Vec<HashMap<Symbol, bool>> {
    (0..(1u32 << vars.len()))
        .map(|mask| {
            vars.iter()
                .enumerate()
                .map(|(i, v)| (Symbol::from(*v), (mask >> i) & 1 == 1))
                .collect()
        })
        .collect()
}

/// Whether two expressions evaluate identically over every assignment to `vars`.
fn evaluates_same(a: &BoolExpr, b: &BoolExpr, vars: &[&str]) -> bool {
    all_assignments(vars)
        .iter()
        .all(|m| a.evaluate(m) == b.evaluate(m))
}

// ---- Construction and structural equality ---------------------------------------------------------

#[test]
fn var_and_constant_construct() {
    let a = BoolExpr::var("a");
    assert_eq!(a, BoolExpr::variable("a")); // `variable` is an alias of `var`
    assert_ne!(BoolExpr::var("a"), BoolExpr::var("b"));
    assert_ne!(BoolExpr::constant(true), BoolExpr::constant(false));
    assert_ne!(BoolExpr::var("a"), BoolExpr::constant(true));
}

#[test]
fn var_accepts_any_as_ref_str() {
    // `var` takes any `AsRef<str>`, not only `&str`.
    let from_string = BoolExpr::var(String::from("x"));
    let from_str = BoolExpr::var("x");
    assert_eq!(from_string, from_str);
}

#[test]
fn clone_is_structural_equal() {
    let f = BoolExpr::var("a") & BoolExpr::var("b");
    assert_eq!(f.clone(), f);
}

#[test]
fn equality_is_syntactic() {
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");

    // Same structure ⇒ equal.
    assert_eq!(&a & &b, &a & &b);
    // a & b is NOT b & a syntactically (commutativity is semantic, not syntactic).
    assert_ne!(&a & &b, &b & &a);
    // Different operator ⇒ not equal.
    assert_ne!(&a & &b, &a | &b);
    // Negation differs.
    assert_ne!(a.clone(), !a.clone());
}

#[test]
fn hash_agrees_with_eq() {
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let mut set: HashSet<BoolExpr> = HashSet::new();
    set.insert(&a & &b);
    assert!(set.contains(&(&a & &b)));
    assert!(!set.contains(&(&b & &a)));
    assert!(!set.contains(&(&a | &b)));
}

// ---- Operators ------------------------------------------------------------------------------------

#[test]
fn operator_ref_and_value_forms_agree() {
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let by_value = a.clone() & b.clone();
    assert_eq!(&a & &b, by_value);
    assert_eq!(a.clone() & &b, by_value);
    assert_eq!(&a & b.clone(), by_value);
    // NOT, by value and by reference.
    assert_eq!(!a.clone(), !&a);
}

#[test]
fn operators_match_named_methods() {
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    assert_eq!(&a & &b, a.and(&b));
    assert_eq!(&a | &b, a.or(&b));
    assert_eq!(&a ^ &b, a.xor(&b));
    assert_eq!(!&a, a.not());
}

// ---- Evaluation -----------------------------------------------------------------------------------

#[test]
fn evaluate_truth_tables() {
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");

    let and = &a & &b;
    let or = &a | &b;
    let xor = &a ^ &b;
    let not_a = !&a;
    let nested = (&a & &b) | !&a; // a & b | !a

    for m in all_assignments(&["a", "b"]) {
        let av = m[&Symbol::from("a")];
        let bv = m[&Symbol::from("b")];
        assert_eq!(and.evaluate(&m), av && bv);
        assert_eq!(or.evaluate(&m), av || bv);
        assert_eq!(xor.evaluate(&m), av ^ bv);
        assert_eq!(not_a.evaluate(&m), !av);
        assert_eq!(nested.evaluate(&m), (av && bv) || !av);
    }
}

#[test]
fn evaluate_missing_var_is_false() {
    let expr = BoolExpr::var("a") & BoolExpr::var("b");
    let only_a: HashMap<&str, bool> = HashMap::from([("a", true)]);
    assert!(!expr.evaluate(&only_a));
    let empty: HashMap<&str, bool> = HashMap::new();
    assert!(!BoolExpr::var("a").evaluate(&empty));
    assert!(BoolExpr::constant(true).evaluate(&empty));
}

// ---- Parsing --------------------------------------------------------------------------------------

#[test]
fn parse_round_trips_semantically() {
    let parsed = BoolExpr::parse("a & b | c").unwrap();
    let built = (BoolExpr::var("a") & BoolExpr::var("b")) | BoolExpr::var("c");
    assert!(evaluates_same(&parsed, &built, &["a", "b", "c"]));
}

#[test]
fn parse_star_plus_equals_amp_pipe() {
    // The grammar accepts both spellings; they lower to the same canonical token set, so the parsed
    // expressions are structurally identical.
    assert_eq!(
        BoolExpr::parse("a * b + c").unwrap(),
        BoolExpr::parse("a & b | c").unwrap()
    );
    assert_eq!(
        BoolExpr::parse("~a").unwrap(),
        BoolExpr::parse("!a").unwrap()
    );
}

#[test]
fn parse_respects_precedence() {
    // a | b & c parses as a | (b & c); not (a | b) & c.
    let parsed = BoolExpr::parse("a | b & c").unwrap();
    let expected = BoolExpr::var("a") | (BoolExpr::var("b") & BoolExpr::var("c"));
    assert!(evaluates_same(&parsed, &expected, &["a", "b", "c"]));
    // Differs semantically from the other grouping.
    let other = (BoolExpr::var("a") | BoolExpr::var("b")) & BoolExpr::var("c");
    assert!(!evaluates_same(&parsed, &other, &["a", "b", "c"]));
}

#[test]
fn from_str_works() {
    let expr: BoolExpr = "a ^ b".parse().unwrap();
    assert!(evaluates_same(
        &expr,
        &(BoolExpr::var("a") ^ BoolExpr::var("b")),
        &["a", "b"]
    ));
}

// ---- Display --------------------------------------------------------------------------------------

#[test]
fn display_uses_canonical_spellings_minimal_parens() {
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let c = BoolExpr::var("c");

    assert_eq!((&a & &b | &c).to_string(), "a & b | c");
    assert_eq!(((&a | &b) & &c).to_string(), "(a | b) & c");
    assert_eq!((!(&a & &b)).to_string(), "!(a & b)");
    assert_eq!((!&a & &b).to_string(), "!a & b");
    assert_eq!((&a ^ &b).to_string(), "a ^ b");
    assert_eq!(BoolExpr::constant(true).to_string(), "1");
    assert_eq!(BoolExpr::constant(false).to_string(), "0");
}

#[test]
fn display_reparses_to_equivalent() {
    // Display is syntactic, but parsing its output must recover an equivalent function.
    let exprs = [
        BoolExpr::var("a") & BoolExpr::var("b") | !BoolExpr::var("c"),
        (BoolExpr::var("a") | BoolExpr::var("b")) & BoolExpr::var("c"),
        BoolExpr::var("a") ^ BoolExpr::var("b") ^ BoolExpr::var("c"),
        !(BoolExpr::var("a") & (BoolExpr::var("b") | BoolExpr::var("c"))),
    ];
    for expr in &exprs {
        let reparsed = BoolExpr::parse(expr.to_string()).unwrap();
        assert!(
            evaluates_same(expr, &reparsed, &["a", "b", "c"]),
            "display `{expr}` did not reparse equivalently"
        );
    }
}

// ---- Syntactic variables --------------------------------------------------------------------------

#[test]
fn variables_are_syntactic() {
    let f = BoolExpr::var("b") & (BoolExpr::var("a") | !BoolExpr::var("a"));
    let vars: Vec<String> = f.variables().iter().map(|s| s.to_string()).collect();
    // Syntactic scan reports every occurring variable (a appears even though a | !a is a tautology).
    assert_eq!(vars, vec!["a".to_string(), "b".to_string()]);
}

// ---- Bridge to the BDD layer ----------------------------------------------------------------------

#[test]
fn bdd_build_matches_parse() {
    let ctx = crate::bdd_context!();
    let f = BoolExpr::var("a") & BoolExpr::var("b") | BoolExpr::var("c");
    let built = ctx.build(&f);
    let parsed = ctx.parse("a & b | c").unwrap();
    assert!(built.equivalent_to(parsed));
}

#[test]
fn bdd_build_canonicalises_commutativity() {
    let ctx = crate::bdd_context!();
    // a & b and b & a are different BoolExpr values but the same Bdd.
    let ab = ctx.build(&(BoolExpr::var("a") & BoolExpr::var("b")));
    let ba = ctx.build(&(BoolExpr::var("b") & BoolExpr::var("a")));
    assert!(ab.equivalent_to(ba));
}

#[test]
fn to_expr_round_trips_semantically() {
    let ctx = crate::bdd_context!();
    let f = (BoolExpr::var("a") & BoolExpr::var("b")) | (BoolExpr::var("a") & BoolExpr::var("c"));
    let bdd = ctx.build(&f);
    let recovered = bdd.to_expr();
    // to_expr is factored/syntactic, so compare semantically over all assignments.
    assert!(evaluates_same(&f, &recovered, &["a", "b", "c"]));
}

#[test]
fn sync_context_build_works() {
    let ctx = crate::sync_bdd_context!();
    let f = BoolExpr::var("a") ^ BoolExpr::var("b");
    assert!(ctx.build(&f).equivalent_to(ctx.parse("a ^ b").unwrap()));
}
