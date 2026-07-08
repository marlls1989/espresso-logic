//! Behaviour of the `expr!` macro: operand conventions, operator precedence, spelling equivalence, and
//! hygiene.

use espresso_logic::{expr, BoolExpr, Symbol};

#[test]
fn or_binds_looser_than_xor() {
    let a: BoolExpr = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let c = BoolExpr::var("c");
    assert_eq!(expr!(a + b ^ c), a.clone() | (b.clone() ^ c.clone()));
}

#[test]
fn xor_binds_looser_than_and() {
    let a: BoolExpr = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let c = BoolExpr::var("c");
    assert_eq!(expr!(a ^ b * c), a.clone() ^ (b.clone() & c.clone()));
}

#[test]
fn both_operator_spellings_agree() {
    let a: BoolExpr = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    assert_eq!(expr!(a * b), expr!(a & b));
    assert_eq!(expr!(a + b), expr!(a | b));
    assert_eq!(expr!(~a), expr!(!a));
}

#[test]
fn string_literals_are_fresh_variables() {
    assert_eq!(
        expr!("a" & "b"),
        BoolExpr::<Symbol>::var("a") & BoolExpr::var("b")
    );
}

#[test]
fn identifiers_graft_existing_expressions() {
    let sub: BoolExpr = BoolExpr::parse("a & b").unwrap();
    assert_eq!(expr!(sub | "c"), sub.clone() | BoolExpr::var("c"));
}

#[test]
fn integer_literals_are_constants() {
    assert_eq!(expr!(0), BoolExpr::<Symbol>::constant(false));
    assert_eq!(expr!(1), BoolExpr::<Symbol>::constant(true));
    let a: BoolExpr = BoolExpr::var("a");
    assert_eq!(expr!(a & 1), a.clone() & BoolExpr::constant(true));
}

#[test]
fn parentheses_override_precedence() {
    let a: BoolExpr = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let c = BoolExpr::var("c");
    assert_eq!(expr!((a + b) * c), (a.clone() | b.clone()) & c.clone());
}

#[test]
fn hygiene_does_not_capture_a_user_ident_named_like_the_builder() {
    // A user binding spelled exactly like the macro's internal builder ident must still resolve to the
    // user's value (mixed-site hygiene keeps the two distinct).
    let __expr_builder: BoolExpr = BoolExpr::var("z");
    assert_eq!(expr!(__expr_builder), BoolExpr::var("z"));
}

#[test]
fn reference_operands_graft() {
    let foo: BoolExpr = BoolExpr::var("x");
    assert_eq!(expr!(&foo), expr!(foo));
    assert_eq!(expr!("a" & &foo), expr!("a" & foo));

    // A real `&BoolExpr` binding: `expr!(r)` already works via deref coercion; `expr!(&r)` (a
    // double reference) must too.
    let r = &foo;
    assert_eq!(expr!(r), expr!(foo));
    assert_eq!(expr!(&r), expr!(foo));
}

#[test]
fn not_of_reference() {
    let foo: BoolExpr = BoolExpr::var("x");
    assert_eq!(expr!(!&foo), expr!(!foo));
}

#[test]
fn unspaced_double_ampersand_prefix_and_infix() {
    // `&&` lexes as a single `AndAnd` token in rustc's own tokenizer, but proc-macro2's token stream
    // splits it into two adjacent single-char `&` `Punct`s, so the atom parser sees the same tokens
    // whether the source is spaced (`& &foo`) or not (`&&foo`).

    // Leading `&&`: two reference levels folded into one graft operand.
    let foo: BoolExpr = BoolExpr::var("x");
    assert_eq!(expr!(&&foo), expr!(foo));

    // Infix `&&`: the AND tier's `&` operator consumes the *first* `&`; the leftover second `&` is
    // then picked up as a leading reference by the right operand's atom parser. So unspaced `a && b`
    // parses as `a & (&b)`, which — via `graft`'s `&BoolExpr` deref coercion — is the same value as
    // `a & b`. This only works when the right operand starts with a graft-operand starter
    // (identifier / `self` / path); a string-literal operand there is a parse error, since a leading
    // `&` must be followed by a graft operand, not a literal.
    let a: BoolExpr = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    assert_eq!(expr!(a && b), expr!(a & b));
}

#[test]
fn macro_call_operands_graft() {
    macro_rules! make {
        () => {
            expr!("m")
        };
    }
    let grafted: BoolExpr = expr!(make!());
    assert_eq!(grafted, expr!("m"));

    macro_rules! wrap {
        ($e:expr) => {
            $e
        };
    }
    let foo: BoolExpr = BoolExpr::var("x");
    assert_eq!(expr!(wrap!(foo.clone()) & "y"), expr!(foo.clone() & "y"));

    let bracketed: BoolExpr = expr!(make![]);
    assert_eq!(bracketed, expr!("m"));
    let braced: BoolExpr = expr!(make! {});
    assert_eq!(braced, expr!("m")); // brace-delimited macro call

    // A postfix chain continues after a bang-macro call: `make!()` grafts whole, then `.clone()`
    // (BoolExpr: Clone) applies to the result, same as calling it on the direct form.
    let cloned: BoolExpr = expr!(make!().clone());
    assert_eq!(cloned, expr!("m"));
}

#[test]
fn macro_and_operator_forms_are_equivalent_as_bdds() {
    let a: BoolExpr = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let macro_form = expr!(a * !b + !a * b);
    let operator_form = (a.clone() & !b.clone()) | (!a.clone() & b.clone());

    let builder: espresso_logic::BddBuilder<_, espresso_logic::LocalCell> =
        espresso_logic::bdd_builder!();
    assert!(builder
        .build(&macro_form)
        .equivalent_to(&builder.build(&operator_form)));
}
