//! Procedural macro for [`espresso-logic`](https://docs.rs/espresso-logic): the `expr!` Boolean
//! expression macro.

use proc_macro::TokenStream;
use proc_macro2::Span;
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

/// Parse OR expressions (lowest precedence).
fn parse_or(input: ParseStream) -> Result<Expr> {
    let mut left = parse_xor(input)?;
    while input.peek(Token![+]) || input.peek(Token![|]) {
        if input.peek(Token![+]) {
            input.parse::<Token![+]>()?;
        } else {
            input.parse::<Token![|]>()?;
        }
        let right = parse_xor(input)?;
        left = Expr::Or(Box::new(left), Box::new(right));
    }
    Ok(left)
}

/// Parse XOR expressions (between OR and AND).
fn parse_xor(input: ParseStream) -> Result<Expr> {
    let mut left = parse_and(input)?;
    while input.peek(Token![^]) {
        input.parse::<Token![^]>()?;
        let right = parse_and(input)?;
        left = Expr::Xor(Box::new(left), Box::new(right));
    }
    Ok(left)
}

/// Parse AND expressions (higher precedence).
fn parse_and(input: ParseStream) -> Result<Expr> {
    let mut left = parse_unary(input)?;
    while input.peek(Token![*]) || input.peek(Token![&]) {
        if input.peek(Token![*]) {
            input.parse::<Token![*]>()?;
        } else {
            input.parse::<Token![&]>()?;
        }
        let right = parse_unary(input)?;
        left = Expr::And(Box::new(left), Box::new(right));
    }
    Ok(left)
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
        let value: u8 = lit.base10_parse()?;
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

/// Build a [`BoolExpr`] from infix Boolean syntax.
///
/// `expr!(…)` produces an owned, syntactic `BoolExpr`, composing through [`BoolExpr::build`] so a large
/// expression is assembled cheaply. A `Bdd` is obtained from the result with `builder.build(&expr!(…))`.
///
/// # Operands
///
/// - identifier — an existing `BoolExpr` in scope, spliced in;
/// - `"x"` — a fresh variable named `x`;
/// - `0` / `1` — the constants `false` / `true` (any other integer is an error).
///
/// # Operators (highest to lowest precedence)
///
/// `( )` > `!` / `~` (NOT) > `*` / `&` (AND) > `^` (XOR) > `+` / `|` (OR).
///
/// ```ignore
/// use espresso_logic::{expr, BoolExpr};
///
/// let a = BoolExpr::var("a");
/// let b = BoolExpr::var("b");
/// let f = expr!(a & !b);              // splice existing expressions
/// let g = expr!("a" & !"b" | "c");    // fresh variables from string literals
/// ```
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
    TokenStream::from(quote! {
        ::espresso_logic::BoolExpr::build(|#builder| #body)
    })
}
