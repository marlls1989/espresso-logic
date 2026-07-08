//! Streaming batch composition: apply one substitution across a stream of handles, sharing a single
//! short-lived memo so functions that overlap in structure are composed once.
//!
//! The single-function [`Bdd::compose`](super::Bdd::compose) /
//! [`Bdd::compose_map`](super::Bdd::compose_map) walk one function with a throwaway memo. When the
//! *same* substitution is applied to many related functions — renaming a signal across every gate in
//! a netlist, say — those functions share large sub-graphs, and re-deriving them per call is wasted
//! work. [`Composer`] runs the whole stream through one shared memo instead: because a node composes
//! identically under a fixed substitution, a sub-graph computed for one function is reused for the
//! next, and the memo is dropped when the batch iterator is. It is reached by calling
//! [`compose`](Composer::compose) / [`compose_map`](Composer::compose_map) on any iterator of handles
//! (owned [`Bdd`] or borrowed [`ScopedBdd`]).

use std::collections::HashMap;
use std::fmt;
use std::iter::FusedIterator;

use super::encoding;
use super::manager::{BddOps, NodeId, VarId};
use super::{Bdd, Brand, ManagerCell, ScopedBdd};

mod sealed {
    use super::{ManagerCell, NodeId};

    /// The callable, unimplementable core of [`BatchHandle`](super::BatchHandle): the handle-shaped
    /// operations the batch machinery needs, so the iterator and the [`Composer`](super::Composer)
    /// methods can be written once for both owned [`Bdd`](super::Bdd) and borrowed
    /// [`ScopedBdd`](super::ScopedBdd) handles.
    ///
    /// Kept in a private module so its name is unreachable downstream: safe code outside the crate can
    /// therefore neither call `hold`/`mint` nor implement it, so it cannot fabricate a handle with a
    /// chosen root and break the mint invariant.
    pub trait Sealed: Sized {
        /// The storage cell backing this handle's manager.
        type Cell: ManagerCell;
        /// How the batch iterator keeps the manager alive between pulls: an owned refcount clone for
        /// [`Bdd`](super::Bdd), a borrow for [`ScopedBdd`](super::ScopedBdd).
        type Held;
        fn root(&self) -> NodeId;
        /// Capture the manager from a handle into the iterator-held form.
        fn hold(&self) -> Self::Held;
        /// Borrow the manager back out to run the walk and mint results.
        fn cell(held: &Self::Held) -> &Self::Cell;
        /// Mint a result handle for `root` on the held manager.
        fn mint(held: &Self::Held, root: NodeId) -> Self;
        /// Backstop that a handle shares the held manager (a real assertion for the owned handle; a
        /// no-op for the scoped handle, whose invariant lifetime already guarantees it).
        fn check(held: &Self::Held, handle: &Self);
    }
}

use sealed::Sealed;

/// Marker for the handle types the batch machinery accepts, so [`Composer`] and [`ComposeMany`] can be
/// written once for both owned [`Bdd`] and borrowed [`ScopedBdd`] handles. Two `IntoIterator`-bound
/// blanket impls (one per handle type) would clash under coherence; routing through this single trait
/// avoids that.
///
/// Sealed and method-less: implemented only for [`Bdd`] and [`ScopedBdd`], neither nameable-to-implement
/// nor callable downstream. The handle-shaped operations live on a private supertrait so safe downstream
/// code cannot mint a handle with an arbitrary root.
pub trait BatchHandle: Sealed {}

impl<T: Sealed> BatchHandle for T {}

impl<B: Brand, C: ManagerCell> Sealed for Bdd<B, C> {
    type Cell = C;
    type Held = C;
    fn root(&self) -> NodeId {
        Bdd::root(self)
    }
    fn hold(&self) -> C {
        self.cell().clone()
    }
    fn cell(held: &C) -> &C {
        held
    }
    fn mint(held: &C, root: NodeId) -> Self {
        Bdd::from_root(held, root)
    }
    fn check(held: &C, handle: &Self) {
        assert!(
            held.as_ptr() == handle.cell().as_ptr(),
            "batch compose: a function came from a different manager (brand clash); mixing them is a bug"
        );
    }
}

impl<'s, B: Brand, C: ManagerCell> Sealed for ScopedBdd<'s, B, C> {
    type Cell = C;
    type Held = &'s C;
    fn root(&self) -> NodeId {
        ScopedBdd::root(*self)
    }
    fn hold(&self) -> &'s C {
        self.cell()
    }
    fn cell<'a>(held: &'a &'s C) -> &'a C {
        held
    }
    fn mint(held: &&'s C, root: NodeId) -> Self {
        ScopedBdd::from_root(held, root)
    }
    fn check(_held: &&'s C, _handle: &Self) {
        // The invariant scope lifetime already pins every scoped handle to one manager.
    }
}

/// The lazy result of composing one substitution across a stream of handles, yielded by
/// [`Composer::compose`] / [`Composer::compose_map`]. It owns the resolved substitution and the one
/// shared memo, composing the next function on each pull and reusing sub-graphs already seen; both are
/// dropped when the iterator is. Yields one result per input, in order.
pub struct ComposeMany<I: Iterator, H: BatchHandle> {
    functions: I,
    /// Seeded from the substitution when there is one, else lazily from the first function (the
    /// empty-substitution identity path needs no manager up front).
    held: Option<H::Held>,
    map: HashMap<VarId, NodeId>,
    memo: HashMap<NodeId, NodeId>,
}

