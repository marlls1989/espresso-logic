//! Procedural macro for [`espresso-logic`](https://docs.rs/espresso-logic): the `expr!` Boolean
//! expression macro.

use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream, Result};
use syn::{Ident, Token};

/// AST for Boolean expressions.
enum Expr {
    /// An existing expression to splice in via `graft` — its captured tokens (a path, field access,
    /// method/function call, index, or a bare identifier; see [`parse_graft_operand`]).
    Graft(proc_macro2::TokenStream),
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
    /// `builder` is the closure's (hygienic) parameter ident. An in-scope expression is grafted in as an
    /// existing `BoolExpr`; a string literal becomes a fresh variable; a `0`/`1` becomes a constant. The
    /// operators compose the handles directly (the handle is `Copy` and implements `& | ^ !`).
    fn to_expr_tokens(&self, builder: &Ident) -> proc_macro2::TokenStream {
        match self {
            // An in-scope `BoolExpr`, spliced in. A non-`BoolExpr` operand is a type error at this call.
            Expr::Graft(expr) => quote! { #builder.graft(&(#expr)) },
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

/// Parse atoms: identifiers, string literals, the `0`/`1` constants, parenthesised expressions, and
/// (optionally `&`-referenced) graft operands.
fn parse_atom(input: ParseStream) -> Result<Expr> {
    // Domain-specific message for a token that cannot start an operand, shared by every failure path
    // below (`parse_graft_operand` on its own would only surface syn's generic "expected identifier").
    const EXPECTED: &str = "expected a Boolean operand: a string literal (\"name\"), a constant (0 or \
        1), a parenthesised expression, or a (possibly `&`-referenced) expression yielding a `BoolExpr`";

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
    } else if input.peek(Token![&]) {
        // `&` in operand position is a reference, never the binary AND operator: `parse_atom` is only
        // ever invoked in operand position (immediately after an operator, or at expression start), and
        // `parse_and` already peels off any *binary* `&` between two complete operands before recursing
        // down to an atom. So this branch is only reached for a leading `&`.
        //
        // Peek (on a fork, so nothing is consumed on a `false` result) past the leading `&`(s) — there
        // may be several, e.g. `&&foo` — for a graft-operand starter. Only then commit: fold the `&`(s)
        // into the captured graft stream verbatim. `graft` takes `&BoolExpr`, and `&(&foo)` (or deeper
        // reference levels) deref-coerces back down to it at the call site.
        let ahead = input.fork();
        while ahead.peek(Token![&]) {
            ahead.parse::<Token![&]>()?;
        }
        if ahead.peek(Token![self]) || ahead.peek(Token![::]) || ahead.peek(Ident) {
            let mut tokens = proc_macro2::TokenStream::new();
            while input.peek(Token![&]) {
                input.parse::<Token![&]>()?.to_tokens(&mut tokens);
            }
            tokens.extend(parse_graft_operand(input)?);
            Ok(Expr::Graft(tokens))
        } else {
            // A `&` not followed by a graft starter is not a valid operand either.
            Err(syn::Error::new(input.span(), EXPECTED))
        }
    } else if input.peek(Token![self]) || input.peek(Token![::]) || input.peek(Ident) {
        Ok(Expr::Graft(parse_graft_operand(input)?))
    } else {
        // Nothing here can start a graft operand either (no `&`, no `self`, no path, no identifier).
        // Report the domain-specific set of accepted operands, at the offending token (or, if input is
        // exhausted, at the end of the stream).
        Err(syn::Error::new(input.span(), EXPECTED))
    }
}

/// Parse a graft operand — an in-scope expression to splice in via `graft` — capturing its tokens.
///
/// Accepts a *postfix* expression: a leading `self` or a (possibly `::`-rooted) path, optionally followed by
/// a bang-macro call (`path!(…)` / `path![…]` / `path!{…}`) grafting the macro's expansion whole, then any
/// number of field accesses (`.field`), method calls (`.method(args)`), function calls (`(args)`), and
/// indexes (`[index]`). Argument, index, and macro-call groups are captured whole (their delimiters bound
/// them), so the tokens inside them are unrestricted. Parsing stops before any binary operator, leaving it
/// for the surrounding precedence parser — so the macro's own `&`/`|`/`^`/`+`/`*` are never mistaken for
/// Rust operators. An operand that itself needs a top-level binary operator must be bound to a local first.
fn parse_graft_operand(input: ParseStream) -> Result<proc_macro2::TokenStream> {
    let mut tokens = proc_macro2::TokenStream::new();

    // Leading primary: `self`, or a path of identifiers with an optional leading `::`.
    let is_self = input.peek(Token![self]);
    if is_self {
        input.parse::<Token![self]>()?.to_tokens(&mut tokens);
    } else {
        if input.peek(Token![::]) {
            input.parse::<Token![::]>()?.to_tokens(&mut tokens);
        }
        input.parse::<Ident>()?.to_tokens(&mut tokens);
        while input.peek(Token![::]) {
            input.parse::<Token![::]>()?.to_tokens(&mut tokens);
            input.parse::<Ident>()?.to_tokens(&mut tokens);
        }
    }

    // A bang-macro call (`path!(…)`, `path![…]`, or `path!{…}`) grafts the macro's expansion whole, e.g.
    // `expr!(make!())`. Only valid after a path — `self!` is not valid Rust — and only when a delimited
    // group follows the `!` (mirroring how the postfix loop below only takes `(…)` as call parens after a
    // method name); otherwise the `!` is left where it is (e.g. for the unary-NOT parser to pick up).
    if !is_self
        && input.peek(Token![!])
        && (input.peek2(syn::token::Paren)
            || input.peek2(syn::token::Brace)
            || input.peek2(syn::token::Bracket))
    {
        input.parse::<Token![!]>()?.to_tokens(&mut tokens);
        input
            .parse::<proc_macro2::TokenTree>()?
            .to_tokens(&mut tokens);
    }

    // Postfix chain: field access / tuple index, method or function calls, and indexing. A `(…)`/`[…]`
    // group is pulled as one token tree, capturing its delimiters and inner tokens verbatim.
    loop {
        if input.peek(Token![.]) {
            input.parse::<Token![.]>()?.to_tokens(&mut tokens);
            if input.peek(syn::LitInt) {
                input.parse::<syn::LitInt>()?.to_tokens(&mut tokens);
            } else {
                input.parse::<Ident>()?.to_tokens(&mut tokens);
            }
            if input.peek(syn::token::Paren) {
                input
                    .parse::<proc_macro2::TokenTree>()?
                    .to_tokens(&mut tokens);
            }
        } else if input.peek(syn::token::Paren) || input.peek(syn::token::Bracket) {
            input
                .parse::<proc_macro2::TokenTree>()?
                .to_tokens(&mut tokens);
        } else {
            break;
        }
    }

    Ok(tokens)
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
