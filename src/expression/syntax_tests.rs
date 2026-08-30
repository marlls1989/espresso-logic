//! Unit tests for the surface [`Syntax`](super::Syntax) parameter, exercised through
//! [`VerilogSyntax`](super::VerilogSyntax) and [`LibertySyntax`](super::LibertySyntax).
//!
//! These cover each syntax's lexicon — its spellings and the spellings it rejects — the round trip from
//! text through `Display` and back, and the retag between syntaxes, including the three-way retag that
//! carries one tree through all three lexicons. Liberty is where the precedence itself diverges (its XOR
//! binds tighter than its AND, the opposite of the order the standard and Verilog syntaxes share), so its
//! tests also cover that structurally. The assertions compare token streams rather than rendered strings
//! wherever the *structure* is the point: two spellings of one expression must reach the same canonical
//! stream, and two different expressions must not, whatever they print as. That is why this module sits
//! inside the crate, where [`tokens`](super::BoolExpr::tokens) is reachable.
//!
//! Every parse is spelled `text.parse::<BoolExpr<VerilogSyntax>>()` (or `<LibertySyntax>`): origination
//! is concrete on `BoolExpr<StdSyntax>`, so there is no per-syntax inherent `parse` to call.

use super::rpn::Token;
use super::{
    BoolExpr, ExpressionParseError, LibertySyntax, ParseBoolExprError, StdSyntax, VerilogSyntax,
};
use crate::Symbol;

/// Parse Verilog text that is expected to read, reporting the parse error if it does not.
fn verilog(text: &str) -> BoolExpr<VerilogSyntax> {
    text.parse::<BoolExpr<VerilogSyntax>>()
        .unwrap_or_else(|e| panic!("{text:?} should parse as Verilog: {e}"))
}

/// A variable token, for spelling an expected stream out literally.
fn var(name: &str) -> Token {
    Token::Var(Symbol::from(name))
}

/// Parse Verilog text that must fail, and return the byte offset lalrpop reported.
///
/// Both error enums are `#[non_exhaustive]`, but that binds downstream crates only: in here the single
/// variant of each is an exhaustive match, so there is no catch-all arm to write.
fn verilog_error_position(text: &str) -> Option<usize> {
    match text.parse::<BoolExpr<VerilogSyntax>>() {
        Err(ParseBoolExprError::Parse(ExpressionParseError::InvalidSyntax {
            position, ..
        })) => position,
        Ok(_) => panic!("expected a parse error for {text:?}"),
    }
}

/// Parse Liberty text that is expected to read, reporting the parse error if it does not.
fn liberty(text: &str) -> BoolExpr<LibertySyntax> {
    text.parse::<BoolExpr<LibertySyntax>>()
        .unwrap_or_else(|e| panic!("{text:?} should parse as Liberty: {e}"))
}

/// Parse Liberty text that must fail, and return the byte offset lalrpop reported.
fn liberty_error_position(text: &str) -> Option<usize> {
    match text.parse::<BoolExpr<LibertySyntax>>() {
        Err(ParseBoolExprError::Parse(ExpressionParseError::InvalidSyntax {
            position, ..
        })) => position,
        Ok(_) => panic!("expected a parse error for {text:?}"),
    }
}

// ---- Round-tripping -------------------------------------------------------------------------------

#[test]
fn verilog_text_round_trips_through_display() {
    // What a syntax emits it also parses, so rendering and re-reading must recover the same tree. The
    // right-nested shapes are the ones this can lose: `a & (b & c)` is a different expression from
    // `(a & b) & c`, and its parentheses may not be dropped as redundant.
    let corpus = [
        "a",
        "~a",
        "~~a",
        "a & b",
        "a | b",
        "a ^ b",
        "a ^~ b",
        "a & b | ~c",
        "a & (b | c)",
        "a & (b & c)",
        "a | (b | c)",
        "a ^ (b ^ c)",
        "~(a & b) | ~(a | b)",
        "a & ~(b ^~ c)",
        "1'b1 & a | 1'b0",
        "xtop/xcore/net12 & data[3]",
    ];

    for text in corpus {
        let parsed = verilog(text);
        let rendered = parsed.to_string();
        let reparsed = verilog(&rendered);
        assert_eq!(
            parsed.tokens(),
            reparsed.tokens(),
            "{text:?} rendered as {rendered:?}"
        );
    }
}

// ---- The XNOR spellings ---------------------------------------------------------------------------

