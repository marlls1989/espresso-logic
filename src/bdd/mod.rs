//! Canonical Binary Decision Diagram layer.
//!
//! This module provides a builder-scoped, canonical BDD engine for Boolean functions. It is built
//! around three pieces:
//!
//! - a [`Brand`] — a sealed, zero-sized marker type that names one BDD namespace for uniqueness;
//! - a [`ManagerCell`] — the storage backend, [`LocalCell`] (single-threaded) or [`SyncCell`]
//!   (`Send + Sync`);
//! - a builder — [`BddBuilder`], parameterised by both, that owns an independent BDD manager and hands out
//!   handles branded to it;
//! - a [`Bdd`] handle — a lightweight, refcounted (`Clone`, not `Copy`) owner of a clone of the builder's
//!   manager, denoting one canonical function.
//!
//! The brand and the storage backend are orthogonal: the brand selects no behaviour (it only keeps handles
//! of two builders from unifying), while the cell alone determines thread-safety. Each builder owns its own
//! manager: there is no process-global state and no default brand. Because a handle carries the builder's
//! brand as an invariant type parameter, two handles can be combined only when they came from the *same*
//! builder — combining handles of two different brands is a compile error. A handle holds a refcounted
//! clone of the manager, so it can outlive the builder.
//!
//! # Canonicity
//!
//! Within one builder every Boolean function has exactly one root node, so equivalent functions are
//! *identical* handles: [`Bdd::equivalent_to`] is an O(1) root-id comparison, and the operators keep
//! every result canonical via hash-consing.
//!
//! # What this layer offers
//!
//! - Boolean operators by value and by reference: `&` (AND), `|` (OR), `^` (XOR), `!` (NOT) — the
//!   last also available as the named [`Bdd::complement`] / [`Bdd::not`] aliases — plus [`Bdd::ite`].
//! - Shannon cofactor / quantification: [`Bdd::restrict`] / [`Bdd::cofactor`], [`Bdd::forall`],
//!   [`Bdd::exists`].
//! - Constant queries: [`Bdd::is_tautology`], [`Bdd::is_contradiction`].
//! - Materialisation: [`Bdd::to_cubes`] (a single-output sum-of-products cover), [`Bdd::maximize`]
//!   (the fully-expanded maximal cover over an explicit, widenable variable set), and
//!   [`Bdd::minimize`].
//! - Introspection: [`Bdd::variables`], [`Bdd::node_count`], [`Bdd::var_count`].
//!
//! # Construction
//!
//! A builder is minted by the [`bdd_builder!`](crate::bdd_builder) (single-threaded) or
//! [`sync_bdd_builder!`](crate::sync_bdd_builder) (thread-safe) macro, each of which mints a fresh brand
//! per call — paired with [`LocalCell`] or [`SyncCell`] respectively — so handles of two different builders
//! can never be combined. A [`BoolExpr`] is built into a handle with [`BddBuilder::build`] /
//! [`BddBuilder::parse`], and a handle is lowered back to a factored [`BoolExpr`] with [`Bdd::to_expr`].
//! For allocation-free composition without `.clone()`, [`BddBuilder::scope`] hands a closure a [`Scope`]
//! of `Copy`, by-reference [`ScopedBdd`] handles and returns the owned [`Bdd`] for the root.
//!
//! [`BoolExpr`]: crate::BoolExpr

mod brand;
mod builder;
mod encoding;
mod handle;
mod scope;

pub use crate::expression::manager_cell::{LocalCell, ManagerCell, SyncCell};
pub use brand::Brand;
pub use builder::BddBuilder;
pub use handle::{Bdd, BddNode, BddVariables};
pub use scope::{Scope, ScopedBdd};

/// Items the `bdd_builder!` / `sync_bdd_builder!` macros need to name at their (possibly downstream)
/// call sites. Not part of the documented public API; named only by those macros.
#[doc(hidden)]
pub mod __macro_support {
    pub use super::brand::brand_seal::Sealed;
    pub use crate::expression::manager_cell::{LocalCell, ManagerCell, SyncCell};
}

#[cfg(test)]
mod tests;
