//! The reverse-Polish (postfix) token stream backing [`BoolExpr`](super::BoolExpr).
//!
//! A [`BoolExpr`](super::BoolExpr) is an owned, syntactic value: a flat sequence of [`Token`]s in
//! reverse-Polish order. This module owns that representation and the two operations the rest of the
//! expression layer needs over it:
//!
//! - [`binary`] / [`unary_not`] — composition helpers that concatenate operand token streams and push
//!   an operator, the mechanism every [operator](super::operators) and the factoriser build with.
//!
//! Composition is plain concatenation, so an arbitrarily deep expression is built without recursion.

use crate::Symbol;
use std::sync::Arc;

/// One step of a reverse-Polish Boolean expression program.
///
/// The variable operand carries a [`Symbol`] (the expression layer's interned name type), not a raw
/// `String`. There is a single canonical operator set — `&`/`|`/`^`/`!` (AND/OR/XOR/NOT) — even though
/// the text parser additionally accepts the `*`/`+`/`~` spellings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Token {
    /// Push a variable by name.
    Var(Symbol),
    /// Push a constant.
    Const(bool),
    /// Pop one operand, push its negation.
    Not,
    /// Pop two operands, push their conjunction.
    And,
    /// Pop two operands, push their disjunction.
    Or,
    /// Pop two operands, push their exclusive-or.
    Xor,
}

/// Compose a binary operator: `concat(left, right) ++ [op]`.
///
/// Concatenating two postfix programs and appending the operator yields the postfix program of the
/// combined expression — the algebraic basis of all the binary [operators](super::operators).
pub(crate) fn binary(op: Token, left: &[Token], right: &[Token]) -> Arc<[Token]> {
    debug_assert!(matches!(op, Token::And | Token::Or | Token::Xor));
    let mut tokens = Vec::with_capacity(left.len() + right.len() + 1);
    tokens.extend_from_slice(left);
    tokens.extend_from_slice(right);
    tokens.push(op);
    tokens.into()
}

/// Compose a unary negation: `child ++ [Not]`.
pub(crate) fn unary_not(child: &[Token]) -> Arc<[Token]> {
    let mut tokens = Vec::with_capacity(child.len() + 1);
    tokens.extend_from_slice(child);
    tokens.push(Token::Not);
    tokens.into()
}