#[test]
fn xnor_spellings_lower_to_the_xor_then_not_pair() {
    let expected = [var("a"), var("b"), Token::Xor, Token::Not];

    let caret_tilde = verilog("a ^~ b");
    let tilde_caret = verilog("a ~^ b");
    assert_eq!(caret_tilde.tokens(), expected.as_slice());
    assert_eq!(tilde_caret.tokens(), expected.as_slice());
    assert_eq!(caret_tilde, tilde_caret);

    // XNOR is no token of its own, so neither spelling comes back out: the pair renders as the tree it
    // is, `prec_xor < prec_not` forcing the parentheses.
    assert_eq!(caret_tilde.to_string(), "~(a ^ b)");
    assert_eq!(tilde_caret.to_string(), "~(a ^ b)");
}

#[test]
fn maximal_munch_separates_xnor_from_xor_of_a_not() {
    // The lexer takes the longest match, so `^~` is one operator while `^ ~` is two — a different
    // expression, not a spelling variant of the same one.
    let xnor = verilog("a ^~ b");
    let xor_of_not = verilog("a ^ ~b");

    assert_eq!(
        xor_of_not.tokens(),
        [var("a"), var("b"), Token::Not, Token::Xor].as_slice()
    );
    assert_ne!(xnor.tokens(), xor_of_not.tokens());
    assert_eq!(xor_of_not.to_string(), "a ^ ~b");
}

// ---- Constants and identifiers --------------------------------------------------------------------

#[test]
fn constants_read_in_every_spelling_and_emit_sized() {
    for text in ["1'b1", "1'B1", "1"] {
        assert_eq!(
            verilog(text).tokens(),
            [Token::Const(true)].as_slice(),
            "{text:?}"
        );
    }
    for text in ["1'b0", "1'B0", "0"] {
        assert_eq!(
            verilog(text).tokens(),
            [Token::Const(false)].as_slice(),
            "{text:?}"
        );
    }

    // Whichever spelling was read, the lowercase sized form is the one emitted.
    assert_eq!(verilog("1'B1").to_string(), "1'b1");
    assert_eq!(verilog("1").to_string(), "1'b1");
    assert_eq!(verilog("1'B0").to_string(), "1'b0");
    assert_eq!(verilog("0").to_string(), "1'b0");
}

#[test]
fn true_and_false_are_ordinary_identifiers() {
    // They name variables in Verilog rather than constants, so the grammar has no keyword for either.
    let f = verilog("true & false");
    assert_eq!(
        f.tokens(),
        [var("true"), var("false"), Token::And].as_slice()
    );
    assert_eq!(f.to_string(), "true & false");
}

// ---- What the lexicon excludes --------------------------------------------------------------------

#[test]
fn verilog_rejects_spellings_outside_its_lexicon() {
    // `*` and `+` are the standard syntax's AND and OR, which Verilog does not share; a `'` outside a
    // sized constant matches no rule; and `&&` is not this grammar's AND, so it fails at its second
    // `&`, where an operand is expected.
    for text in ["a * b", "a + b", "a'", "a && b"] {
        assert!(
            text.parse::<BoolExpr<VerilogSyntax>>().is_err(),
            "{text:?} should not parse as Verilog"
        );
    }
}

#[test]
fn empty_input_errs_at_offset_zero() {
    assert_eq!(verilog_error_position(""), Some(0));
}

// ---- Round-tripping (Liberty) ----------------------------------------------------------------------

#[test]
fn liberty_text_round_trips_through_display() {
    // As with Verilog, the right-nested shapes are what a lossy renderer would collapse: `a * (b * c)`
    // is a different tree from `(a * b) * c`, and Liberty's postfix `'` needs the same care as any other
    // operator.
    let corpus = [
        "a",
        "a'",
        "a''",
        "a * b",
        "a + b",
        "a ^ b",
        "a * b + c'",
        "a * (b + c)",
        "a * (b * c)",
        "a + (b + c)",
        "a ^ (b ^ c)",
        "(a * b)' + (a * c)'",
        "a * (b ^ c)'",
        // Liberty's XOR already binds tighter than AND, so these are the two shapes where its
        // parenthesisation diverges from the standard and Verilog syntaxes on the way out. `a * (b ^ c)`
        // carries redundant parentheses — `b ^ c` groups first regardless — so it renders bare as
        // `a * b ^ c`; `(a * b) ^ c` carries load-bearing ones, since without them the same text would
        // reparse as `a * (b ^ c)`. Both must still round-trip to the same tree, so this compares token
        // streams rather than pinning either rendered form.
        "a * (b ^ c)",
        "(a * b) ^ c",
        "1 * a + 0",
    ];

    for text in corpus {
        let parsed = liberty(text);
        let rendered = parsed.to_string();
        let reparsed = liberty(&rendered);
        assert_eq!(
            parsed.tokens(),
            reparsed.tokens(),
            "{text:?} rendered as {rendered:?}"
        );
    }
}

