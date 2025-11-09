use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Ident, Token};
use syn::parse::{Parse, ParseStream, Result};

/// AST for boolean expressions
enum Expr {
    Variable(Ident),
    StringLiteral(syn::LitStr),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

impl Expr {
    /// Generate code for this expression, cloning variables as needed
    fn to_tokens(&self) -> proc_macro2::TokenStream {
        match self {
            Expr::Variable(ident) => {
                // Clone the variable since we might use it multiple times
                quote! {
                    #ident.clone()
                }
            }
            Expr::StringLiteral(lit) => {
                // Create a variable from the string literal
                quote! {
                    BoolExpr::variable(#lit)
                }
            }
            Expr::Not(inner) => {
                let inner_tokens = inner.to_tokens();
                quote! {
                    (#inner_tokens).not()
                }
            }
            Expr::And(left, right) => {
                let left_tokens = left.to_tokens();
                let right_tokens = right.to_tokens();
                quote! {
                    (#left_tokens).and(&(#right_tokens))
                }
            }
            Expr::Or(left, right) => {
                let left_tokens = left.to_tokens();
                let right_tokens = right.to_tokens();
                quote! {
                    (#left_tokens).or(&(#right_tokens))
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

    while input.peek(Token![+]) {
        input.parse::<Token![+]>()?;
        let right = parse_and(input)?;
        left = Expr::Or(Box::new(left), Box::new(right));
    }

    Ok(left)
}

/// Parse AND expressions (higher precedence)
fn parse_and(input: ParseStream) -> Result<Expr> {
    let mut left = parse_unary(input)?;

    while input.peek(Token![*]) {
        input.parse::<Token![*]>()?;
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
    } else {
        parse_atom(input)
    }
}

/// Parse atomic expressions (variables, string literals, and parenthesized expressions)
fn parse_atom(input: ParseStream) -> Result<Expr> {
    if input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in input);
        parse_or(&content)
    } else if input.peek(syn::LitStr) {
        let lit: syn::LitStr = input.parse()?;
        Ok(Expr::StringLiteral(lit))
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
/// - `!a` - NOT operation
/// - `a * b` - AND operation
/// - `a + b` - OR operation
/// - `(a + b) * c` - Parentheses for grouping
///
/// # Operator Precedence
///
/// From highest to lowest:
/// 1. `!` (NOT)
/// 2. `*` (AND)
/// 3. `+` (OR)
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
/// // Option 3: Mix both styles
/// let expr1 = expr!(a * b);
/// let combined = expr!(expr1 + "c");
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
