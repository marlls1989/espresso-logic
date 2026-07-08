//! Unit tests for the owned, syntactic [`BoolExpr`].
//!
//! These cover construction, structural equality/hashing, the bitwise operators, evaluation, parsing,
//! display round-tripping, and the bridge to the canonical BDD layer. Semantic equivalence is checked
//! either by evaluating over a full truth table (because `BoolExpr` equality is *syntactic*) or via a
//! `BddBuilder` (whose `equivalent_to` is O(1) canonical).

use super::BoolExpr;
use crate::expr;
use std::collections::HashSet;

/// Whether two expressions denote the same Boolean function. `BoolExpr` equality is *syntactic*, so
/// semantic equivalence is checked through the canonical BDD layer: both are built into one builder and
/// compared by `equivalent_to` (an O(1) canonical-root comparison).
fn equivalent(a: &BoolExpr, b: &BoolExpr) -> bool {
    let builder = crate::bdd_builder!();
    builder.build(a).equivalent_to(&builder.build(b))
}

// ---- Construction and structural equality ---------------------------------------------------------

#[test]
fn var_and_constant_construct() {
    let a: BoolExpr = BoolExpr::var("a");
    assert_eq!(a, BoolExpr::var("a"));
    assert_ne!(BoolExpr::<crate::Symbol>::var("a"), BoolExpr::var("b"));
    assert_ne!(
        BoolExpr::<crate::Symbol>::constant(true),
        BoolExpr::constant(false)
    );
    assert_ne!(
        BoolExpr::<crate::Symbol>::var("a"),
        BoolExpr::constant(true)
    );
}

#[test]
fn default_is_constant_false() {
    assert_eq!(
        BoolExpr::default(),
        BoolExpr::<crate::Symbol>::constant(false)
    );
}

#[test]
fn var_accepts_any_as_ref_str() {
    // `var` takes any `AsRef<str>`, not only `&str`.
    let from_string: BoolExpr = BoolExpr::var(String::from("x"));
    let from_str = BoolExpr::var("x");
    assert_eq!(from_string, from_str);
}

#[test]
fn clone_is_structural_equal() {
    let f: BoolExpr = BoolExpr::var("a") & BoolExpr::var("b");
    assert_eq!(f.clone(), f);
}

#[test]
fn equality_is_syntactic() {
    let a: BoolExpr = BoolExpr::var("a");
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
    let a: BoolExpr = BoolExpr::var("a");
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
    // The named forms (`and`/`or`/`xor`/`not`) are `pub(crate)` — implementation details the operators
    // delegate to, not part of the public API — but remain visible here since this test module is
    // crate-internal too; this checks operator/named equivalence.
    let a: BoolExpr = BoolExpr::var("a");
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
    let a: BoolExpr = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let builder = crate::bdd_builder!();
    let (ba, bb) = (builder.var("a"), builder.var("b"));

    assert!(builder
        .build(&(&a & &b))
        .equivalent_to(&(ba.clone() & bb.clone())));
    assert!(builder
        .build(&(&a | &b))
        .equivalent_to(&(ba.clone() | bb.clone())));
    assert!(builder.build(&(&a ^ &b)).equivalent_to(&(ba.clone() ^ bb)));
    assert!(builder.build(&!&a).equivalent_to(&!ba));
}

// ---- Folding over tokens --------------------------------------------------------------------------

#[test]
fn fold_walks_the_token_structure_including_xor() {
    use super::ExprNode;

    // f = (a ^ b) | !c — counts each operator node; the fold must visit a real Xor node.
    let expr: BoolExpr = expr!("a" ^ "b" | !"c");

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
        BoolExpr::<crate::Symbol>::parse("a * b + c").unwrap(),
        BoolExpr::parse("a & b | c").unwrap()
    );
    assert_eq!(
        BoolExpr::<crate::Symbol>::parse("~a").unwrap(),
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
    assert!(equivalent(
        &expr,
        &(BoolExpr::var("a") ^ BoolExpr::var("b"))
    ));
}

