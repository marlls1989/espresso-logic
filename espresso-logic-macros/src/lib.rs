use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::{parse_macro_input, Ident, Token};

/// AST for boolean expressions
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
    /// Emit the body of a `BoolExpr::build` closure: builder method calls that compose `Bdd` handles,
    /// rather than chained monadic `BoolExpr` operations.
    ///
    /// `builder` is the closure's (hygienic) parameter ident. Variable identifiers in scope are grafted
    /// in as existing `BoolExpr`s; string literals become fresh variables; the whole expression is built
    /// under the single lock `build` holds. Methods take `&self`, so nested calls in one expression are
    /// fine.
    fn to_bdd_tokens(&self, builder: &Ident) -> proc_macro2::TokenStream {
        match self {
            Expr::Variable(ident) => {
                // An in-scope BoolExpr (or &BoolExpr — deref coercion handles the extra reference).
                quote! {
                    #builder.graft(&(#ident))
                }
            }
            Expr::StringLiteral(lit) => {
                quote! {
                    #builder.var(#lit)
                }
            }
            Expr::Constant(value) => {
                quote! {
                    #builder.constant(#value)
                }
            }
            Expr::Not(inner) => {
                let inner_tokens = inner.to_bdd_tokens(builder);
                quote! {
                    #builder.not(#inner_tokens)
                }
            }
            Expr::And(left, right) => {
                let left_tokens = left.to_bdd_tokens(builder);
                let right_tokens = right.to_bdd_tokens(builder);
                quote! {
                    #builder.and(#left_tokens, #right_tokens)
                }
            }
            Expr::Xor(left, right) => {
                let left_tokens = left.to_bdd_tokens(builder);
                let right_tokens = right.to_bdd_tokens(builder);
                quote! {
                    #builder.xor(#left_tokens, #right_tokens)
                }
            }
            Expr::Or(left, right) => {
                let left_tokens = left.to_bdd_tokens(builder);
                let right_tokens = right.to_bdd_tokens(builder);
                quote! {
                    #builder.or(#left_tokens, #right_tokens)
                }
            }
        }
    }
}

/// Parser for boolean expressions with operator precedence
struct BoolExprParser {
    expr: Expr,
}

impl Parse for BoolExprParser {
    fn parse(input: ParseStream) -> Result<Self> {
        let expr = parse_or(input)?;
        Ok(BoolExprParser { expr })
    }
}

/// Parse OR expressions (lowest precedence)
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

/// Parse XOR expressions (between OR and AND)
fn parse_xor(input: ParseStream) -> Result<Expr> {
    let mut left = parse_and(input)?;

    while input.peek(Token![^]) {
        input.parse::<Token![^]>()?;
        let right = parse_and(input)?;
        left = Expr::Xor(Box::new(left), Box::new(right));
    }

    Ok(left)
}

/// Parse AND expressions (higher precedence)
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

/// Parse unary expressions (NOT) and atoms (highest precedence)
fn parse_unary(input: ParseStream) -> Result<Expr> {
    if input.peek(Token![!]) {
        input.parse::<Token![!]>()?;
        let inner = parse_unary(input)?;
        Ok(Expr::Not(Box::new(inner)))
    } else if input.peek(Token![~]) {
        input.parse::<Token![~]>()?;
        let inner = parse_unary(input)?;
        Ok(Expr::Not(Box::new(inner)))
    } else {
        parse_atom(input)
    }
}

/// Parse atomic expressions (variables, string literals, numeric literals, and parenthesized expressions)
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

/// The `expr!` procedural macro for boolean expressions
///
/// Provides clean syntax for building boolean expressions from existing `BoolExpr` values
/// with proper operator precedence.
///
/// # Supported Syntax
///
/// - `a` - Variable or any `BoolExpr` identifier in scope
/// - `"a"` - String literal (creates `BoolExpr::variable("a")` automatically)
/// - `0` - False constant (creates `BoolExpr::constant(false)`)
/// - `1` - True constant (creates `BoolExpr::constant(true)`)
/// - `!a` or `~a` - NOT operation (both syntaxes supported, like the parser)
/// - `a * b` or `a & b` - AND operation (both `*` and `&` supported)
/// - `a ^ b` - XOR operation
/// - `a + b` or `a | b` - OR operation (both `+` and `|` supported)
/// - `(a + b) * c` - Parentheses for grouping
///
/// # Operator Precedence
///
/// From highest to lowest:
/// 1. `( )` (Parentheses - force evaluation order)
/// 2. `!` / `~` (NOT)
/// 3. `*` / `&` (AND)
/// 4. `^` (XOR)
/// 5. `+` / `|` (OR)
///
/// # Examples
///
/// ```ignore
/// use espresso_logic::{BoolExpr, expr};
///
/// // Option 1: Use string literals (variables created automatically)
/// let xor = expr!("a" * !"b" + !"a" * "b");
/// let complex = expr!(("a" + "b") * "c");
///
/// // Option 2: Use existing BoolExpr variables
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let c = BoolExpr::variable("c");
/// let and_expr = expr!(a * b);
/// let or_expr = expr!(a + b);
/// let not_expr = expr!(!a);
///
/// // Option 3: Use constants
/// let with_const = expr!("a" * 1 + "b" * 0);  // a AND true OR b AND false
/// let always_true = expr!(1);
/// let always_false = expr!(0);
///
/// // Option 4: Mix all styles
/// let expr1 = expr!(a * b);
/// let combined = expr!(expr1 + "c" * 1);
///
/// // Complex nested expressions
/// let xor = expr!(a * !b + !a * b);
/// let complex = expr!((a + b) * c);
///
/// // Can compose sub-expressions
/// let sub_expr1 = expr!(a * b);
/// let sub_expr2 = expr!(c + !a);
/// let combined = expr!(sub_expr1 + sub_expr2);
/// ```
#[proc_macro]
pub fn expr(input: TokenStream) -> TokenStream {
    let parser = parse_macro_input!(input as BoolExprParser);
    // `mixed_site` hygiene: the builder binding is invisible to (and cannot capture) user identifiers,
    // so an expression like `expr!(b)` where the user has a variable `b` is unaffected.
    let builder = Ident::new("__expr_builder", Span::mixed_site());
    let body = parser.expr.to_bdd_tokens(&builder);
    let tokens = quote! {
        BoolExpr::build(|#builder| #body)
    };
    TokenStream::from(tokens)
}
