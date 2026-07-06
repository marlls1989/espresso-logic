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

use crate::Symbol;
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
/// # Internal representation
///
/// Backed by an `Arc<[Token]>` reverse-Polish token stream, so [`Clone`] is a cheap reference-count
/// bump and composition concatenates token streams.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BoolExpr {
    /// The expression as a reverse-Polish token stream. `Arc<[Token]>` so cloning is a refcount bump
    /// and the structural `PartialEq`/`Eq`/`Hash` derive compares/hashes the token sequence.
    tokens: Arc<[Token]>,
}

/// The constant `false` — the identity element for `|`/`^`, so it composes cleanly as a starting
/// accumulator.
impl Default for BoolExpr {
    fn default() -> Self {
        BoolExpr::constant(false)
    }
}

impl BoolExpr {
    /// Create a variable expression with the given name.
    #[must_use]
    pub fn var<S: AsRef<str>>(name: S) -> Self {
        BoolExpr {
            tokens: Arc::from([Token::Var(Symbol::from(name.as_ref()))]),
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
    pub(crate) fn from_tokens(tokens: Arc<[Token]>) -> Self {
        BoolExpr { tokens }
    }

    /// The expression's reverse-Polish token stream (for sibling layers such as the BDD `build`).
    pub(crate) fn tokens(&self) -> &[Token] {
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
    pub fn variables(&self) -> ExprVariables<'_> {
        ExprVariables {
            tokens: self.tokens.iter(),
            seen: std::collections::HashSet::new(),
        }
    }
}

/// Lazy iterator over the variables appearing syntactically in a [`BoolExpr`], created by
/// [`BoolExpr::variables`].
///
/// Scans the reverse-Polish token stream, yielding each variable [`Symbol`] the first time it is seen
/// (deduplicated via a running seen-set) in token order — nothing is sorted or materialised up front.
pub struct ExprVariables<'a> {
    tokens: std::slice::Iter<'a, Token>,
    seen: std::collections::HashSet<Symbol>,
}

/// Opaque: the token cursor and seen-set carry no useful `Debug`.
impl std::fmt::Debug for ExprVariables<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExprVariables").finish_non_exhaustive()
    }
}

impl Iterator for ExprVariables<'_> {
    type Item = Symbol;

    fn next(&mut self) -> Option<Symbol> {
        for token in self.tokens.by_ref() {
            if let Token::Var(name) = token {
                if self.seen.insert(name.clone()) {
                    return Some(name.clone());
                }
            }
        }
        None
    }
}

// Once the token stream is exhausted the cursor stays exhausted, so `None` is terminal.
impl std::iter::FusedIterator for ExprVariables<'_> {}

#[cfg(test)]
mod tests;
