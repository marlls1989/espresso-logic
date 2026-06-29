//! The reverse-Polish (postfix) token stream backing [`BoolExpr`](super::BoolExpr).
//!
//! A [`BoolExpr`](super::BoolExpr) is an owned, syntactic value: a flat sequence of [`Token`]s in
//! reverse-Polish order. This module owns that representation and the two operations the rest of the
//! expression layer needs over it:
//!
//! - [`evaluate`] — a stack fold that interprets the token stream under a variable assignment.
//! - [`binary`] / [`unary`] — composition helpers that concatenate operand token streams and push an
//!   operator, the mechanism every [operator](super::operators) and the factoriser build with.
//!
//! Both are iterative (an explicit value stack, no recursion), so an arbitrarily deep expression can be
//! evaluated or composed without overflowing the call stack.

use crate::Symbol;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
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

/// Evaluate a token stream under a variable `assignment`.
///
/// Interpreted iteratively with an explicit value stack, so a long operator chain or deep nesting
/// cannot overflow the call stack. **A variable absent from `assignment` reads as `false`** (partial
/// assignments are allowed). The key type can be any `Borrow<str>` (`&str`, `String`, `Symbol`,
/// `Arc<str>`, …).
///
/// The stream is well-formed by construction (constructors and the parser only ever produce balanced
/// postfix), so the value stack neither underflows nor ends non-singleton.
pub(crate) fn evaluate<K>(tokens: &[Token], assignment: &HashMap<K, bool>) -> bool
where
    K: Borrow<str> + Eq + Hash,
{
    let mut stack: Vec<bool> = Vec::with_capacity(tokens.len());
    for token in tokens {
        let value = match token {
            Token::Var(name) => assignment.get(name.as_str()).copied().unwrap_or(false),
            Token::Const(value) => *value,
            Token::Not => {
                let a = stack.pop().expect("rpn evaluate: underflow on NOT");
                !a
            }
            Token::And => {
                let b = stack.pop().expect("rpn evaluate: underflow on AND");
                let a = stack.pop().expect("rpn evaluate: underflow on AND");
                a && b
            }
            Token::Or => {
                let b = stack.pop().expect("rpn evaluate: underflow on OR");
                let a = stack.pop().expect("rpn evaluate: underflow on OR");
                a || b
            }
            Token::Xor => {
                let b = stack.pop().expect("rpn evaluate: underflow on XOR");
                let a = stack.pop().expect("rpn evaluate: underflow on XOR");
                a ^ b
            }
        };
        stack.push(value);
    }
    stack.pop().expect("rpn evaluate: empty token stream")
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
