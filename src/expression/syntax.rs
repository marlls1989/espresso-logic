//! The sealed syntax marker governing a [`BoolExpr`](super::BoolExpr)'s surface form.
//!
//! A *syntax* is a zero-sized marker type that fixes the **surface form** of an expression: which
//! operator spellings its text form accepts when parsed, which spellings it emits when displayed, and
//! what precedence binds them. It selects no semantics — underneath, every expression is the same
//! canonical reverse-Polish token stream over `&`/`|`/`^`/`!`, whichever syntax labels it.
//!
//! [`StdSyntax`] is the crate's own surface form: `&`/`|`/`^`/`!` with `1`/`0` for the constants, in the
//! precedence order (loosest to tightest) `|` < `^` < `&` < `!` < atom.
//!
//! The trait is **sealed**: it cannot be implemented outside this crate. A syntax is inseparable from an
//! in-crate grammar that parses its spellings, so a downstream marker type would have nothing to parse
//! with and could not be made to work.

use super::error::ParseBoolExprError;
use super::rpn::Token;

/// A surface syntax for [`BoolExpr`](super::BoolExpr): accepted spellings, emitted spellings and
/// operator precedence.
///
/// Sealed: only this crate can implement it (see the module docs). A syntax carries no data — it is a
/// type-level marker that selects the grammar an expression's text form is parsed with and the
/// specification it is rendered against. `Copy + 'static` keeps syntax values trivially duplicable.
///
/// ```compile_fail
/// use espresso_logic::Syntax;
///
/// #[derive(Clone, Copy)]
/// struct MySyntax;
/// // error: `Syntax` is sealed; only `espresso_logic` can implement it.
/// impl Syntax for MySyntax {}
/// ```
pub trait Syntax: syntax_seal::Sealed + Copy + 'static {}

pub(crate) mod syntax_seal {
    use super::{ParseBoolExprError, SyntaxSpec, Token};

    /// Sealing supertrait for [`Syntax`](super::Syntax): only impls inside this crate can name it, so
    /// the syntax trait cannot be implemented downstream. Not part of the public API.
    ///
    /// The seal also *carries* the per-syntax payload — the specification a syntax renders against and
    /// the entry point into the grammar it parses with. Both speak in crate-internal types
    /// ([`SyntaxSpec`], [`Token`]); this module's `pub(crate)` visibility, and that of the `rpn` module
    /// [`Token`] lives in, keep the whole surface out of anything nameable downstream.
    pub trait Sealed: 'static {
        /// The spellings and binding tightnesses this syntax renders with.
        const SPEC: SyntaxSpec;

        /// Parse text in this syntax into a reverse-Polish [`Token`] program.
        fn parse_tokens(input: &str) -> Result<Vec<Token>, ParseBoolExprError>;
    }
}

/// The surface spellings and binding tightnesses of one syntax.
///
/// The operator spellings are plain strings, and NOT is given as a `not_prefix`/`not_suffix` **pair**
/// rather than a fixity flag: a prefix form leaves the suffix empty, a postfix form leaves the prefix
/// empty, and the renderer emits both around the operand either way.
///
/// The `prec_*` fields are binding-tightness levels, highest binds tightest. An atom (variable or
/// constant) binds tighter than any operator, and the relative order of the operator levels mirrors the
/// syntax's own grammar, so that a rendered expression re-parses to the tree it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntaxSpec {
    /// Spelling of AND, as an infix operator.
    pub and: &'static str,
    /// Spelling of OR, as an infix operator.
    pub or: &'static str,
    /// Spelling of XOR, as an infix operator.
    pub xor: &'static str,
    /// Text emitted before NOT's operand (empty for a postfix syntax).
    pub not_prefix: &'static str,
    /// Text emitted after NOT's operand (empty for a prefix syntax).
    pub not_suffix: &'static str,
    /// Spelling of the constant `true`.
    pub one: &'static str,
    /// Spelling of the constant `false`.
    pub zero: &'static str,
    /// Binding tightness of a variable or constant.
    pub prec_atom: u8,
    /// Binding tightness of NOT.
    pub prec_not: u8,
    /// Binding tightness of AND.
    pub prec_and: u8,
    /// Binding tightness of XOR.
    pub prec_xor: u8,
    /// Binding tightness of OR.
    pub prec_or: u8,
}

/// The crate's own expression syntax, and the default for [`BoolExpr`](super::BoolExpr).
///
/// Renders as `&` (AND), `|` (OR), `^` (XOR), `!` (NOT) and `1`/`0` for the constants, with the
/// precedence order (loosest to tightest) `|` < `^` < `&` < `!` < atom. Parsing additionally accepts the
/// `*`/`+`/`~` spellings and the `true`/`false` constants, all of which lower to the same canonical
/// token set — see [`BoolExpr::parse`](super::BoolExpr::parse) for the full lexicon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StdSyntax;

impl syntax_seal::Sealed for StdSyntax {
    const SPEC: SyntaxSpec = SyntaxSpec {
        and: "&",
        or: "|",
        xor: "^",
        not_prefix: "!",
        not_suffix: "",
        one: "1",
        zero: "0",
        prec_atom: 4,
        prec_not: 3,
        prec_and: 2,
        prec_xor: 1,
        prec_or: 0,
    };

    fn parse_tokens(input: &str) -> Result<Vec<Token>, ParseBoolExprError> {
        super::parser::parse_std(input)
    }
}

impl Syntax for StdSyntax {}
