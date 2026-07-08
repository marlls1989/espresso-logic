//! Owned, syntactic Boolean expressions.
//!
//! This module provides [`BoolExpr`], an **owned, syntactic** Boolean expression. A `BoolExpr` is a
//! value: build it, compose it, parse it, display it, and fold over its structure. Internally it is a
//! flat reverse-Polish token stream.
//!
//! `BoolExpr` is purely syntactic. It does **not** canonicalise: `a & b` and `b & a` are *different*
//! expressions, and equality ([`Eq`]) compares the token structure, not the Boolean function. For
//! canonical, semantic operations — logical equivalence, Shannon cofactors, quantification, tautology
//! checks — build the expression into a [`Bdd`](crate::bdd::Bdd) through a
//! [`BddBuilder`](crate::bdd::BddBuilder) and use that layer.
//!
//! # Construction
//!
//! - [`expr!`](crate::expr) — infix Boolean syntax; the recommended way to compose. It is sugar
//!   for [`BoolExpr::build`], the closure builder it lowers to; reach for `build` directly when
//!   construction is data-driven (looping/folding a runtime set of variables).
//! - [`BoolExpr::parse`] / [`str::parse`] — from text.
//! - [`BoolExpr::var`] / [`BoolExpr::constant`] — leaves. The bitwise operators `&` (AND), `|`
//!   (OR), `^` (XOR), `!` (NOT) and the named methods also compose, but each reallocates the token
//!   stream, so `expr!`/`build` are preferred beyond a couple of terms.
//!
//! ```
//! use espresso_logic::{expr, BoolExpr};
//!
//! let f = expr!("a" & "b" | !"c");
//! let g = BoolExpr::parse("a & b | !c").unwrap();
//! // Structural equality: the same syntactic tree.
//! assert_eq!(f, g);
//! ```
//!
//! # Label types
//!
//! [`BoolExpr<S>`](BoolExpr) is generic over its stored variable-name type `S`, bounded by
//! [`StringLabel`] and defaulting to [`Symbol`]. The stored label type resolves by annotation, by
//! turbofish, or by how the value is consumed: `let e: BoolExpr = BoolExpr::var("a")` pins it to the
//! `Symbol` default, `BoolExpr::<String>::var("a")` selects `String`, and parsing chooses it from the
//! target type — `"a & b".parse::<BoolExpr<String>>()`.

// Submodules
mod ast;
mod builder;
mod display;
pub mod error;
pub(crate) mod factorization;
mod operators;
mod parser;
pub(crate) mod rpn;

pub use error::{ExpressionParseError, ParseBoolExprError};

// Re-export AST types
pub(crate) use ast::BoolExprAst;
pub use ast::ExprNode;

// The auxiliary builder behind `BoolExpr::build`.
pub use builder::{Expr, ExprBuilder};

use crate::{StringLabel, Symbol};
use rpn::Token;

use std::sync::Arc;

/// An owned, syntactic Boolean expression.
///
/// A `BoolExpr` is a value: build it, compose it with the bitwise operators, [`parse`](Self::parse) it
/// from text, [`Display`](std::fmt::Display) it, and [`fold`](Self::fold) over its structure. Semantic
/// operations — logical equivalence, evaluation, cofactors — live on [`Bdd`](crate::bdd::Bdd).
///
/// # Equality is *syntactic*, not semantic
///
/// [`PartialEq`]/[`Eq`]/[`Hash`] compare the **token structure** of the expression, i.e. its syntax.
/// Two expressions are equal exactly when they are the same syntactic tree:
///
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::var("a");
/// let b = BoolExpr::var("b");
/// assert_eq!(a.clone() & b.clone(), a.clone() & b.clone()); // same structure
/// assert_ne!(a.clone() & b.clone(), b.clone() & a.clone()); // a & b is NOT b & a syntactically
/// assert_ne!(a.clone() & b.clone(), a.clone() | b.clone()); // different operator
/// ```
///
/// This is **not** logical/semantic equality. `a & b` and `b & a` denote the same Boolean function but
/// are different `BoolExpr` values. To compare functions, build both into [`Bdd`](crate::bdd::Bdd)
/// handles in a shared [`BddBuilder`](crate::bdd::BddBuilder) and use
/// [`Bdd::equivalent_to`](crate::bdd::Bdd::equivalent_to), which is an O(1) canonical comparison.
///
/// # The stored label type `S`
///
/// The variable names are stored as the label type `S`, which defaults to [`Symbol`] and is bounded by
/// [`StringLabel`] (so `String`, `Arc<str>`, … also qualify). `S` is a genuine generic with a type-level
/// default: the constructors ([`var`](Self::var), [`constant`](Self::constant), [`build`](Self::build),
/// [`Default`]) and the [`expr!`](crate::expr) macro produce whichever `S` the binding, turbofish, or
/// consuming context fixes, falling back to `Symbol` only where an annotation pins it there. To
/// construct a `BoolExpr<S>` with a specific `S`, annotate the binding, turbofish the constructor
/// (`BoolExpr::<String>::var("a")`), or parse into it (`"a & b".parse::<BoolExpr<String>>()`).
///
/// # Internal representation
///
/// Backed by an `Arc<[Token]>` reverse-Polish token stream, so [`Clone`] is a cheap reference-count
/// bump and composition concatenates token streams.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BoolExpr<S: StringLabel = Symbol> {
    /// The expression as a reverse-Polish token stream. `Arc<[Token]>` so cloning is a refcount bump
    /// and the structural `PartialEq`/`Eq`/`Hash` derive compares/hashes the token sequence.
    tokens: Arc<[Token<S>]>,
}

