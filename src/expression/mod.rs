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
//! - [`BoolExpr::var`] / [`BoolExpr::constant`] — leaves.
//! - The bitwise operators `&` (AND), `|` (OR), `^` (XOR), `!` (NOT), by value and by reference.
//! - [`BoolExpr::parse`] / [`str::parse`] — from text.
//!
//! ```
//! use espresso_logic::BoolExpr;
//!
//! let f = BoolExpr::var("a") & BoolExpr::var("b") | !BoolExpr::var("c");
//! let g = BoolExpr::parse("a & b | !c").unwrap();
//! // Structural equality: the same syntactic tree.
//! assert_eq!(f, g);
//! ```

// Submodules
mod ast;
mod display;
pub mod error;
pub(crate) mod factorization;
pub(crate) mod manager;
pub(crate) mod manager_cell;
mod operators;
mod parser;
pub(crate) mod rpn;

pub use error::{ExpressionParseError, ParseBoolExprError};

// Re-export AST types
pub(crate) use ast::BoolExprAst;
pub use ast::ExprNode;

use crate::Symbol;
use rpn::Token;

use std::collections::BTreeSet;
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

    /// The variables appearing syntactically in this expression, in canonical (sorted) order.
    ///
    /// This is a purely **syntactic** scan of the token stream: a variable is reported if it occurs in
    /// the expression's text, regardless of whether the function actually depends on it (e.g. `a & !a`
    /// still reports `a`). For the semantic support of a function, build a [`Bdd`](crate::bdd::Bdd) and
    /// use [`Bdd::collect_variables`](crate::bdd::Bdd::collect_variables).
    #[must_use]
    pub fn variables(&self) -> BTreeSet<Symbol> {
        self.tokens
            .iter()
            .filter_map(|t| match t {
                Token::Var(name) => Some(name.clone()),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
