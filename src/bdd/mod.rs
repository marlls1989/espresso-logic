//! Canonical Binary Decision Diagram layer.
//!
//! This module provides a builder-scoped, canonical BDD engine for Boolean functions. It is built
//! around three pieces:
//!
//! - a [`Brand`] — a sealed, zero-sized marker type that names one BDD namespace and selects, at the
//!   type level, whether its builder is single-threaded or thread-safe;
//! - a builder — [`BddBuilder`] (single-threaded) or [`SyncBddBuilder`] (`Send + Sync`) — that owns an
//!   independent BDD manager and hands out handles branded to it;
//! - a [`Bdd`] handle — a lightweight, `Copy` borrow into a builder denoting one canonical function.
//!
//! Each builder owns its own manager: there is no process-global state and no default brand. Because a
//! handle borrows its builder and carries the builder's brand as an invariant type parameter, two
//! handles can be combined only when they came from the *same* builder — combining handles from two
//! different builders is a compile error, and the borrow checker prevents a handle from outliving its
//! builder.
//!
//! # Canonicity
//!
//! Within one builder every Boolean function has exactly one root node, so equivalent functions are
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
//! A builder is minted by the [`bdd_builder!`](crate::bdd_builder) (single-threaded) or
//! [`sync_bdd_builder!`](crate::sync_bdd_builder) (thread-safe) macro, each of which mints a fresh
//! brand per call so handles of two different builders can never be combined. A [`BoolExpr`] is built
//! into a handle with [`BddBuilder::build`] / [`BddBuilder::parse`], and a handle is lowered back to a
//! factored [`BoolExpr`] with [`Bdd::to_expr`].
//!
//! [`BoolExpr`]: crate::BoolExpr

mod brand;
mod builder;
mod handle;

pub use brand::Brand;
pub use builder::{BddBuilder, SyncBddBuilder};
pub use handle::{Bdd, BddNode};

/// Items the `bdd_builder!` / `sync_bdd_builder!` macros need to name at their (possibly downstream)
/// call sites. Not part of the documented public API; named only by those macros.
#[doc(hidden)]
pub mod __macro_support {
    pub use crate::expression::manager_cell::{LocalCell, ManagerCell, SyncCell};
    pub use super::brand::brand_seal::Sealed;
}

#[cfg(test)]
mod tests;
