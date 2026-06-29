//! Scoped, branded BDD contexts.
//!
//! A [`BddContext`] owns its own BDD manager and hands out [`BoolExpr`]s branded to it. The
//! [`bdd_context!`](crate::bdd_context) macro mints a fresh brand per call, so expressions from two
//! different contexts cannot be combined — it is a compile error, caught by the invariant brand type
//! parameter rather than a runtime check.
//!
//! This is the isolated alternative to the [`Global`]-brand free constructors
//! ([`BoolExpr::variable`](crate::BoolExpr::variable) and friends), which share one process-global
//! manager. A scoped context's manager is independent — separate node table, no contention or cache
//! pollution from unrelated global expressions — but still `Arc<RwLock<…>>`-backed, so scoped
//! expressions remain `Send`/`Sync`. Both coexist; `Global` is simply the default brand.
//!
//! ```
//! use espresso_logic::bdd_context;
//!
//! let ctx = bdd_context!();
//! let a = ctx.var("a");
//! let b = ctx.var("b");
//! let f = &a * &b + !&a * !&b; // a == b
//! assert_eq!(f, ctx.parse("a * b + ~a * ~b").unwrap());
//! ```

use super::manager::{BddManager, Store};
use super::BoolExpr;
use std::marker::PhantomData;

/// A brand: a zero-sized type identifying one BDD namespace.
///
/// [`Global`] is the default brand (the shared, process-global manager). Scoped brands are minted by
/// the [`bdd_context!`](crate::bdd_context) macro, which mints a fresh local type per call. The brand
/// flows through [`BoolExpr<B>`](crate::BoolExpr) as an invariant type parameter, so two distinct
/// brands never unify and their expressions cannot be mixed.
///
/// This trait is sealed: it cannot be implemented outside this crate. Every brand is therefore either
/// [`Global`] or a type minted by the [`bdd_context!`](crate::bdd_context) macro, which guarantees a
/// brand always corresponds to exactly one manager — so same-brand expressions always share a store.
///
/// ```compile_fail
/// use espresso_logic::Brand;
///
/// struct MyBrand;
/// impl Brand for MyBrand {} // error: `Brand` is sealed; only this crate can implement it
/// ```
pub trait Brand: __brand_seal::Sealed {}

impl<T: __brand_seal::Sealed> Brand for T {}

#[doc(hidden)]
pub mod __brand_seal {
    /// Sealing supertrait for [`Brand`](super::Brand). Implemented only by [`Global`](super::Global)
    /// and by the types the [`bdd_context!`](crate::bdd_context) macro mints; not part of the public
    /// API.
    pub trait Sealed: 'static {}
}

/// The default brand: the shared, process-global BDD manager.
///
/// `BoolExpr` with no explicit brand is `BoolExpr<Global>`; every expression built via the free
/// constructors shares one canonical, `Send`/`Sync` manager.
pub struct Global;

impl __brand_seal::Sealed for Global {}

/// An owned, scoped BDD namespace.
///
/// Create one with [`bdd_context!`](crate::bdd_context). Every [`BoolExpr`] it builds is branded to it
/// (via the macro-minted brand `B`) and shares this context's independent manager. Expressions own a
/// handle to that manager, so they remain valid after the context is dropped.
pub struct BddContext<B: Brand> {
    store: Store,
    _brand: PhantomData<fn() -> B>,
}

impl<B: Brand> BddContext<B> {
    /// Create a new context with a fresh, empty BDD manager.
    ///
    /// Prefer the [`bdd_context!`](crate::bdd_context) macro, which mints the brand for you.
    #[must_use]
    pub fn new() -> Self {
        BddContext {
            store: BddManager::new_store(),
            _brand: PhantomData,
        }
    }

    /// A variable expression, creating the variable in this context's ordering on first use.
    #[must_use]
    pub fn var<S: AsRef<str>>(&self, name: S) -> BoolExpr<B> {
        BoolExpr::var_in(self.store.clone(), name.as_ref())
    }

    /// A constant `true`/`false` expression in this context.
    #[must_use]
    pub fn constant(&self, value: bool) -> BoolExpr<B> {
        BoolExpr::constant_in(self.store.clone(), value)
    }

    /// Build an expression by composing [`Bdd`](super::Bdd) handles inside a closure (the scoped
    /// counterpart of [`BoolExpr::build`](crate::BoolExpr::build)).
    #[must_use]
    pub fn build<F>(&self, f: F) -> BoolExpr<B>
    where
        F: for<'b> FnOnce(&super::BddBuilder<'b, B>) -> super::Bdd<'b, B>,
    {
        super::builder::build_in(self.store.clone(), f)
    }

    /// Parse a boolean expression from a string into this context (the scoped counterpart of
    /// [`BoolExpr::parse`](crate::BoolExpr::parse)).
    pub fn parse<S: AsRef<str>>(
        &self,
        input: S,
    ) -> Result<BoolExpr<B>, super::error::ParseBoolExprError> {
        super::parser::parse_in(self.store.clone(), input.as_ref())
    }
}

impl<B: Brand> Default for BddContext<B> {
    fn default() -> Self {
        Self::new()
    }
}
