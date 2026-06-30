//! Procedural macro for [`espresso-logic`](https://docs.rs/espresso-logic): the `expr!` Boolean
//! expression macro.

use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::{Ident, Token};

/// AST for Boolean expressions.
enum Expr {
    Variable(Ident),
    StringLiteral(syn::LitStr),
    Constant(bool),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Xor(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

impl Expr {
    /// Emit the body of a [`BoolExpr::build`] closure: an expression over the closure's [`Expr`] handles.
    ///
    /// `builder` is the closure's (hygienic) parameter ident. An identifier in scope is grafted in as an
    /// existing `BoolExpr`; a string literal becomes a fresh variable; a `0`/`1` becomes a constant. The
    /// operators compose the handles directly (the handle is `Copy` and implements `& | ^ !`).
    fn to_expr_tokens(&self, builder: &Ident) -> proc_macro2::TokenStream {
        match self {
            // An in-scope `BoolExpr`, spliced in.
            Expr::Variable(ident) => quote! { #builder.graft(&(#ident)) },
            Expr::StringLiteral(lit) => quote! { #builder.var(#lit) },
            Expr::Constant(value) => quote! { #builder.constant(#value) },
            Expr::Not(inner) => {
                let inner = inner.to_expr_tokens(builder);
                quote! { (!(#inner)) }
            }
            Expr::And(left, right) => {
                let left = left.to_expr_tokens(builder);
                let right = right.to_expr_tokens(builder);
                quote! { ((#left) & (#right)) }
            }
            Expr::Xor(left, right) => {
                let left = left.to_expr_tokens(builder);
                let right = right.to_expr_tokens(builder);
                quote! { ((#left) ^ (#right)) }
            }
            Expr::Or(left, right) => {
                let left = left.to_expr_tokens(builder);
                let right = right.to_expr_tokens(builder);
                quote! { ((#left) | (#right)) }
            }
        }
    }
}

/// Parser for Boolean expressions with operator precedence.
struct BoolExprParser {
    expr: Expr,
}

impl Parse for BoolExprParser {
    fn parse(input: ParseStream) -> Result<Self> {
        let expr = parse_or(input)?;
        Ok(BoolExprParser { expr })
    }
}

/// Parse one left-associative binary precedence tier.
///
/// `next` parses an operand at the next-higher tier; `op` peeks the input and, when this tier's operator is
/// present, consumes it and returns the [`Expr`] variant constructor to combine the two operands (the
/// `Expr::And`/`Xor`/`Or` variants are usable directly as `fn(Box<Expr>, Box<Expr>) -> Expr`). It returns
/// `None` when no operator of this tier follows, ending the fold. The three binary tiers differ only in
/// their `next` parser, operator tokens, and variant, so they all delegate here.
fn parse_binary_level(
    input: ParseStream,
    next: fn(ParseStream) -> Result<Expr>,
    op: impl Fn(ParseStream) -> Result<Option<fn(Box<Expr>, Box<Expr>) -> Expr>>,
) -> Result<Expr> {
    let mut left = next(input)?;
    while let Some(ctor) = op(input)? {
        let right = next(input)?;
        left = ctor(Box::new(left), Box::new(right));
    }
    Ok(left)
}

/// Parse OR expressions (lowest precedence). `+` and `|` are both accepted.
fn parse_or(input: ParseStream) -> Result<Expr> {
    parse_binary_level(input, parse_xor, |input| {
        if input.peek(Token![+]) {
            input.parse::<Token![+]>()?;
            Ok(Some(Expr::Or))
        } else if input.peek(Token![|]) {
            input.parse::<Token![|]>()?;
            Ok(Some(Expr::Or))
        } else {
            Ok(None)
        }
    })
}

/// Parse XOR expressions (between OR and AND).
fn parse_xor(input: ParseStream) -> Result<Expr> {
    parse_binary_level(input, parse_and, |input| {
        if input.peek(Token![^]) {
            input.parse::<Token![^]>()?;
            Ok(Some(Expr::Xor))
        } else {
            Ok(None)
        }
    })
}

/// Parse AND expressions (higher precedence). `*` and `&` are both accepted.
fn parse_and(input: ParseStream) -> Result<Expr> {
    parse_binary_level(input, parse_unary, |input| {
        if input.peek(Token![*]) {
            input.parse::<Token![*]>()?;
            Ok(Some(Expr::And))
        } else if input.peek(Token![&]) {
            input.parse::<Token![&]>()?;
            Ok(Some(Expr::And))
        } else {
            Ok(None)
        }
    })
}

/// Parse unary NOT and atoms (highest precedence). `!` and `~` are both accepted.
fn parse_unary(input: ParseStream) -> Result<Expr> {
    if input.peek(Token![!]) {
        input.parse::<Token![!]>()?;
        Ok(Expr::Not(Box::new(parse_unary(input)?)))
    } else if input.peek(Token![~]) {
        input.parse::<Token![~]>()?;
        Ok(Expr::Not(Box::new(parse_unary(input)?)))
    } else {
        parse_atom(input)
    }
}

/// Parse atoms: identifiers, string literals, the `0`/`1` constants, and parenthesised expressions.
fn parse_atom(input: ParseStream) -> Result<Expr> {
    if input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in input);
        parse_or(&content)
    } else if input.peek(syn::LitStr) {
        let lit: syn::LitStr = input.parse()?;
        Ok(Expr::StringLiteral(lit))
    } else if input.peek(syn::LitInt) {
        let lit: syn::LitInt = input.parse()?;
        // Parse wide so any out-of-range integer reaches the `0`/`1` check and reports the intended
        // message, rather than failing first with a generic "number too large to fit in u8".
        let value: u128 = lit.base10_parse()?;
        match value {
            0 => Ok(Expr::Constant(false)),
            1 => Ok(Expr::Constant(true)),
            _ => Err(syn::Error::new(
                lit.span(),
                "only 0 and 1 are supported as boolean constants",
            )),
        }
    } else {
        let ident: Ident = input.parse()?;
        Ok(Expr::Variable(ident))
    }
}

/// Resolve the path to the `espresso-logic` crate for use in the macro's output.
///
/// The expansion references the base crate, which a downstream crate may have renamed in its `Cargo.toml`.
/// [`crate_name`] returns [`FoundCrate::Name`] with the name actually in scope for a downstream crate, so a
/// renamed dependency is referenced as `::<renamed>`.
///
/// Every other case resolves to `::espresso_logic`. [`crate_name`] reports [`FoundCrate::Itself`] not only
/// for the library's own code but also for the package's examples, integration tests, and benches (they
/// share the package, yet each is a *separate* crate whose `crate` is its own root, so emitting `crate`
/// would be wrong there). `::espresso_logic` works in all of them: library code resolves it through the
/// `extern crate self as espresso_logic;` alias, and the other targets through the package dependency that
/// Cargo makes available under that name. The `Err` fallback resolves the same way.
fn espresso_logic_path() -> proc_macro2::TokenStream {
    match crate_name("espresso-logic") {
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        Ok(FoundCrate::Itself) | Err(_) => quote!(::espresso_logic),
    }
}

/// Build a [`BoolExpr`] from infix Boolean syntax.
///
/// See the [`expr!`](../espresso_logic/macro.expr.html) re-export in the `espresso-logic` crate for the full
/// documentation (operands, operator precedence, and examples); that is the documented public entry point.
#[proc_macro]
pub fn expr(input: TokenStream) -> TokenStream {
    let parser = match syn::parse::<BoolExprParser>(input) {
        Ok(parser) => parser,
        Err(e) => return e.to_compile_error().into(),
    };
    // `mixed_site` hygiene: the builder binding cannot capture or be captured by user identifiers, so
    // `expr!(b)` where the caller has a variable `b` is unaffected.
    let builder = Ident::new("__expr_builder", Span::mixed_site());
    let body = parser.expr.to_expr_tokens(&builder);
    let krate = espresso_logic_path();
    TokenStream::from(quote! {
        #krate::BoolExpr::build(|#builder| #body)
    })
}