impl<I, H> Iterator for ComposeMany<I, H>
where
    I: Iterator<Item = H>,
    H: BatchHandle,
{
    type Item = H;

    fn next(&mut self) -> Option<H> {
        let f = self.functions.next()?;
        if self.held.is_none() {
            self.held = Some(f.hold());
        }
        let held = self.held.as_ref().expect("held seeded above");
        H::check(held, &f);
        // An empty substitution is the identity: yield the pulled function unchanged rather than
        // re-walking its graph to rebuild an identical result (mirrors the non-batch short-circuit in
        // `encoding`). The stream-function manager check above still runs, so a cross-manager stream is
        // caught either way.
        if self.map.is_empty() {
            return Some(f);
        }
        let root = H::cell(held).compose_into(f.root(), &self.map, &mut self.memo);
        Some(H::mint(held, root))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.functions.size_hint()
    }
}

impl<I, H> ExactSizeIterator for ComposeMany<I, H>
where
    I: ExactSizeIterator<Item = H>,
    H: BatchHandle,
{
}

impl<I, H> FusedIterator for ComposeMany<I, H>
where
    I: FusedIterator<Item = H>,
    H: BatchHandle,
{
}

impl<I: Iterator, H: BatchHandle> fmt::Debug for ComposeMany<I, H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComposeMany").finish_non_exhaustive()
    }
}

/// Apply one substitution across a stream of handles as a lazy iterator that shares a single
/// short-lived memo. Import this trait (`use espresso_logic::bdd::Composer;`) to call the methods on
/// any iterator of [`Bdd`] or [`ScopedBdd`] handles.
///
/// [`compose`](Self::compose) substitutes one function for a variable across the stream;
/// [`compose_map`](Self::compose_map) applies several substitutions at once. Both read every function
/// as it was *before* substitution and yield one result per input, in order. A substitution naming a
/// variable a function does not test — or an empty map — leaves that function unchanged.
///
/// ```
/// use espresso_logic::{bdd_builder, BddBuilder, LocalCell};
/// use espresso_logic::bdd::Composer;
///
/// let builder: BddBuilder<_, LocalCell> = bdd_builder!();
/// let a = builder.var("a");
/// let b = builder.var("b");
/// let c = builder.var("c");
///
/// // Substitute b := c across two functions in one shared-cache pass.
/// let f1 = &a & &b;
/// let f2 = &b | &c;
/// let out: Vec<_> = [f1.clone(), f2.clone()].into_iter().compose("b", c.clone()).collect();
///
/// assert!(out[0].equivalent_to(&(&a & &c)));
/// assert!(out[1].equivalent_to(&(&c | &c)));
/// ```
pub trait Composer<H: BatchHandle>: IntoIterator<Item = H> + Sized {
    /// Substitute `g` for the variable `var` in every function of the stream: `f[var := g]` for each
    /// `f`. The single-substitution shorthand for [`compose_map`](Self::compose_map).
    fn compose<S: AsRef<str>>(self, var: S, g: H) -> ComposeMany<Self::IntoIter, H>;

    /// Simultaneously substitute every `(name, g)` in `substitution` across every function of the
    /// stream. A repeated name takes its last entry; names absent from a function are left alone; an
    /// empty substitution yields each function unchanged.
    fn compose_map<S: AsRef<str>>(
        self,
        substitution: impl IntoIterator<Item = (S, H)>,
    ) -> ComposeMany<Self::IntoIter, H>;
}

impl<T, H> Composer<H> for T
where
    T: IntoIterator<Item = H>,
    H: BatchHandle,
{
    fn compose<S: AsRef<str>>(self, var: S, g: H) -> ComposeMany<Self::IntoIter, H> {
        let held = g.hold();
        let map = encoding::resolve_substitution(H::cell(&held), std::iter::once((var, g.root())));
        ComposeMany {
            functions: self.into_iter(),
            held: Some(held),
            map,
            memo: HashMap::new(),
        }
    }

    fn compose_map<S: AsRef<str>>(
        self,
        substitution: impl IntoIterator<Item = (S, H)>,
    ) -> ComposeMany<Self::IntoIter, H> {
        let entries: Vec<(S, H)> = substitution.into_iter().collect();
        // The substitution seeds the held manager; an empty one leaves the identity path to seed it
        // from the first function on the first pull.
        let held = entries.first().map(|(_, g)| g.hold());
        let map = match &held {
            Some(h) => {
                // Every substitute's root is resolved against this one held manager, so assert each
                // belongs to it first — otherwise a foreign `g` is read in the wrong NodeId space,
                // surfacing as the internal "invalid node id" panic or a silent wrong result. Mirrors
                // the per-substitute `assert_same_manager` of the single-function `Bdd::compose_map`.
                for (_, g) in &entries {
                    H::check(h, g);
                }
                encoding::resolve_substitution(
                    H::cell(h),
                    entries.iter().map(|(name, g)| (name.as_ref(), g.root())),
                )
            }
            None => HashMap::new(),
        };
        ComposeMany {
            functions: self.into_iter(),
            held,
            map,
            memo: HashMap::new(),
        }
    }
}
