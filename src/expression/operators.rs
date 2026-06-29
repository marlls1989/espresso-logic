//! Bitwise operator overloading and Boolean operations for [`BoolExpr`].
//!
//! The owned, syntactic [`BoolExpr`] composes through the bitwise operators — `&` (AND), `|` (OR),
//! `^` (XOR), `!` (NOT). Each is provided by value **and** by reference, so `a & b`, `&a & b`,
//! `a & &b` and `&a & &b` all type-check (the by-reference forms avoid moving the operands). Every
//! operator concatenates the operands' reverse-Polish token streams and appends the operator (see
//! [`rpn`](super::rpn)); the result is a new syntactic expression, never canonicalised.

use super::rpn::{self, Token};
use super::BoolExpr;
use std::ops::{BitAnd, BitOr, BitXor, Not};

// Boolean operation methods (the by-value/by-ref operator impls all delegate here).
impl BoolExpr {
    /// Logical AND: a new expression that is the conjunction of `self` and `other`.
    #[must_use]
    pub fn and(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr::from_tokens(rpn::binary(Token::And, self.tokens(), other.tokens()))
    }

    /// Logical OR: a new expression that is the disjunction of `self` and `other`.
    #[must_use]
    pub fn or(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr::from_tokens(rpn::binary(Token::Or, self.tokens(), other.tokens()))
    }

    /// Logical XOR: a new expression that is the exclusive-or of `self` and `other`.
    #[must_use]
    pub fn xor(&self, other: &BoolExpr) -> BoolExpr {
        BoolExpr::from_tokens(rpn::binary(Token::Xor, self.tokens(), other.tokens()))
    }

    /// Logical NOT: a new expression that is the negation of `self`.
    #[must_use]
    pub fn not(&self) -> BoolExpr {
        BoolExpr::from_tokens(rpn::unary_not(self.tokens()))
    }
}

/// Implement a binary bitwise operator for every owned/borrowed combination of operands, all
/// delegating to the named [`BoolExpr`] method.
macro_rules! bin_op {
    ($trait:ident, $method:ident, $call:ident) => {
        impl $trait for BoolExpr {
            type Output = BoolExpr;
            fn $method(self, rhs: BoolExpr) -> BoolExpr {
                BoolExpr::$call(&self, &rhs)
            }
        }
        impl $trait<&BoolExpr> for BoolExpr {
            type Output = BoolExpr;
            fn $method(self, rhs: &BoolExpr) -> BoolExpr {
                BoolExpr::$call(&self, rhs)
            }
        }
        impl $trait<BoolExpr> for &BoolExpr {
            type Output = BoolExpr;
            fn $method(self, rhs: BoolExpr) -> BoolExpr {
                BoolExpr::$call(self, &rhs)
            }
        }
        impl $trait<&BoolExpr> for &BoolExpr {
            type Output = BoolExpr;
            fn $method(self, rhs: &BoolExpr) -> BoolExpr {
                BoolExpr::$call(self, rhs)
            }
        }
    };
}

bin_op!(BitAnd, bitand, and);
bin_op!(BitOr, bitor, or);
bin_op!(BitXor, bitxor, xor);

impl Not for BoolExpr {
    type Output = BoolExpr;
    fn not(self) -> BoolExpr {
        BoolExpr::not(&self)
    }
}

impl Not for &BoolExpr {
    type Output = BoolExpr;
    fn not(self) -> BoolExpr {
        BoolExpr::not(self)
    }
}
