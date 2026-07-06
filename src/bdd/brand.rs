//! The sealed brand trait for the canonical BDD layer.
//!
//! A *brand* is a zero-sized marker type that names one BDD namespace for uniqueness. It carries no
//! behaviour and selects no implementation: it exists only so that two builders mint handles of distinct
//! types. The brand flows through every [`Bdd`](super::Bdd) handle as an invariant type parameter, so two
//! distinct brands never unify — handles minted by two different builders cannot be combined, and the
//! mismatch is a compile error rather than a runtime check.
//!
//! The orthogonal choice of storage backend (single-threaded versus thread-safe) is the separate
//! [`ManagerCell`](crate::bdd::manager_cell::ManagerCell) parameter, not the brand.
//!
//! The trait is **sealed**: it cannot be implemented outside this crate. There is deliberately **no
//! global / default brand** — the canonical layer has no process-global manager; every builder owns an
//! independent one. In-crate tests mint their own brand types (the seal permits in-crate impls); the
//! ergonomic public `bdd_builder!` / `sync_bdd_builder!` macros mint brands for callers.

/// A brand identifying one BDD namespace, for uniqueness only.
///
/// Sealed: only this crate can implement it (see the module docs). A brand carries no associated data and
/// selects no behaviour; it is purely a type-level marker that keeps handles of distinct builders from
/// unifying. `Copy + 'static` keeps brand values trivially duplicable.
///
/// ```compile_fail
/// use espresso_logic::bdd::Brand;
///
/// #[derive(Clone, Copy)]
/// struct MyBrand;
/// // error: `Brand` is sealed; only `espresso_logic` can implement it.
/// impl Brand for MyBrand {}
/// ```
pub trait Brand: brand_seal::Sealed + Copy + 'static {}

pub(crate) mod brand_seal {
    /// Sealing supertrait for [`Brand`](super::Brand): only impls inside this crate can name it, so the
    /// brand trait cannot be implemented downstream. Not part of the public API.
    pub trait Sealed: 'static {}
}
