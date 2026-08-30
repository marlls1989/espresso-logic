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
mod syntax;

pub use error::{ExpressionParseError, ParseBoolExprError};

// Re-export AST types
pub(crate) use ast::BoolExprAst;
pub use ast::ExprNode;

// The auxiliary builder behind `BoolExpr::build`.
pub use builder::{Expr, ExprBuilder};

// The surface syntax an expression is spelled in.
pub use syntax::{LibertySyntax, StdSyntax, Syntax, VerilogSyntax};

use crate::Symbol;
use rpn::Token;

use std::marker::PhantomData;
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
/// Comparison is also within one syntax: `==` relates two expressions of the *same* `S`, and comparing
/// across syntaxes is a compile error rather than a silent `false`. Relabel one side with
/// [`as_syntax`](Self::as_syntax) to compare the token structure of expressions spelled differently.
///
/// # Internal representation
///
/// Backed by an `Arc<[Token]>` reverse-Polish token stream, so [`Clone`] is a cheap reference-count
/// bump and composition concatenates token streams. The syntax parameter is a [`PhantomData`] marker
/// with no runtime footprint.
pub struct BoolExpr<S: Syntax = StdSyntax> {
    /// The expression as a reverse-Polish token stream. `Arc<[Token]>` so cloning is a refcount bump
    /// and the hand-written `PartialEq`/`Eq`/`Hash` compare/hash the token sequence.
    tokens: Arc<[Token]>,
    /// The surface syntax this expression is spelled in — a marker only; no syntax value is ever
    /// stored. `fn() -> S` rather than `S` so the expression is [`Send`]/[`Sync`] regardless of `S`,
    /// and so the parameter stays covariant.
    _syntax: PhantomData<fn() -> S>,
}

// `Clone`/`PartialEq`/`Eq`/`Hash` are written out rather than derived: a derive would bound them on
// `S: Clone`/`S: PartialEq`/…, which a marker type has no business having to satisfy. All four operate
// on the token stream alone, exactly as the derives did before the syntax parameter existed.

/// A cheap reference-count bump on the shared token stream.
impl<S: Syntax> Clone for BoolExpr<S> {
    fn clone(&self) -> Self {
        Self::from_tokens(Arc::clone(&self.tokens))
    }
}

/// Structural equality over the token stream — see the type-level docs. `PartialEq<Self>` only, so
/// comparing two expressions of different syntaxes does not compile.
impl<S: Syntax> PartialEq for BoolExpr<S> {
    fn eq(&self, other: &Self) -> bool {
        self.tokens == other.tokens
    }
}

impl<S: Syntax> Eq for BoolExpr<S> {}

/// Hashes the token stream, consistent with the structural [`PartialEq`].
impl<S: Syntax> std::hash::Hash for BoolExpr<S> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.tokens.hash(state);
    }
}

/// The constant `false` — the identity element for `|`/`^`, so it composes cleanly as a starting
/// accumulator.
impl<S: Syntax> Default for BoolExpr<S> {
    fn default() -> Self {
        // Built from tokens rather than through `constant`, which originates in the standard syntax
        // only; the constant `false` is the same single token whichever syntax spells it.
        Self::from_tokens(Arc::from([Token::Const(false)]))
    }
}

// Origination is deliberately concrete: `var`, `constant`, `parse` and `build` all sit on a bare
// `impl BoolExpr` block, which is `impl BoolExpr<StdSyntax>` through the type-position default.
//
// Type parameter defaults do not participate in inference on stable Rust (checked against rustc
// 1.96.0). Making these constructors generic over the syntax therefore leaves every existing
// unannotated call site ambiguous — error[E0283] `type annotations needed` — and a second inherent
// `parse` on another instantiation makes the method name ambiguous — error[E0034] `multiple applicable
// items in scope`. So no per-syntax inherent constructor is added: an expression in another syntax
// comes from text through the `FromStr` impl, `text.parse::<BoolExpr<OtherSyntax>>()`, and from an
// existing expression through `as_syntax`.
impl BoolExpr {
    /// Create a variable expression with the given name.
    #[must_use]
    pub fn var<S: AsRef<str>>(name: S) -> Self {
        BoolExpr::from_tokens(Arc::from([Token::Var(Symbol::from(name.as_ref()))]))
    }

    /// Create a constant expression (`true` or `false`).
    #[must_use]
    pub fn constant(value: bool) -> Self {
        BoolExpr::from_tokens(Arc::from([Token::Const(value)]))
    }
}

impl<S: Syntax> BoolExpr<S> {
    /// Build from an owned token stream (the single internal constructor over raw tokens, and the one
    /// place the syntax marker is written).
    pub(crate) fn from_tokens(tokens: Arc<[Token]>) -> Self {
        BoolExpr {
            tokens,
            _syntax: PhantomData,
        }
    }

    /// The expression's reverse-Polish token stream (for sibling layers such as the BDD `build`).
    pub(crate) fn tokens(&self) -> &[Token] {
        self.tokens_arc()
    }

    /// The token stream's `Arc` handle, for consumers that keep the stream rather than read it: a graft
    /// into a builder bumps the reference count instead of copying the tokens.
    pub(crate) fn tokens_arc(&self) -> &Arc<[Token]> {
        &self.tokens
    }

    /// Reinterpret this expression under another surface syntax.
    ///
    /// Infallible and total: the token stream is the same canonical operator set whichever syntax
    /// spells it, so this is a relabelling — a reference-count bump on the shared tokens, with no
    /// re-parse and no rewrite. Only the spellings and precedence used to [`Display`](std::fmt::Display)
    /// the expression change.
    ///
    /// This is not a [`From`] impl: a blanket `From<BoolExpr<S>> for BoolExpr<T>` would overlap the
    /// reflexive `From<T> for T` in `core` and is rejected for it.
    ///
    /// # Examples
    ///
    /// Equality relates expressions of one syntax, so the retag is what brings two differently spelled
    /// expressions onto common ground:
    ///
    /// ```
    /// use espresso_logic::{BoolExpr, VerilogSyntax};
    ///
    /// # fn main() -> Result<(), espresso_logic::expression::ParseBoolExprError> {
    /// let standard = BoolExpr::parse("a & b | !c")?;
    /// let verilog = "a & b | ~c".parse::<BoolExpr<VerilogSyntax>>()?;
    ///
    /// assert_eq!(standard.as_syntax::<VerilogSyntax>(), verilog);
    /// assert_eq!(verilog.to_string(), "a & b | ~c");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Comparing across syntaxes without the retag is a compile error rather than a silent `false`:
    ///
    /// ```compile_fail
    /// use espresso_logic::{BoolExpr, VerilogSyntax};
    ///
    /// let standard = BoolExpr::parse("a & b").unwrap();
    /// let verilog = "a & b".parse::<BoolExpr<VerilogSyntax>>().unwrap();
    ///
    /// // error[E0308]: mismatched types — `PartialEq` relates one syntax to itself only.
    /// let _ = standard == verilog;
    /// ```
    #[must_use]
    pub fn as_syntax<T: Syntax>(&self) -> BoolExpr<T> {
        BoolExpr::from_tokens(Arc::clone(&self.tokens))
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
mod syntax_tests;
#[cfg(test)]
mod tests;
