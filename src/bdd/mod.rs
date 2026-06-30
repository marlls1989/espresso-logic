//! Canonical Binary Decision Diagram layer.
//!
//! This module provides a context-scoped, canonical BDD engine for Boolean functions. It is built
//! around three pieces:
//!
//! - a [`Brand`] — a sealed, zero-sized marker type that names one BDD namespace and selects, at the
//!   type level, whether its context is single-threaded or thread-safe;
//! - a context — [`BddContext`] (single-threaded) or [`SyncBddContext`] (`Send + Sync`) — that owns an
//!   independent BDD manager and hands out handles branded to it;
//! - a [`Bdd`] handle — a lightweight, `Copy` borrow into a context denoting one canonical function.
//!
//! Each context owns its own manager: there is no process-global state and no default brand. Because a
//! handle borrows its context and carries the context's brand as an invariant type parameter, two
//! handles can be combined only when they came from the *same* context — combining handles from two
//! different contexts is a compile error, and the borrow checker prevents a handle from outliving its
//! context.
//!
//! # Canonicity
//!
//! Within one context every Boolean function has exactly one root node, so equivalent functions are
//! *identical* handles: [`Bdd::equivalent_to`] is an O(1) root-id comparison, and the operators keep
//! every result canonical via hash-consing.
//!
//! # What this layer offers
//!
//! - Boolean operators by value and by reference: `&` (AND), `|` (OR), `^` (XOR), `!` (NOT), plus
//!   [`Bdd::ite`].
//! - Shannon cofactor / quantification: [`Bdd::restrict`] / [`Bdd::cofactor`], [`Bdd::forall`],
//!   [`Bdd::exists`].
//! - Constant queries: [`Bdd::is_tautology`], [`Bdd::is_contradiction`].
//! - Materialisation: [`Bdd::to_cubes`] (a single-output sum-of-products cover), [`Bdd::to_minterms`]
//!   (fully-expanded minterms over an explicit, widenable variable set), and [`Bdd::minimize`].
//! - Introspection: [`Bdd::collect_variables`], [`Bdd::node_count`], [`Bdd::var_count`].
//!
//! # Construction
//!
//! A context is minted by the [`bdd_context!`](crate::bdd_context) (single-threaded) or
//! [`sync_bdd_context!`](crate::sync_bdd_context) (thread-safe) macro, each of which mints a fresh
//! brand per call so handles of two different contexts can never be combined. A [`BoolExpr`] is built
//! into a handle with [`BddContext::build`] / [`BddContext::parse`], and a handle is lowered back to a
//! factored [`BoolExpr`] with [`Bdd::to_expr`].
//!
//! [`BoolExpr`]: crate::BoolExpr

mod brand;
mod context;
mod handle;

pub use brand::Brand;
pub use context::{BddContext, SyncBddContext};
pub use handle::{Bdd, BddNode};

/// Items the `bdd_context!` / `sync_bdd_context!` macros need to name at their (possibly downstream)
/// call sites. Not part of the documented public API; named only by those macros.
#[doc(hidden)]
pub mod __macro_support {
    pub use crate::expression::manager_cell::{LocalCell, ManagerCell, SyncCell};
    pub use super::brand::brand_seal::Sealed;
}

#[cfg(test)]
mod tests;
