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
//! # Construction in tests today
//!
//! The ergonomic public macros (`bdd_context!` / `sync_bdd_context!`) that mint a brand per call arrive
//! with the 5.0 breaking cut. Until then a context is constructed in-crate against a brand type that
//! implements the sealed [`Brand`] trait (the seal permits in-crate impls).
//!
//! Syntactic construction from a Boolean expression (`BddContext::build(&BoolExpr)`, `parse`, and
//! `Bdd::to_expr()`) also arrives with the breaking cut, alongside the new owned-RPN `BoolExpr`.

mod brand;
mod context;
mod handle;

pub use brand::Brand;
pub use context::{BddContext, SyncBddContext};
pub use handle::Bdd;

#[cfg(test)]
mod tests;