/// The constant `false` — the identity element for `|`/`^`, so it composes cleanly as a starting
/// accumulator.
impl<S: StringLabel> Default for BoolExpr<S> {
    fn default() -> Self {
        Self::constant(false)
    }
}

impl<S: StringLabel> BoolExpr<S> {
    /// Create a variable expression with the given name.
    ///
    /// The stored label type `S` follows the binding or turbofish
    /// (`BoolExpr::<String>::var("a")`), falling back to [`Symbol`] where an annotation pins it there
    /// (`let e: BoolExpr = BoolExpr::var("a")`).
    #[must_use]
    pub fn var<N: AsRef<str>>(name: N) -> Self {
        BoolExpr {
            tokens: Arc::from([Token::Var(S::from(name.as_ref()))]),
        }
    }

    /// Create a constant expression (`true` or `false`).
    #[must_use]
    pub fn constant(value: bool) -> Self {
        BoolExpr {
            tokens: Arc::from([Token::Const(value)]),
        }
    }

    /// Build from an owned token stream (the single internal constructor over raw tokens).
    pub(crate) fn from_tokens(tokens: Arc<[Token<S>]>) -> Self {
        BoolExpr { tokens }
    }

    /// The expression's reverse-Polish token stream (for sibling layers such as the BDD `build`).
    pub(crate) fn tokens(&self) -> &[Token<S>] {
        &self.tokens
    }

    /// The variables appearing syntactically in this expression, as a lazy [`ExprVariables`] iterator.
    ///
    /// This is a purely **syntactic** scan of the token stream: a variable is reported if it occurs in
    /// the expression's text, regardless of whether the function actually depends on it (e.g. `a & !a`
    /// still reports `a`). Each variable is yielded once (deduplicated) the first time it is seen, in
    /// token order — not sorted. For the semantic support of a function, build a [`Bdd`](crate::bdd::Bdd)
    /// and use [`Bdd::variables`](crate::bdd::Bdd::variables).
    #[must_use]
    pub fn variables(&self) -> ExprVariables<'_, S> {
        ExprVariables {
            tokens: self.tokens.iter(),
            seen: std::collections::HashSet::new(),
        }
    }

    /// Cast to a `BoolExpr<T>` with a different stored label type, preserving every variable name.
    ///
    /// Each variable name is re-interned into the target label type `T` (via its `&str` view) while the
    /// operator and constant structure is copied unchanged, so the result is syntactically identical —
    /// same variables, same tree — with only the stored label representation differing.
    ///
    /// Crate-internal and permanent: the Cover→expression bridge relies on it to re-key an extracted
    /// expression into a caller's label type. It is not a transitional shim.
    pub(crate) fn cast<T: StringLabel>(&self) -> BoolExpr<T> {
        let tokens: Arc<[Token<T>]> = self
            .tokens
            .iter()
            .map(|token| match token {
                Token::Var(name) => Token::Var(T::from(name.as_ref())),
                Token::Const(value) => Token::Const(*value),
                Token::Not => Token::Not,
                Token::And => Token::And,
                Token::Or => Token::Or,
                Token::Xor => Token::Xor,
            })
            .collect();
        BoolExpr::from_tokens(tokens)
    }
}

/// Lazy iterator over the variables appearing syntactically in a [`BoolExpr`], created by
/// [`BoolExpr::variables`].
///
/// Scans the reverse-Polish token stream, yielding each variable label `S` the first time it is seen
/// (deduplicated via a running seen-set) in token order — nothing is sorted or materialised up front.
pub struct ExprVariables<'a, S: StringLabel = Symbol> {
    tokens: std::slice::Iter<'a, Token<S>>,
    /// Deduplication is done on the `&str` view borrowed from the tokens, so no extra clone is needed
    /// to record a seen name and the set carries no `S` bound of its own.
    seen: std::collections::HashSet<&'a str>,
}

/// Opaque: the token cursor and seen-set carry no useful `Debug`.
impl<S: StringLabel> std::fmt::Debug for ExprVariables<'_, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExprVariables").finish_non_exhaustive()
    }
}

impl<'a, S: StringLabel> Iterator for ExprVariables<'a, S> {
    type Item = S;

    fn next(&mut self) -> Option<S> {
        for token in self.tokens.by_ref() {
            if let Token::Var(name) = token {
                if self.seen.insert(name.as_ref()) {
                    return Some(name.clone());
                }
            }
        }
        None
    }
}

// Once the token stream is exhausted the cursor stays exhausted, so `None` is terminal.
impl<S: StringLabel> std::iter::FusedIterator for ExprVariables<'_, S> {}

#[cfg(test)]
mod tests;