// ---- Liberty's postfix NOT -------------------------------------------------------------------------

#[test]
fn liberty_postfix_not_parenthesises_binary_operands_only() {
    // A binary operand needs parentheses to keep the `'` attached to the whole subexpression rather
    // than just its rightmost operand; an atom or another NOT does not.
    let grouped = liberty("(a + b)'");
    assert_eq!(grouped.to_string(), "(a + b)'");
    let reparsed = liberty(&grouped.to_string());
    assert_eq!(grouped.tokens(), reparsed.tokens());

    let bare = liberty("a'");
    assert_eq!(bare.to_string(), "a'");

    // Two primes are two `Not` tokens: the second negates what the first produced.
    let double = liberty("a''");
    assert_eq!(
        double.tokens(),
        [var("a"), Token::Not, Token::Not].as_slice()
    );
    assert_eq!(double.to_string(), "a''");

    // The prefix `!` is accepted on input but never emitted, so two prefix NOTs read the same tree as
    // two postfix primes and render the same way.
    let prefix_double = liberty("!!a");
    assert_eq!(prefix_double.tokens(), double.tokens());
    assert_eq!(prefix_double.to_string(), "a''");
}

// ---- Juxtaposition ----------------------------------------------------------------------------------

#[test]
fn juxtaposition_reads_as_and_but_never_writes_out() {
    // Juxtaposition is an input spelling only: whichever conjunction spelling parses, the `*` is what
    // comes back out.
    let spelled = liberty("a * b");
    let juxtaposed = liberty("a b");
    assert_eq!(juxtaposed.tokens(), spelled.tokens());
    assert_eq!(juxtaposed.to_string(), "a * b");
    assert_eq!(spelled.to_string(), "a * b");

    // A postfix NOT immediately followed by another identifier juxtaposes too: `a'b` is `!a & b`.
    let negated_juxt = liberty("a'b");
    assert_eq!(
        negated_juxt.tokens(),
        [var("a"), Token::Not, var("b"), Token::And].as_slice()
    );

    // Juxtaposition reaches over a parenthesised group as well.
    let over_group = liberty("a (b + c)");
    assert_eq!(
        over_group.tokens(),
        [var("a"), var("b"), var("c"), Token::Or, Token::And].as_slice()
    );
    assert_eq!(over_group.to_string(), "a * (b + c)");
}

// ---- Precedence divergence --------------------------------------------------------------------------

#[test]
fn xor_binds_tighter_than_and_in_liberty() {
    // Liberty's XOR sits inside AND, the opposite of the order the standard and Verilog syntaxes share:
    // `a ^ b * c` groups as `(a ^ b) & c` here, unparenthesised, where the standard syntax needs explicit
    // parentheses to say the same thing.
    let bare = liberty("a ^ b * c");
    let grouped = liberty("(a ^ b) * c");
    let other_grouping = liberty("a ^ (b * c)");

    assert_eq!(bare.tokens(), grouped.tokens());
    assert_ne!(bare.tokens(), other_grouping.tokens());

    // Retagging onto the standard syntax makes the same tree need the parentheses Liberty dropped.
    assert_eq!(bare.as_syntax::<StdSyntax>().to_string(), "(a ^ b) & c");
}

// ---- Constants and identifiers (Liberty) ------------------------------------------------------------

#[test]
fn true_is_an_ordinary_identifier_in_liberty() {
    // A Liberty function names its constants `1` and `0`; `true` carries no meaning here and reads as a
    // variable rather than a constant.
    let f = liberty("true");
    assert_eq!(f.tokens(), [var("true")].as_slice());
}

// ---- What the lexicon excludes (Liberty) ------------------------------------------------------------

#[test]
fn liberty_rejects_spellings_outside_its_lexicon() {
    // `~` is the standard/Verilog NOT and no part of Liberty's lexicon; `&&` is not this grammar's AND,
    // so it fails at its second `&`, where an operand is expected.
    for text in ["~a", "a && b"] {
        assert!(
            text.parse::<BoolExpr<LibertySyntax>>().is_err(),
            "{text:?} should not parse as Liberty"
        );
    }
}

