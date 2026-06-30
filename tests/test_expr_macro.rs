//! Behaviour of the `expr!` macro: operand conventions, operator precedence, spelling equivalence, and
//! hygiene.

use espresso_logic::{expr, BoolExpr};

#[test]
fn or_binds_looser_than_xor() {
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let c = BoolExpr::var("c");
    assert_eq!(expr!(a + b ^ c), a.clone() | (b.clone() ^ c.clone()));
}

#[test]
fn xor_binds_looser_than_and() {
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let c = BoolExpr::var("c");
    assert_eq!(expr!(a ^ b * c), a.clone() ^ (b.clone() & c.clone()));
}

#[test]
fn both_operator_spellings_agree() {
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    assert_eq!(expr!(a * b), expr!(a & b));
    assert_eq!(expr!(a + b), expr!(a | b));
    assert_eq!(expr!(~a), expr!(!a));
}

#[test]
fn string_literals_are_fresh_variables() {
    assert_eq!(expr!("a" & "b"), BoolExpr::var("a") & BoolExpr::var("b"));
}

#[test]
fn identifiers_graft_existing_expressions() {
    let sub = BoolExpr::parse("a & b").unwrap();
    assert_eq!(expr!(sub | "c"), sub.clone() | BoolExpr::var("c"));
}

#[test]
fn integer_literals_are_constants() {
    assert_eq!(expr!(0), BoolExpr::constant(false));
    assert_eq!(expr!(1), BoolExpr::constant(true));
    let a = BoolExpr::var("a");
    assert_eq!(expr!(a & 1), a.clone() & BoolExpr::constant(true));
}

#[test]
fn parentheses_override_precedence() {
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let c = BoolExpr::var("c");
    assert_eq!(expr!((a + b) * c), (a.clone() | b.clone()) & c.clone());
}

#[test]
fn hygiene_does_not_capture_a_user_ident_named_like_the_builder() {
    // A user binding spelled exactly like the macro's internal builder ident must still resolve to the
    // user's value (mixed-site hygiene keeps the two distinct).
    let __expr_builder = BoolExpr::var("z");
    assert_eq!(expr!(__expr_builder), BoolExpr::var("z"));
}

#[test]
fn macro_and_operator_forms_are_equivalent_as_bdds() {
    let a = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let macro_form = expr!(a * !b + !a * b);
    let operator_form = (a.clone() & !b.clone()) | (!a.clone() & b.clone());

    let builder = espresso_logic::bdd_builder!();
    assert!(builder
        .build(&macro_form)
        .equivalent_to(&builder.build(&operator_form)));
}