/// Conformance: the `expr!` proc-macro and the `BoolExpr::parse` grammar are independent parsers (Rust
/// tokens vs runtime text), but must agree. Each case asserts they build the **structurally identical**
/// `BoolExpr` (equality is syntactic), pinning precedence, associativity, and every operator spelling so a
/// future change to either parser that diverges is caught in CI.
#[test]
fn macro_and_parser_agree() {
    macro_rules! same {
        ($built:expr, $text:literal) => {{
            let parsed: BoolExpr = BoolExpr::parse($text).expect("parses");
            assert_eq!($built, parsed, "expr! and parse disagree on `{}`", $text);
        }};
    }

    // Precedence: AND binds tighter than XOR binds tighter than OR.
    same!(expr!("a" | "b" & "c"), "a | b & c");
    same!(expr!("a" ^ "b" & "c"), "a ^ b & c");
    same!(expr!("a" | "b" ^ "c"), "a | b ^ c");
    same!(expr!("a" | "b" & "c" ^ "d"), "a | b & c ^ d");

    // Left-associativity of each binary operator.
    same!(expr!("a" & "b" & "c"), "a & b & c");
    same!(expr!("a" | "b" | "c"), "a | b | c");
    same!(expr!("a" ^ "b" ^ "c"), "a ^ b ^ c");

    // Parentheses override precedence.
    same!(expr!(("a" | "b") & "c"), "(a | b) & c");
    same!(expr!("a" & ("b" | "c")), "a & (b | c)");

    // NOT binds tightest; double negation nests.
    same!(expr!(!"a"), "!a");
    same!(expr!(!!"a"), "!!a");
    same!(expr!(!("a" & "b")), "!(a & b)");

    // Constants.
    same!(expr!("a" & 1), "a & 1");
    same!(expr!("a" | 0), "a | 0");

    // Every spelling: the macro's `*`/`+`/`~` build the same tree as the parser's `&`/`|`/`!`.
    same!(expr!("a" * "b"), "a & b");
    same!(expr!("a" + "b"), "a | b");
    same!(expr!(~"a"), "!a");
    same!(expr!(~"a" * "b" + "c"), "!a & b | c");
}

// ---- Display --------------------------------------------------------------------------------------

#[test]
fn display_uses_canonical_spellings_minimal_parens() {
    let a: BoolExpr = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let c = BoolExpr::var("c");

    assert_eq!(expr!(a & b | c).to_string(), "a & b | c");
    assert_eq!(expr!((a | b) & c).to_string(), "(a | b) & c");
    assert_eq!(expr!(!(a & b)).to_string(), "!(a & b)");
    assert_eq!(expr!(!a & b).to_string(), "!a & b");
    assert_eq!(expr!(a ^ b).to_string(), "a ^ b");
    assert_eq!(BoolExpr::<crate::Symbol>::constant(true).to_string(), "1");
    assert_eq!(BoolExpr::<crate::Symbol>::constant(false).to_string(), "0");
}

#[test]
fn display_reparses_to_equivalent() {
    // Display is syntactic, but parsing its output must recover an equivalent function.
    let exprs = [
        expr!("a" & "b" | !"c"),
        expr!(("a" | "b") & "c"),
        expr!("a" ^ "b" ^ "c"),
        expr!(!("a" & ("b" | "c"))),
    ];
    for expr in &exprs {
        let reparsed = BoolExpr::parse(expr.to_string()).unwrap();
        assert!(
            equivalent(expr, &reparsed),
            "display `{expr}` did not reparse equivalently"
        );
    }
}

#[test]
fn display_round_trips_right_nested_associative() {
    // `& | ^` are left-associative, so a right-nested tree must keep enough parentheses that
    // re-parsing rebuilds the *same syntactic tree* — not merely an equivalent function. (`BoolExpr`
    // equality is structural.)
    let exprs: [BoolExpr; 5] = [
        expr!("a" & ("b" & "c")),
        expr!("a" | ("b" | "c")),
        expr!("a" ^ ("b" ^ "c")),
        expr!("a" & ("b" | "c")),
        expr!("a" | ("b" ^ "c")),
    ];
    for expr in &exprs {
        let reparsed = BoolExpr::parse(expr.to_string()).unwrap();
        assert_eq!(
            *expr, reparsed,
            "display `{expr}` did not round-trip syntactically"
        );
    }

    // Minimal parentheses: the right-nested form keeps them, the left-nested form drops them.
    let right_nested: BoolExpr = expr!("a" & ("b" & "c"));
    assert_eq!(right_nested.to_string(), "a & (b & c)");
    let left_nested: BoolExpr = expr!(("a" & "b") & "c");
    assert_eq!(left_nested.to_string(), "a & b & c");
}

// ---- Syntactic variables --------------------------------------------------------------------------

#[test]
fn variables_are_syntactic() {
    let f: BoolExpr = expr!("b" & ("a" | !"a"));
    // `variables()` yields in token order (not sorted); sort here to compare. The scan reports every
    // occurring variable once (a appears even though a | !a is a tautology).
    let mut vars: Vec<String> = f.variables().map(|s| s.to_string()).collect();
    vars.sort();
    assert_eq!(vars, vec!["a".to_string(), "b".to_string()]);
}

// ---- Graft operands (the expr! splice form) -------------------------------------------------------