#[test]
fn liberty_empty_input_errs_at_offset_zero() {
    assert_eq!(liberty_error_position(""), Some(0));
}

// ---- Retagging across syntaxes --------------------------------------------------------------------

#[test]
fn retagging_respells_the_same_tokens() {
    let std_expr = BoolExpr::parse("a & b | !c").expect("standard syntax should parse");
    assert_eq!(std_expr.to_string(), "a & b | !c");

    // The retag is a relabelling: same stream, Verilog spellings.
    let as_verilog = std_expr.as_syntax::<VerilogSyntax>();
    assert_eq!(as_verilog.tokens(), std_expr.tokens());
    assert_eq!(as_verilog.to_string(), "a & b | ~c");

    // And the Verilog grammar reads what it emitted back to that very stream.
    assert_eq!(verilog(&as_verilog.to_string()).tokens(), std_expr.tokens());

    // Retagging back recovers the standard type — and with it the standard rendering.
    let back: BoolExpr<StdSyntax> = as_verilog.as_syntax::<StdSyntax>();
    assert_eq!(back, std_expr);
    assert_eq!(back.to_string(), "a & b | !c");
}

#[test]
fn three_way_retag_respells_one_tree() {
    // `a ^~ b` is Verilog's XNOR: the XOR-then-NOT pair, with no operator of its own in either the
    // standard or the Liberty lexicon, so each retag falls back to parenthesising the XOR under its own
    // NOT spelling.
    let as_verilog = verilog("a ^~ b");

    let as_std = as_verilog.as_syntax::<StdSyntax>();
    assert_eq!(as_std.tokens(), as_verilog.tokens());
    assert_eq!(as_std.to_string(), "!(a ^ b)");

    let as_liberty = as_verilog.as_syntax::<LibertySyntax>();
    assert_eq!(as_liberty.tokens(), as_verilog.tokens());
    assert_eq!(as_liberty.to_string(), "(a ^ b)'");

    // Retagging back recovers the syntax the tokens started in, tokens and rendering both.
    let back: BoolExpr<VerilogSyntax> = as_liberty.as_syntax::<VerilogSyntax>();
    assert_eq!(back, as_verilog);
    assert_eq!(back.to_string(), "~(a ^ b)");
}

// ---- The parameter's footprint --------------------------------------------------------------------

#[test]
fn syntax_parameter_leaks_no_bounds() {
    // `Clone`, `PartialEq`, `Eq` and `Hash` are hand-written rather than derived, and the marker is
    // `PhantomData<fn() -> S>` rather than `S`. Between them, an expression is `Send`/`Sync`/`Clone`
    // without the syntax type having to satisfy anything itself. Instantiating this for all three
    // syntaxes is the check; it fails to compile if a bound ever leaks through.
    //
    // `Default` is deliberately not part of this check: like `var`/`constant`/`parse`/`build`, it is
    // an origination point and stays concrete on `BoolExpr<StdSyntax>` rather than generic over `S`
    // (see `syntax_tests::default_is_the_unannotated_constant_false` below), so
    // `BoolExpr<VerilogSyntax>`/`BoolExpr<LibertySyntax>` are not `Default` at all.
    fn assert_props<T: Send + Sync + Clone>() {}

    assert_props::<BoolExpr<StdSyntax>>();
    assert_props::<BoolExpr<VerilogSyntax>>();
    assert_props::<BoolExpr<LibertySyntax>>();
}

#[test]
fn default_is_the_unannotated_constant_false() {
    // `BoolExpr::default()` must resolve on its own, with nothing else in the expression naming a
    // type or pinning `S` through unification — that is the whole point of keeping `Default`
    // concrete (`impl Default for BoolExpr`, not `impl<S: Syntax> Default for BoolExpr<S>`).
    // `src/expression/tests.rs::default_is_constant_false` compares against `BoolExpr::constant(false)`
    // instead, which pins `S` via `PartialEq` regardless of how `Default` is declared, so it would not
    // catch a regression back to the generic impl. Asserting on `to_string()` here does not reintroduce
    // that: `Display` is implemented for every `S: Syntax`, so it adds no constraint of its own.
    let d = BoolExpr::default();
    assert_eq!(d.to_string(), "0");
}
