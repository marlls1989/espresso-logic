use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Ident, Token};
use syn::parse::{Parse, ParseStream, Result};

/// AST for boolean expressions
enum Expr {
    Variable(Ident),
    StringLiteral(syn::LitStr),
    Constant(bool),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

impl Expr {
    /// Generate code for this expression using references (no cloning in the macro)
    /// 
    /// The macro generates references and lets the monadic interface methods
    /// (and, or, not) handle any necessary cloning internally. This follows
    /// good Rust design - the macro doesn't assume ownership semantics.
    fn to_tokens(&self) -> proc_macro2::TokenStream {
        match self {
            Expr::Variable(ident) => {
                // Just use the variable by reference
                // The monadic methods already take &self and clone internally
                quote! {
                    #ident
                }
            }
            Expr::StringLiteral(lit) => {
                // Create a variable from the string literal
                quote! {
                    BoolExpr::variable(#lit)
                }
            }
            Expr::Constant(value) => {
                // Create a constant from the boolean value
                quote! {
                    BoolExpr::constant(#value)
                }
            }
            Expr::Not(inner) => {
                let inner_tokens = inner.to_tokens();
                quote! {
                    (&(#inner_tokens)).not()
                }
            }
            Expr::And(left, right) => {
                let left_tokens = left.to_tokens();
                let right_tokens = right.to_tokens();
                quote! {
                    (&(#left_tokens)).and(&(#right_tokens))
                }
            }
            Expr::Or(left, right) => {
                let left_tokens = left.to_tokens();
                let right_tokens = right.to_tokens();
                quote! {
                    (&(#left_tokens)).or(&(#right_tokens))
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
    let mut left = parse_and(input)?;

    while input.peek(Token![+]) || input.peek(Token![|]) {
        if input.peek(Token![+]) {
            input.parse::<Token![+]>()?;
        } else {
            input.parse::<Token![|]>()?;
        }
        let right = parse_and(input)?;
        left = Expr::Or(Box::new(left), Box::new(right));
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
                "only 0 and 1 are supported as boolean constants"
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
/// - `a + b` or `a | b` - OR operation (both `+` and `|` supported)
/// - `(a + b) * c` - Parentheses for grouping
///
/// # Operator Precedence
///
/// From highest to lowest:
/// 1. `( )` (Parentheses - force evaluation order)
/// 2. `!` / `~` (NOT)
/// 3. `*` / `&` (AND)
/// 4. `+` / `|` (OR)
///
/// # Examples
///
/// ```ignore
/// use espresso_logic::{BoolExpr, expr};
///
/// // Option 1: Use string literals (variables created automatically)
/// let xor = expr!("a" * "b" + !"a" * !"b");
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
/// let xor = expr!(a * b + !a * !b);
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
    let tokens = parser.expr.to_tokens();
    TokenStream::from(tokens)
}
