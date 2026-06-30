//! The sealed brand trait for the canonical BDD layer.
//!
//! A *brand* is a zero-sized marker type that names one BDD namespace. Each brand chooses, at the type
//! level, **which** interior-mutability cell its context owns via the associated [`Cell`](Brand::Cell)
//! type:
//!
//! - a brand whose `Cell` is [`LocalCell`](crate::expression::manager_cell::LocalCell) backs a
//!   single-threaded [`BddBuilder`](super::BddBuilder) (`!Send`/`!Sync`);
//! - a brand whose `Cell` is [`SyncCell`](crate::expression::manager_cell::SyncCell) backs a
//!   thread-safe [`SyncBddBuilder`](super::SyncBddBuilder) (`Send + Sync`).
//!
//! The brand flows through every [`Bdd`](super::Bdd) handle as an invariant type parameter, so two
//! distinct brands never unify: handles minted by two different contexts cannot be combined, and the
//! mismatch is a compile error rather than a runtime check.
//!
//! The trait is **sealed**: it cannot be implemented outside this crate. There is deliberately **no
//! global / default brand** — the canonical layer has no process-global manager; every context owns an
//! independent one. In-crate tests mint their own brand types (the seal permits in-crate impls); the
//! ergonomic public `bdd_builder!` / `sync_bdd_builder!` macros that mint brands for callers arrive with
//! the 5.0 breaking cut.

use crate::expression::manager_cell::ManagerCell;

/// A brand identifying one BDD namespace, selecting the cell its context owns.
///
/// Sealed: only this crate can implement it (see the module docs). `Copy + 'static` keeps brand values
/// trivially duplicable and lets a brand be used purely as a type-level marker.
///
/// ```compile_fail
/// use espresso_logic::bdd::Brand;
///
/// #[derive(Clone, Copy)]
/// struct MyBrand;
/// // error: `Brand` is sealed; only `espresso_logic` can implement it.
/// impl Brand for MyBrand {
///     type Cell = ();
/// }
/// ```
pub trait Brand: brand_seal::Sealed + Copy + 'static {
    /// The interior-mutability cell a context for this brand owns. Selecting
    /// [`LocalCell`](crate::expression::manager_cell::LocalCell) gives a single-threaded context;
    /// selecting [`SyncCell`](crate::expression::manager_cell::SyncCell) gives a `Send + Sync` one.
    type Cell: ManagerCell;
}

pub(crate) mod brand_seal {
    /// Sealing supertrait for [`Brand`](super::Brand): only impls inside this crate can name it, so the
    /// brand trait cannot be implemented downstream. Not part of the public API.
    pub trait Sealed: 'static {}
}
