//! Canonical Binary Decision Diagram layer.
//!
//! This module provides a builder-scoped, canonical BDD engine for Boolean functions. It is built
//! around three pieces:
//!
//! - a [`Brand`] — a sealed, zero-sized marker type that names one BDD namespace for uniqueness;
//! - a [`ManagerCell`] — the storage backend, [`LocalCell`] (single-threaded) or [`SyncCell`]
//!   (`Send + Sync`), itself generic over the stored label type through its
//!   [`Label`](ManagerCell::Label) associated type;
//! - a builder — [`BddBuilder`], parameterised by the brand and the cell, that owns an independent BDD
//!   manager and hands out handles branded to it;
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
//! - Shannon cofactor / quantification: [`Bdd::restrict`] / [`Bdd::cofactor`],
//!   [`Bdd::restrict_many`] (simultaneous multi-variable cofactor), [`Bdd::restrict_to`] (restrict to
//!   the subspace pinned by a [`Minterm`](crate::Minterm)), [`Bdd::forall`], [`Bdd::exists`]
//!   (restrict/restrict_many/restrict_to are mirrored on [`ScopedBdd`]).
//! - Composition: [`Bdd::compose`] (substitute a function for a variable) and
//!   [`Bdd::compose_map`] (simultaneous multi-variable substitution) — both mirrored on
//!   [`ScopedBdd`] — plus [`Composer`], which streams one substitution across a whole iterator of
//!   handles, sharing a single short-lived cache so overlapping functions are composed once.
//! - Constant queries: [`Bdd::is_tautology`], [`Bdd::is_contradiction`].
//! - Materialisation: [`Bdd::cover`] (a single-output sum-of-products cover), [`Bdd::maximize`]
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
//! # Label types
//!
//! [`Bdd`], [`BddBuilder`] and [`Scope`] are parameterised by only the [`Brand`] and the
//! [`ManagerCell`]; the stored label type lives on the cell itself, as its
//! [`Label`](ManagerCell::Label) associated type (bounded by [`StringLabel`](crate::StringLabel),
//! defaulting to [`Symbol`](crate::Symbol) on both [`LocalCell`] and [`SyncCell`]). Variable names are
//! genuinely stored as that type — not interned as `Symbol` and re-presented — so every label-producing
//! output ([`Bdd::variables`], the `cover`/`primes`/`maximize`/`minimize` families, [`Bdd::to_expr`])
//! comes back as `C::Label` directly, with no relabelling step.
//!
//! The `bdd_builder!` / `sync_bdd_builder!` macros leave the cell's label parameter as an inference
//! placeholder (`LocalCell<_>` / `SyncCell<_>`), so it resolves like any other unconstrained type
//! parameter: from a binding annotation (`let b: BddBuilder<_, LocalCell> = bdd_builder!();` picks
//! `Symbol` via the cell's own default; `let b: BddBuilder<_, LocalCell<String>> = bdd_builder!();`
//! picks another [`StringLabel`](crate::StringLabel)), or from consuming a labelled output downstream. A builder whose
//! labels are never pinned either way needs the one-time annotation. There is no `relabel` — a builder
//! or handle's label type is fixed for its lifetime once resolved.
//!
//! [`BoolExpr`]: crate::BoolExpr

mod batch;
mod brand;
mod builder;
mod encoding;
mod handle;
pub(crate) mod manager;
pub(crate) mod manager_cell;
mod scope;

pub use crate::bdd::manager_cell::{LocalCell, ManagerCell, SyncCell};
pub use batch::{ComposeMany, Composer};
pub use brand::Brand;
pub use builder::BddBuilder;
pub use handle::{Bdd, BddNode, BddVariables};
pub use scope::{Scope, ScopedBdd};

/// Items the `bdd_builder!` / `sync_bdd_builder!` macros need to name at their (possibly downstream)
/// call sites. Not part of the documented public API; named only by those macros.
#[doc(hidden)]
pub mod __macro_support {
    pub use super::brand::brand_seal::Sealed;
    pub use crate::bdd::manager_cell::{LocalCell, ManagerCell, SyncCell};
}

#[cfg(test)]
mod tests;
