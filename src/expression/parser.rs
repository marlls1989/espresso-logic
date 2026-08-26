//! Parsing support for boolean expressions.
//!
//! The lalrpop grammar (`bool_expr.lalrpop`) emits a reverse-Polish [`Token`] program directly, which
//! [`BoolExpr::parse`] wraps into an owned [`BoolExpr`].

use super::error::{ExpressionParseError, ParseBoolExprError};
use super::rpn::Token;
use super::syntax::syntax_seal;
use super::{BoolExpr, Syntax};
use lalrpop_util::ParseError;
use std::sync::Arc;

// The lalrpop-generated parser, included from OUT_DIR via the library's own macro (the documented
// idiom). `clippy::all` blankets the generated code's clippy lints; it triggers no rustc warnings.
lalrpop_util::lalrpop_mod!(
    #[allow(clippy::all)]
    parser_impl,
    "/expression/bool_expr.rs"
);

/// Turn a lalrpop parse error over `input` into this crate's [`ParseBoolExprError`].
///
/// Generic over the token type because every generated grammar mints its own `Token<'input>`; the
/// location and user-error types are shared, since each grammar uses lalrpop's built-in lexer.
pub(crate) fn map_error<T: std::fmt::Display>(
    input: &str,
    e: ParseError<usize, T, &'static str>,
) -> ParseBoolExprError {
    // The grammar uses lalrpop's built-in lexer (no custom `Location`/`Error` types), so `e` is
    // `ParseError<usize, Token<'input>, &'static str>`: every location lalrpop reports is already
    // a byte offset into `input`. Extract it structurally instead of scraping the `Display` text.
    let position = match &e {
        ParseError::InvalidToken { location } => Some(*location),
        ParseError::UnrecognizedEof { location, .. } => Some(*location),
        ParseError::UnrecognizedToken {
            token: (start, ..), ..
        } => Some(*start),
        ParseError::ExtraToken { token: (start, ..) } => Some(*start),
        ParseError::User { .. } => None,
    };
    let message = e.to_string();
    ExpressionParseError::InvalidSyntax {
        message: Arc::from(message.as_str()),
        input: Arc::from(input),
        position,
    }
    .into()
}

/// Parse a string in the standard syntax into a reverse-Polish [`Token`] program.
///
/// The grammar entry point behind [`StdSyntax`](super::StdSyntax); [`BoolExpr::parse`] is its public
/// face.
pub(crate) fn parse_std(input: &str) -> Result<Vec<Token>, ParseBoolExprError> {
    parser_impl::ExprParser::new()
        .parse(input)
        .map_err(|e| map_error(input, e))
}

impl BoolExpr {
    /// Parse a boolean expression from a string, in the standard syntax.
    ///
    /// The lexicon and precedence below are those of [`StdSyntax`](super::StdSyntax), the syntax this
    /// returns an expression in. Text in another syntax goes through [`str::parse`] instead, naming the
    /// syntax on the target type (`text.parse::<BoolExpr<OtherSyntax>>()`); the syntax is not a
    /// parameter of this function, because a type parameter default does not participate in inference
    /// and every existing call site would become ambiguous.
    ///
    /// Supports standard boolean operators, in precedence order (lowest to highest):
    /// - `+` or `|` for OR
    /// - `^` for XOR
    /// - `*` or `&` for AND
    /// - `~` or `!` for NOT
    /// - Parentheses for grouping
    /// - Constants: `0`, `1`, `true`, `false`
    ///
    /// A variable name is one or more segments joined by `/` or `.`, so it can carry the hierarchical
    /// path a netlist gives a node — `xtop/xcore/net12`, or `xtop.xcore.net12` in the dotted spelling.
    /// The first segment opens with a letter or underscore, which is what keeps the constants above
    /// readable; later segments may be wholly numeric. Any segment may carry a bus index, `data<3>` or
    /// `data[3]`, or a range, `data<7:0>` / `data[7:0]`, in either the angle spelling Spectre writes or
    /// the bracket spelling Verilog does.
    ///
    /// The whole name is one variable: `bus[3]` and `bus[4]` are unrelated variables rather than one
    /// indexed object, and nothing reads structure into the path or the index. The characters admitted
    /// are those the operator set leaves free, so a name can never swallow an operator — which is why
    /// `!` and `+` stay out of names even though netlists use them (`vdd!`).
    ///
    /// All binary operators are left-associative. The result is the owned, syntactic [`BoolExpr`] of
    /// the parsed text (both the `*`/`+`/`~` and `&`/`|`/`!` spellings lower to the same canonical
    /// operator set).
    pub fn parse<S: AsRef<str>>(input: S) -> Result<Self, ParseBoolExprError> {
        let program = parse_std(input.as_ref())?;
        Ok(BoolExpr::from_tokens(Arc::from(program)))
    }
}

/// Parse a boolean expression from a string, so `"a + b".parse::<BoolExpr>()` and generic `FromStr`
/// bounds work.
///
/// This is also how an expression in a syntax other than the standard one is obtained from text: the
/// syntax is named on the target type, `text.parse::<BoolExpr<OtherSyntax>>()`, and the grammar that
/// syntax carries does the parsing. Left unannotated, `text.parse::<BoolExpr>()` is the standard syntax
/// through the type-position default, identical to the inherent [`BoolExpr::parse`].
impl<S: Syntax> std::str::FromStr for BoolExpr<S> {
    type Err = ParseBoolExprError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(BoolExpr::from_tokens(Arc::from(
            <S as syntax_seal::Sealed>::parse_tokens(s)?,
        )))
    }
}