#[test]
fn graft_accepts_postfix_expressions() {
    // A bare local, the original splice form.
    let a: BoolExpr = BoolExpr::var("a");
    assert_eq!(expr!(a & "b"), a.clone() & BoolExpr::var("b"));

    // A field access and a method call on it.
    struct Gates {
        set: BoolExpr,
        reset: BoolExpr,
    }
    impl Gates {
        fn enable(&self) -> BoolExpr {
            self.set.clone() & self.reset.clone()
        }
    }
    let g = Gates {
        set: BoolExpr::var("s"),
        reset: BoolExpr::var("r"),
    };
    assert_eq!(expr!(g.set | g.reset), g.set.clone() | g.reset.clone());
    assert_eq!(expr!(g.enable() ^ "t"), g.enable() ^ BoolExpr::var("t"));

    // A `::` path to a function call.
    mod helpers {
        use super::BoolExpr;
        pub fn z() -> BoolExpr {
            BoolExpr::var("z")
        }
    }
    assert_eq!(expr!(helpers::z() | "w"), helpers::z() | BoolExpr::var("w"));

    // Indexing into a slice of expressions.
    let gates: [BoolExpr; 2] = [BoolExpr::var("x"), BoolExpr::var("y")];
    assert_eq!(
        expr!(gates[0] & gates[1]),
        gates[0].clone() & gates[1].clone()
    );
}

// ---- Bridge to the BDD layer ----------------------------------------------------------------------

#[test]
fn bdd_build_matches_parse() {
    let builder = crate::bdd_builder!();
    let f: BoolExpr = expr!("a" & "b" | "c");
    let built = builder.build(&f);
    let parsed = builder.parse("a & b | c").unwrap();
    assert!(built.equivalent_to(&parsed));
}

#[test]
fn bdd_build_canonicalises_commutativity() {
    let builder = crate::bdd_builder!();
    // a & b and b & a are different BoolExpr values but the same Bdd.
    let ab_expr: BoolExpr = expr!("a" & "b");
    let ba_expr: BoolExpr = expr!("b" & "a");
    let ab = builder.build(&ab_expr);
    let ba = builder.build(&ba_expr);
    assert!(ab.equivalent_to(&ba));
}

#[test]
fn to_expr_round_trips_semantically() {
    let builder = crate::bdd_builder!();
    let f = expr!(("a" & "b") | ("a" & "c"));
    let bdd = builder.build(&f);
    let recovered = bdd.to_expr();
    // to_expr is factored/syntactic, so compare semantically through the BDD layer.
    assert!(equivalent(&f, &recovered));
}

#[test]
fn sync_context_build_works() {
    let builder = crate::sync_bdd_builder!();
    let f: BoolExpr = expr!("a" ^ "b");
    assert!(builder
        .build(&f)
        .equivalent_to(&builder.parse("a ^ b").unwrap()));
}

// ---- Generic label parameter S ---------------------------------------------------------------------

#[test]
fn parse_generic_label_displays_and_matches_symbol_parse() {
    let text = "a & (b | !c)";
    let string_parsed: BoolExpr<String> = text.parse().unwrap();
    let symbol_parsed: BoolExpr = BoolExpr::parse(text).unwrap();

    // Display round-trips to the original text under a non-Symbol stored label.
    assert_eq!(string_parsed.to_string(), text);

    // Same syntactic tree as the Symbol parse, only the stored label type differs.
    assert_eq!(string_parsed.to_string(), symbol_parsed.to_string());

    // The turbofish constructor selects the same stored label type as the annotated binding.
    assert_eq!(string_parsed, BoolExpr::<String>::parse(text).unwrap());
}

#[test]
fn variables_generic_label_yields_stored_type() {
    let f: BoolExpr<String> = "a & (b | !c)".parse().unwrap();
    let mut vars: Vec<String> = f.variables().collect();
    vars.sort();
    assert_eq!(
        vars,
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
}

#[test]
fn arc_str_label_preserves_display() {
    let text = "a & (b | !c)";
    let symbol_parsed: BoolExpr = BoolExpr::parse(text).unwrap();
    let arc: BoolExpr<std::sync::Arc<str>> = text.parse().unwrap();
    assert_eq!(arc.to_string(), symbol_parsed.to_string());
}

#[test]
fn cast_reinterns_labels_into_target_type() {
    let text = "a & (b | !c)";
    let symbol_parsed: BoolExpr = BoolExpr::parse(text).unwrap();

    // The crate-internal `cast` re-keys every variable name into the target label type while keeping
    // the same syntactic tree, so it matches a direct parse into that label type.
    let as_string: BoolExpr<String> = symbol_parsed.cast::<String>();
    assert_eq!(as_string, BoolExpr::<String>::parse(text).unwrap());
    assert_eq!(as_string.to_string(), text);
}
