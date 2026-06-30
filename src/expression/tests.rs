//! Unit tests for the owned, syntactic [`BoolExpr`].
//!
//! These cover construction, structural equality/hashing, the bitwise operators, evaluation, parsing,
//! display round-tripping, and the bridge to the canonical BDD layer. Semantic equivalence is checked
//! either by evaluating over a full truth table (because `BoolExpr` equality is *syntactic*) or via a
//! `BddBuilder` (whose `equivalent_to` is O(1) canonical).

use super::BoolExpr;
use std::collections::HashSet;

/// Whether two expressions denote the same Boolean function. `BoolExpr` equality is *syntactic*, so
/// semantic equivalence is checked through the canonical BDD layer: both are built into one builder and
/// compared by `equivalent_to` (an O(1) canonical-root comparison).
fn equivalent(a: &BoolExpr, b: &BoolExpr) -> bool {
    let builder = crate::bdd_builder!();
    builder.build(a).equivalent_to(builder.build(b))
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

// ---- Operator structure ---------------------------------------------------------------------------

#[test]
fn operators_build_the_expected_functions() {
    // The bitwise operators construct the corresponding Boolean functions; checked canonically by
    // building into a builder (evaluation/equivalence are BDD-layer concerns — see `bdd::tests`).
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let builder = crate::bdd_builder!();
    let (ba, bb) = (builder.var("a"), builder.var("b"));

    assert!(builder.build(&(&a & &b)).equivalent_to(ba & bb));
    assert!(builder.build(&(&a | &b)).equivalent_to(ba | bb));
    assert!(builder.build(&(&a ^ &b)).equivalent_to(ba ^ bb));
    assert!(builder.build(&!&a).equivalent_to(!ba));
}

// ---- Folding over tokens --------------------------------------------------------------------------

#[test]
fn fold_walks_the_token_structure_including_xor() {
    use super::ExprNode;

    // f = (a ^ b) | !c — counts each operator node; the fold must visit a real Xor node.
    let expr = (BoolExpr::var("a") ^ BoolExpr::var("b")) | !BoolExpr::var("c");

    let (ops, xors) = expr.fold(|node: ExprNode<(usize, usize)>| match node {
        ExprNode::Variable(_) | ExprNode::Constant(_) => (0, 0),
        ExprNode::Not((o, x)) => (o + 1, x),
        ExprNode::And((lo, lx), (ro, rx)) | ExprNode::Or((lo, lx), (ro, rx)) => {
            (lo + ro + 1, lx + rx)
        }
        ExprNode::Xor((lo, lx), (ro, rx)) => (lo + ro + 1, lx + rx + 1),
    });
    assert_eq!(ops, 3); // ^, |, !
    assert_eq!(xors, 1); // the fold saw the XOR token, not an And/Or/Not expansion
}

// ---- Parsing --------------------------------------------------------------------------------------

#[test]
fn parse_round_trips_semantically() {
    let parsed = BoolExpr::parse("a & b | c").unwrap();
    let built = (BoolExpr::var("a") & BoolExpr::var("b")) | BoolExpr::var("c");
    assert!(equivalent(&parsed, &built));
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
    assert!(equivalent(&parsed, &expected));
    // Differs semantically from the other grouping.
    let other = (BoolExpr::var("a") | BoolExpr::var("b")) & BoolExpr::var("c");
    assert!(!equivalent(&parsed, &other));
}

#[test]
fn from_str_works() {
    let expr: BoolExpr = "a ^ b".parse().unwrap();
    assert!(equivalent(&expr, &(BoolExpr::var("a") ^ BoolExpr::var("b"))));
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
            equivalent(expr, &reparsed),
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
    let builder = crate::bdd_builder!();
    let f = BoolExpr::var("a") & BoolExpr::var("b") | BoolExpr::var("c");
    let built = builder.build(&f);
    let parsed = builder.parse("a & b | c").unwrap();
    assert!(built.equivalent_to(parsed));
}

#[test]
fn bdd_build_canonicalises_commutativity() {
    let builder = crate::bdd_builder!();
    // a & b and b & a are different BoolExpr values but the same Bdd.
    let ab = builder.build(&(BoolExpr::var("a") & BoolExpr::var("b")));
    let ba = builder.build(&(BoolExpr::var("b") & BoolExpr::var("a")));
    assert!(ab.equivalent_to(ba));
}

#[test]
fn to_expr_round_trips_semantically() {
    let builder = crate::bdd_builder!();
    let f = (BoolExpr::var("a") & BoolExpr::var("b")) | (BoolExpr::var("a") & BoolExpr::var("c"));
    let bdd = builder.build(&f);
    let recovered = bdd.to_expr();
    // to_expr is factored/syntactic, so compare semantically through the BDD layer.
    assert!(equivalent(&f, &recovered));
}

#[test]
fn sync_context_build_works() {
    let builder = crate::sync_bdd_builder!();
    let f = BoolExpr::var("a") ^ BoolExpr::var("b");
    assert!(builder.build(&f).equivalent_to(builder.parse("a ^ b").unwrap()));
}
