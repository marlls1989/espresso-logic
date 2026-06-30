//! The interior-mutability cell abstraction the BDD engine is written over.
//!
//! [`ManagerCell`] hides *which* shared, interior-mutable container holds a [`BddManager`] from the
//! node-table / unique-table / `ite` engine in [`manager`](super::manager): the engine functions are
//! written once, generically over a `C: ManagerCell`, and each concrete cell supplies its own borrow
//! guards through the trait's GATs.
//!
//! Two cells exist:
//!
//! - [`LocalCell`] — `Rc<RefCell<BddManager>>`, single-threaded (`!Send`/`!Sync`), backing a
//!   single-threaded builder.
//! - [`SyncCell`] — `Arc<RwLock<BddManager>>`, `Send`/`Sync`, backing a thread-safe builder.
//!
//! [`ManagerCell`] is the second of a BDD handle's two orthogonal type parameters: a
//! [`Brand`](crate::bdd::Brand) marks one namespace for uniqueness, while the cell selects the storage
//! backend (single-threaded versus thread-safe). The two are independent — any brand pairs with either
//! cell.
//!
//! The trait is **sealed** (via [`cell_seal::Sealed`]): no downstream crate can add another cell, so the
//! engine's borrow discipline only ever has to be correct for these two.
//!
//! # Borrow discipline
//!
//! A [`RefCell`] **panics** if a `borrow()` is live when a `borrow_mut()` is requested, where an
//! [`RwLock`] would instead deadlock. The engine therefore never overlaps a [`read`](ManagerCell::read)
//! guard with a [`write`](ManagerCell::write) guard on the same cell: every lookup/commit takes one
//! short-lived guard for its own scope and drops it before the next is taken. This single discipline is
//! correct for both cells.

use super::manager::BddManager;
use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// The storage backend a BDD builder and its handles share: a cloneable handle to one shared,
/// interior-mutable [`BddManager`].
///
/// Implemented only by [`LocalCell`] and [`SyncCell`] (the trait is sealed). It is the second of a
/// handle's two orthogonal type parameters — a [`Brand`](crate::bdd::Brand) marks the namespace, this
/// cell selects single-threaded or thread-safe storage. The BDD engine in [`manager`](super::manager) is
/// generic over this trait, so its node-construction and `ite` logic is written exactly once and shared
/// by both backends.
///
/// `Clone` is a refcount bump (`Rc::clone` / `Arc::clone`): every clone shares the same underlying
/// manager. A [`BddBuilder`](crate::bdd::BddBuilder) owns one cell; each [`Bdd`](crate::bdd::Bdd) handle
/// it mints holds its own refcounted clone, so a handle keeps its manager alive independently of the
/// builder.
pub trait ManagerCell: Clone + cell_seal::Sealed {
    /// Shared-borrow guard, dereferencing to the [`BddManager`] for read-only access.
    type ReadGuard<'a>: core::ops::Deref<Target = BddManager>
    where
        Self: 'a;
    /// Exclusive-borrow guard, dereferencing mutably to the [`BddManager`].
    type WriteGuard<'a>: core::ops::DerefMut<Target = BddManager>
    where
        Self: 'a;

    /// A fresh cell wrapping a manager seeded with the two terminal nodes
    /// ([`new_empty`](BddManager::new_empty)).
    ///
    /// The canonical BDD layer's [`BddBuilder`](crate::bdd::BddBuilder) mints its cell through this.
    fn new_empty() -> Self;

    /// Take a shared borrow of the manager. Must not overlap a [`write`](Self::write) on the same cell
    /// (see the module-level borrow discipline).
    fn read(&self) -> Self::ReadGuard<'_>;

    /// Take an exclusive borrow of the manager. Must not overlap any other borrow on the same cell.
    fn write(&self) -> Self::WriteGuard<'_>;

    /// A stable pointer identifying this cell's manager, for equality / hashing of handles into it.
    /// Two clones of the same cell return the same pointer; two independently created cells do not.
    fn as_ptr(&self) -> *const ();
}

/// Single-threaded cell: `Rc<RefCell<BddManager>>`. `!Send`/`!Sync`.
///
/// A [`BddBuilder`](crate::bdd::BddBuilder) parameterised by this cell is single-threaded and pays no
/// synchronisation cost; the [`bdd_builder!`](crate::bdd_builder) macro mints builders over it.
#[derive(Clone)]
pub struct LocalCell(Rc<RefCell<BddManager>>);

impl cell_seal::Sealed for LocalCell {}

impl ManagerCell for LocalCell {
    type ReadGuard<'a> = Ref<'a, BddManager>;
    type WriteGuard<'a> = RefMut<'a, BddManager>;

    fn new_empty() -> Self {
        LocalCell(Rc::new(RefCell::new(BddManager::new_empty())))
    }

    fn read(&self) -> Self::ReadGuard<'_> {
        self.0.borrow()
    }

    fn write(&self) -> Self::WriteGuard<'_> {
        self.0.borrow_mut()
    }

    fn as_ptr(&self) -> *const () {
        Rc::as_ptr(&self.0).cast::<()>()
    }
}

/// Thread-safe cell: `Arc<RwLock<BddManager>>`. `Send + Sync`.
///
/// Lock poisoning **propagates**: [`read`](ManagerCell::read)/[`write`](ManagerCell::write) `unwrap()`
/// the guard, so a panic while the manager is borrowed poisons the lock for every subsequent access.
///
/// A [`BddBuilder`](crate::bdd::BddBuilder) parameterised by this cell is `Send + Sync`; the
/// [`sync_bdd_builder!`](crate::sync_bdd_builder) macro mints builders over it.
#[derive(Clone)]
pub struct SyncCell(Arc<RwLock<BddManager>>);

impl cell_seal::Sealed for SyncCell {}

impl ManagerCell for SyncCell {
    type ReadGuard<'a> = RwLockReadGuard<'a, BddManager>;
    type WriteGuard<'a> = RwLockWriteGuard<'a, BddManager>;

    fn new_empty() -> Self {
        SyncCell(Arc::new(RwLock::new(BddManager::new_empty())))
    }

    fn read(&self) -> Self::ReadGuard<'_> {
        self.0.read().unwrap()
    }

    fn write(&self) -> Self::WriteGuard<'_> {
        self.0.write().unwrap()
    }

    fn as_ptr(&self) -> *const () {
        Arc::as_ptr(&self.0).cast::<()>()
    }
}

#[doc(hidden)]
pub mod cell_seal {
    /// Sealing supertrait for [`ManagerCell`](super::ManagerCell): only this module's two cells
    /// implement it. (`ManagerCell` is in any case un-implementable downstream — its methods name the
    /// crate-private `BddManager` — so this seal is belt-and-braces.)
    pub trait Sealed {}
}
