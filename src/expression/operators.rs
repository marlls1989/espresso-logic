//! Bitwise operator overloading and Boolean operations for [`BoolExpr`].
//!
//! The owned, syntactic [`BoolExpr`] composes through the bitwise operators — `&` (AND), `|` (OR),
//! `^` (XOR), `!` (NOT). Each is provided by value **and** by reference, so `a & b`, `&a & b`,
//! `a & &b` and `&a & &b` all type-check (the by-reference forms avoid moving the operands). Every
//! operator concatenates the operands' reverse-Polish token streams and appends the operator (see
//! [`rpn`](super::rpn)); the result is a new syntactic expression, never canonicalised.

use super::rpn::{self, Token};
use super::BoolExpr;
use crate::{impl_binary_operator, StringLabel};
use std::ops::Not;

// Boolean operation methods (the by-value/by-ref operator impls all delegate here).
impl<S: StringLabel> BoolExpr<S> {
    /// Logical AND: a new expression that is the conjunction of `self` and `other`. Equivalent to the
    /// `&` operator.
    #[must_use]
    pub fn and(&self, other: &BoolExpr<S>) -> BoolExpr<S> {
        BoolExpr::from_tokens(rpn::binary(Token::And, self.tokens(), other.tokens()))
    }

    /// Logical OR: a new expression that is the disjunction of `self` and `other`. Equivalent to the
    /// `|` operator.
    #[must_use]
    pub fn or(&self, other: &BoolExpr<S>) -> BoolExpr<S> {
        BoolExpr::from_tokens(rpn::binary(Token::Or, self.tokens(), other.tokens()))
    }

    /// Logical XOR: a new expression that is the exclusive-or of `self` and `other`. Equivalent to the
    /// `^` operator.
    #[must_use]
    pub fn xor(&self, other: &BoolExpr<S>) -> BoolExpr<S> {
        BoolExpr::from_tokens(rpn::binary(Token::Xor, self.tokens(), other.tokens()))
    }

    /// Logical NOT: a new expression that is the negation of `self`. Equivalent to the unary `!`
    /// operator.
    #[must_use]
    pub fn not(&self) -> BoolExpr<S> {
        BoolExpr::from_tokens(rpn::unary_not(self.tokens()))
    }
}

// Implement each binary bitwise operator for every owned/borrowed combination of operands, all
// delegating to the named `&self, &Self` [`BoolExpr`] method, via the shared `impl_binary_operator!`
// macro (its leading group carries the stored-label generic parameter).
impl_binary_operator!({S: StringLabel} BoolExpr<S>, BitAnd, bitand, and);
impl_binary_operator!({S: StringLabel} BoolExpr<S>, BitOr, bitor, or);
impl_binary_operator!({S: StringLabel} BoolExpr<S>, BitXor, bitxor, xor);

impl<S: StringLabel> Not for BoolExpr<S> {
    type Output = BoolExpr<S>;
    fn not(self) -> BoolExpr<S> {
        BoolExpr::not(&self)
    }
}

impl<S: StringLabel> Not for &BoolExpr<S> {
    type Output = BoolExpr<S>;
    fn not(self) -> BoolExpr<S> {
        BoolExpr::not(self)
    }
}
