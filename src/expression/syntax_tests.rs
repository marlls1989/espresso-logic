//! Unit tests for the surface [`Syntax`](super::Syntax) parameter, exercised through
//! [`VerilogSyntax`](super::VerilogSyntax).
//!
//! These cover the Verilog lexicon — its XNOR spellings, its constants, and the spellings it rejects —
//! the round trip from text through `Display` and back, and the retag between syntaxes. The assertions
//! compare token streams rather than rendered strings wherever the *structure* is the point: two
//! spellings of one expression must reach the same canonical stream, and two different expressions must
//! not, whatever they print as. That is why this module sits inside the crate, where
//! [`tokens`](super::BoolExpr::tokens) is reachable.
//!
//! Every parse is spelled `text.parse::<BoolExpr<VerilogSyntax>>()`: origination is concrete on
//! `BoolExpr<StdSyntax>`, so there is no per-syntax inherent `parse` to call.

use super::rpn::Token;
use super::{BoolExpr, ExpressionParseError, ParseBoolExprError, StdSyntax, VerilogSyntax};
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

// ---- The parameter's footprint --------------------------------------------------------------------

#[test]
fn syntax_parameter_leaks_no_bounds() {
    // `Clone`, `PartialEq`, `Eq` and `Hash` are hand-written rather than derived, and the marker is
    // `PhantomData<fn() -> S>` rather than `S`. Between them, an expression is `Send`/`Sync`/`Clone`/
    // `Default` without the syntax type having to satisfy anything itself. Instantiating this for both
    // syntaxes is the check; it fails to compile if a bound ever leaks through.
    fn assert_props<T: Send + Sync + Clone + Default>() {}

    assert_props::<BoolExpr<StdSyntax>>();
    assert_props::<BoolExpr<VerilogSyntax>>();
}
