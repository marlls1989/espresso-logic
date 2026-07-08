//! The reverse-Polish (postfix) token stream backing [`BoolExpr`](super::BoolExpr).
//!
//! A [`BoolExpr`](super::BoolExpr) is an owned, syntactic value: a flat sequence of [`Token`]s in
//! reverse-Polish order. This module owns that representation and the two operations the rest of the
//! expression layer needs over it:
//!
//! - [`binary`] / [`unary_not`] — composition helpers that concatenate operand token streams and push
//!   an operator, the mechanism every [operator](super::operators) and the factoriser build with.
//! - [`fold_postfix`] — the one postfix-token interpreter: a value-stack fold over the tokens,
//!   parameterised by the per-token actions. Every consumer that walks a token stream left to right
//!   (the BDD builder, the expression-tree fold, the display renderer) routes through it, so the
//!   stack discipline and the iterative (non-recursive) traversal live in a single place.
//!
//! Composition is plain concatenation, so an arbitrarily deep expression is built without recursion.

use crate::Symbol;
use std::sync::Arc;

/// One step of a reverse-Polish Boolean expression program.
///
/// The variable operand carries a label `S` — the expression layer's interned name type
/// [`Symbol`] by default, not a raw `String`. There is a single canonical operator set —
/// `&`/`|`/`^`/`!` (AND/OR/XOR/NOT) — even though the text parser additionally accepts the
/// `*`/`+`/`~` spellings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Token<S = Symbol> {
    /// Push a variable by name.
    Var(S),
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
pub(crate) fn binary<S: Clone>(
    op: Token<S>,
    left: &[Token<S>],
    right: &[Token<S>],
) -> Arc<[Token<S>]> {
    debug_assert!(matches!(op, Token::And | Token::Or | Token::Xor));
    let mut tokens = Vec::with_capacity(left.len() + right.len() + 1);
    tokens.extend_from_slice(left);
    tokens.extend_from_slice(right);
    tokens.push(op);
    tokens.into()
}

/// Compose a unary negation: `child ++ [Not]`.
pub(crate) fn unary_not<S: Clone>(child: &[Token<S>]) -> Arc<[Token<S>]> {
    let mut tokens = Vec::with_capacity(child.len() + 1);
    tokens.extend_from_slice(child);
    tokens.push(Token::Not);
    tokens.into()
}

/// Interpret a reverse-Polish [`Token`] program as a value-stack fold.
///
/// Each token drives one action over a stack of `T` values: a [`Var`](Token::Var) or
/// [`Const`](Token::Const) pushes a leaf, a unary [`Not`](Token::Not) replaces the top, and a binary
/// operator pops its right then left operand and pushes their combination (the combinators therefore
/// receive `(left, right)` in source order). The walk is iterative — an explicit value stack — so an
/// arbitrarily deep program cannot overflow the call stack. A well-formed program leaves exactly one
/// value, which is returned; an empty or unbalanced program is a bug in the token stream and panics.
///
/// This is the single token-stream interpreter: the BDD builder folds tokens into [`Bdd`] handles, the
/// expression-tree fold into [`ExprNode`] results, and the display renderer into `(text, precedence)`
/// pairs, all by supplying the per-token actions here.
///
/// [`Bdd`]: crate::bdd::Bdd
/// [`ExprNode`]: super::ExprNode
pub(crate) fn fold_postfix<S, T>(
    tokens: &[Token<S>],
    mut var: impl FnMut(&S) -> T,
    mut constant: impl FnMut(bool) -> T,
    mut not: impl FnMut(T) -> T,
    mut and: impl FnMut(T, T) -> T,
    mut or: impl FnMut(T, T) -> T,
    mut xor: impl FnMut(T, T) -> T,
) -> T {
    let mut stack: Vec<T> = Vec::with_capacity(tokens.len());
    for token in tokens {
        let value = match token {
            Token::Var(name) => var(name),
            Token::Const(value) => constant(*value),
            Token::Not => {
                let a = stack.pop().expect("postfix underflow on NOT");
                not(a)
            }
            Token::And => {
                let r = stack.pop().expect("postfix underflow on AND");
                let l = stack.pop().expect("postfix underflow on AND");
                and(l, r)
            }
            Token::Or => {
                let r = stack.pop().expect("postfix underflow on OR");
                let l = stack.pop().expect("postfix underflow on OR");
                or(l, r)
            }
            Token::Xor => {
                let r = stack.pop().expect("postfix underflow on XOR");
                let l = stack.pop().expect("postfix underflow on XOR");
                xor(l, r)
            }
        };
        stack.push(value);
    }
    stack.pop().expect("postfix program produced no result")
}
