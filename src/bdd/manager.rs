//! BDD manager implementation for canonical node representation
//!
//! This module contains the internal BDD data structures and management logic.
//! The BDD manager maintains:
//! - Global singleton manager with thread-local storage
//! - Hash consing for canonical node representation
//! - Operation caching for boolean operations
//! - Variable ordering (first-seen / insertion order)

use super::manager_cell::ManagerCell;
use crate::cover::{Anonymous, Minterm};
use crate::Symbol;
use std::collections::{BTreeMap, HashMap};

/// Node identifier in the BDD
pub(crate) type NodeId = usize;

/// Variable identifier (index in variable ordering)
pub(crate) type VarId = usize;

/// An ITE cofactor triple `(f, g, h)` — the operand node ids of one `if f then g else h`
/// subproblem (or, inside the `compose`/`compose_map` engines' embedded ITE machinery, a
/// splice/recombination triple such as `(g, f_high, f_low)`).
pub(crate) type NodeTriple = (NodeId, NodeId, NodeId);

/// A Shannon expansion of an ITE triple: the split variable plus its low (false-cofactor) and
/// high (true-cofactor) child triples.
pub(crate) type IteSplit = (VarId, NodeTriple, NodeTriple);

/// Terminal node for FALSE
pub(crate) const FALSE_NODE: NodeId = 0;

/// Terminal node for TRUE
pub(crate) const TRUE_NODE: NodeId = 1;

/// Binary decision diagram node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum BddNode {
    /// Terminal node (true or false)
    Terminal(bool),
    /// Decision node
    Decision {
        var: VarId,
        low: NodeId,  // false edge
        high: NodeId, // true edge
    },
}

/// Shared BDD manager that maintains the unique table and caches
///
/// The manager uses interior mutability to allow sharing BDDs across multiple references
/// while still being able to modify internal caches.
///
/// # Critical Invariant: NodeId Stability
///
/// **NodeIds are stable** - once a node is created at a given index, it remains at that
/// index forever. The `nodes` Vec only grows (via `push`), never shrinks or reorders.
/// This guarantees that:
/// - A NodeId is valid for the lifetime of the manager
/// - Multiple threads can safely traverse using NodeIds after releasing read locks
/// - Recursive traversal can release locks between calls without invalidating NodeIds
// Doc-hidden public so the cell types (which the `bdd_builder!` / `sync_bdd_builder!` macros name) can
// expose it through the public `ManagerCell` interface. Its fields and constructors stay crate-private,
// so it is opaque outside the crate.
#[doc(hidden)]
#[derive(Debug)]
pub struct BddManager {
    /// All nodes in the BDD (terminals at indices 0 and 1)
    /// INVARIANT: Nodes are never removed or reordered - only appended
    pub(super) nodes: Vec<BddNode>,
    /// Unique table: (var, low, high) -> NodeId for hash consing
    pub(super) unique_table: HashMap<(VarId, NodeId, NodeId), NodeId>,
    /// Variable ordering: variable name -> variable id
    pub(super) var_to_id: BTreeMap<Symbol, VarId>,
    /// Reverse mapping: variable id -> variable name
    pub(super) id_to_var: Vec<Symbol>,
    /// Cache for ITE operations: (f, g, h) -> result
    pub(super) ite_cache: HashMap<NodeTriple, NodeId>,
    /// Cache for compose operations: (f, var, g) -> f[var := g]
    pub(super) compose_cache: HashMap<(NodeId, VarId, NodeId), NodeId>,
}

impl BddManager {
    /// Resolve an ITE triple **without** Shannon expansion.
    ///
    /// Returns `Some(node)` when `(f, g, h)` is a terminal case or already lives in `ite_cache`,
    /// and `None` when it still needs expanding. This is the memo-aware lookup the iterative
    /// [`ite`](BddOps::ite) loop uses both to short-circuit `Solve` items and to read back child
    /// results in `Combine`. The terminal-case checks mirror the head of the former recursive `ite`.
    fn ite_resolved(&self, f: NodeId, g: NodeId, h: NodeId) -> Option<NodeId> {
        if f == TRUE_NODE {
            return Some(g);
        }
        if f == FALSE_NODE {
            return Some(h);
        }
        if g == TRUE_NODE && h == FALSE_NODE {
            return Some(f);
        }
        if g == h {
            return Some(g);
        }
        self.ite_cache.get(&(f, g, h)).copied()
    }

    /// Shannon-expand a non-terminal ITE triple around its topmost variable.
    ///
    /// Returns the split variable and the two child triples (low/false cofactor and high/true
    /// cofactor). Only called when [`ite_resolved`](Self::ite_resolved) returned `None`, so at
    /// least `f` is a decision node and `top_var` is a real variable.
    fn ite_expand(&self, f: NodeId, g: NodeId, h: NodeId) -> IteSplit {
        let f_node = self.expect_node(f);
        let g_node = self.expect_node(g);
        let h_node = self.expect_node(h);

        let f_var = node_var(f_node);
        let g_var = node_var(g_node);
        let h_var = node_var(h_node);
        let top_var = f_var.min(g_var).min(h_var);

        let (f_low, f_high) = cofactors(f_node, f_var, top_var, f);
        let (g_low, g_high) = cofactors(g_node, g_var, top_var, g);
        let (h_low, h_high) = cofactors(h_node, h_var, top_var, h);

        (top_var, (f_low, g_low, h_low), (f_high, g_high, h_high))
    }
    /// A fresh, empty manager seeded with the two terminal nodes (`FALSE_NODE = 0`, `TRUE_NODE = 1`).
    ///
    /// Every [`BddBuilder`](crate::bdd::BddBuilder) owns one of these (minted through its cell's
    /// [`new_empty`](super::manager_cell::ManagerCell::new_empty)).
    pub(crate) fn new_empty() -> Self {
        BddManager {
            nodes: vec![
                BddNode::Terminal(false), // FALSE_NODE = 0
                BddNode::Terminal(true),  // TRUE_NODE = 1
            ],
            unique_table: HashMap::new(),
            var_to_id: BTreeMap::new(),
            id_to_var: Vec::new(),
            ite_cache: HashMap::new(),
            compose_cache: HashMap::new(),
        }
    }

    /// Read-only lookup of an existing variable id — the shared-borrow fast path of
    /// [`make_var`](BddOps::make_var). Also used by the BDD layer to resolve a variable *name* to a
    /// `VarId` without creating it (a name absent from the ordering yields `None`, which the cofactor /
    /// quantification primitives treat as a no-op).
    pub(crate) fn var_id(&self, name: &str) -> Option<VarId> {
        // `Symbol: Borrow<str>`, so the lookup borrows `name` directly rather than minting a throwaway
        // `Symbol` (and locking the global intern pool for a long name) on every call.
        self.var_to_id.get(name).copied()
    }

    /// Append `name` as a new variable (or return its id if already present). Caller holds the write lock.
    fn get_or_create_var(&mut self, name: &str) -> VarId {
        let key: Symbol = Symbol::from(name);
        if let Some(&id) = self.var_to_id.get(&key) {
            id
        } else {
            let id = self.id_to_var.len();
            self.var_to_id.insert(key.clone(), id);
            self.id_to_var.push(key);
            id
        }
    }

    /// Get variable name from ID
    pub(crate) fn var_name(&self, id: VarId) -> Option<&Symbol> {
        self.id_to_var.get(id)
    }

    /// Intern a decision node, re-checking the unique table. Caller holds the write lock.
    ///
    /// # Invariant
    /// Only creates Decision nodes, never Terminal nodes (terminals are fixed at positions 0 and 1).
    fn insert_node(&mut self, var: VarId, low: NodeId, high: NodeId) -> NodeId {
        // Reduction rule: if low == high, return that node (redundant test elimination)
        if low == high {
            return low;
        }

        // Authoritative unique-table check (the read-path check above is only advisory)
        let key = (var, low, high);
        if let Some(&existing) = self.unique_table.get(&key) {
            return existing;
        }

        // Create new decision node (never terminals - those are at 0 and 1)
        let node_id = self.nodes.len();
        self.nodes.push(BddNode::Decision { var, low, high });
        self.unique_table.insert(key, node_id);
        node_id
    }

    /// Get node by ID
    pub(crate) fn get_node(&self, id: NodeId) -> Option<&BddNode> {
        self.nodes.get(id)
    }

    /// Look up a node by ID, panicking with one consistent message when the ID is invalid.
    ///
    /// Every ID a [`Bdd`](crate::bdd::Bdd) handle holds was minted by this manager, and node IDs are
    /// stable (never removed or reordered), so an out-of-range ID is never user error — it signals a
    /// bug in the BDD implementation. This is the single bounds-checked lookup the traversal and
    /// derivation code routes through.
    pub(crate) fn expect_node(&self, id: NodeId) -> &BddNode {
        self.get_node(id).unwrap_or_else(|| {
            panic!("invalid node id {id} - this indicates a bug in the BDD implementation")
        })
    }
}

/// The crate-internal BDD operation surface: the read-mostly algorithms that manage a cell's borrows
/// themselves, expressed as methods on the cell (`cell.ite(...)`, `cell.make_node(...)`).
///
/// [`ManagerCell`] is the pure storage/locking backend; every operation here schedules its own
/// short-lived shared and exclusive borrows over that cell, so no borrow ever spans more than a single
/// step (the discipline the [`LocalCell`](super::manager_cell::LocalCell)'s `RefCell` and the
/// [`SyncCell`](super::manager_cell::SyncCell)'s `RwLock` both require). The blanket impl below gives
/// every cell these operations, and the sealed [`ManagerCell`] supertrait keeps the surface
/// crate-internal — no outside type can implement or observe it.
pub(crate) trait BddOps: ManagerCell {
    /// Get or create the variable id for `name`, managing the cell's borrow itself.
    ///
    /// Read-mostly: an already-known variable resolves under a shared borrow (concurrent lookups run in
    /// parallel on a [`SyncCell`](super::manager_cell::SyncCell)); only a genuinely new variable escalates
    /// to an exclusive borrow to append it. The shared borrow is dropped before the exclusive borrow is
    /// taken, so the read→write hand-off never overlaps two borrows of the same cell — required for the
    /// [`LocalCell`](super::manager_cell::LocalCell)'s `RefCell` (which would panic on overlap) and the
    /// `SyncCell`'s `RwLock` (which would deadlock).
    fn make_var(&self, name: &str) -> VarId {
        {
            let manager = self.read();
            if let Some(id) = manager.var_id(name) {
                return id;
            }
        }
        // Re-check under the exclusive borrow: another thread may have appended `name` meanwhile.
        self.write().get_or_create_var(name)
    }

    /// Get or create a canonical decision node, managing the cell's borrow itself.
    ///
    /// Read-mostly hash-consing: the reduction rule needs no borrow, an already-interned node resolves
    /// under a shared borrow (concurrent lookups run in parallel on a
    /// [`SyncCell`](super::manager_cell::SyncCell)), and only a brand-new node escalates to an exclusive
    /// borrow. The shared borrow is dropped before the exclusive borrow is taken, so the two never
    /// overlap (see [`make_var`](Self::make_var)). NodeIds are stable, so the id returned from the read
    /// path stays valid after its borrow is released.
    fn make_node(&self, var: VarId, low: NodeId, high: NodeId) -> NodeId {
        // Reduction rule (no borrow): a redundant test collapses to its child.
        if low == high {
            return low;
        }
        let key = (var, low, high);
        // Shared-borrow fast path: an existing canonical node needs no exclusive borrow.
        {
            let manager = self.read();
            if let Some(&existing) = manager.unique_table.get(&key) {
                return existing;
            }
        }
        // Append path: re-check under the exclusive borrow (another thread may have interned it), insert.
        self.write().insert_node(var, low, high)
    }

    /// If-Then-Else (`if f then g else h`), managing the cell's borrow itself.
    ///
    /// The fundamental BDD operation all others derive from. Read-mostly, but — unlike a held-guard
    /// design — **no borrow spans more than a single step**: each `Solve`/`Combine` step takes its own
    /// short-lived shared borrow to read (resolve/expand/read children) and, only when a triple must be
    /// committed, a separate short-lived exclusive borrow to intern the node and record its cache entry.
    /// A shared borrow is never live when the exclusive borrow is taken, which is the discipline the
    /// [`LocalCell`](super::manager_cell::LocalCell)'s `RefCell` requires (overlapping `borrow()` then
    /// `borrow_mut()` would panic) and the [`SyncCell`](super::manager_cell::SyncCell)'s `RwLock` requires
    /// (read→write on the same lock would deadlock). NodeIds and cache entries are stable / monotonic, so
    /// releasing the borrow between steps never invalidates an id read in an earlier step. Each commit
    /// interns the node and records its cache entry as one transaction (never released with a node created
    /// but its result uncached), so re-deriving an existing expression resolves entirely on shared borrows
    /// (parallel across threads on the `SyncCell`), and even a fresh computation takes the exclusive
    /// borrow only momentarily, once per committed triple.
    ///
    /// Evaluated **iteratively** with an explicit work-stack rather than recursion, so a tall BDD (deep
    /// variable ordering) can't overflow the call stack. Memoisation is preserved exactly: every
    /// sub-triple is resolved through `ite_resolved` (terminal cases + `ite_cache`), so shared
    /// sub-problems collapse to cache hits, keeping the walk linear in the number of distinct reachable
    /// triples, not exponential.
    fn ite(&self, f: NodeId, g: NodeId, h: NodeId) -> NodeId {
        /// One unit of work. `Solve` resolves a triple (expanding it if needed); `Combine` runs
        /// after a triple's two children are resolved and builds the result node for it.
        enum Work {
            Solve(NodeId, NodeId, NodeId),
            Combine {
                triple: NodeTriple,
                top_var: VarId,
                low: NodeTriple,
                high: NodeTriple,
            },
        }

        let mut stack = vec![Work::Solve(f, g, h)];
        while let Some(work) = stack.pop() {
            match work {
                Work::Solve(f, g, h) => {
                    // Bail if the triple was already resolved (terminal or memoised).
                    let Some((top_var, low, high)) = ite_solve_step(self, f, g, h) else {
                        continue;
                    };
                    // Schedule both cofactor triples and a Combine that runs once they're resolved
                    // (Combine pushed first → pops last, LIFO).
                    stack.push(Work::Combine {
                        triple: (f, g, h),
                        top_var,
                        low,
                        high,
                    });
                    stack.push(Work::Solve(high.0, high.1, high.2));
                    stack.push(Work::Solve(low.0, low.1, low.2));
                }
                Work::Combine {
                    triple,
                    top_var,
                    low,
                    high,
                } => ite_combine_step(self, triple, top_var, low, high),
            }
        }

        // Final result read under its own short-lived shared borrow.
        self.read().ite_resolved(f, g, h).expect(
            "top-level ITE triple unresolved after iterative evaluation - BDD scheduling bug",
        )
    }

    /// Logical AND of two nodes, `and(f, g) = ite(f, g, FALSE)`, managing the cell's borrow itself.
    ///
    /// A thin [`ite`](Self::ite) encoding — it inherits `ite`'s hash-consing and memoisation and stays
    /// canonical, and does its own read-mostly borrowing. Used by the owned and scoped BDD handles.
    fn and(&self, f: NodeId, g: NodeId) -> NodeId {
        self.ite(f, g, FALSE_NODE)
    }

    /// Logical OR of two nodes, `or(f, g) = ite(f, TRUE, g)`, managing the cell's borrow itself.
    ///
    /// A thin [`ite`](Self::ite) encoding; see [`and`](Self::and).
    fn or(&self, f: NodeId, g: NodeId) -> NodeId {
        self.ite(f, TRUE_NODE, g)
    }

    /// Logical NOT of a node, `not(f) = ite(f, FALSE, TRUE)`, managing the cell's borrow itself.
    ///
    /// A thin [`ite`](Self::ite) encoding; see [`and`](Self::and).
    fn not(&self, f: NodeId) -> NodeId {
        self.ite(f, FALSE_NODE, TRUE_NODE)
    }

    /// Exclusive-or of two nodes, `xor(f, g) = ite(f, !g, g)`, managing the cell's borrow itself.
    ///
    /// Built from [`ite`](Self::ite) (so it inherits the same hash-consing and memoisation and stays
    /// canonical): `!g = ite(g, FALSE, TRUE)`, then select `!g` when `f` is true and `g` when `f` is
    /// false. Each sub-`ite` does its own read-mostly borrowing. Used by the owned and scoped BDD handles.
    fn xor(&self, f: NodeId, g: NodeId) -> NodeId {
        let not_g = self.ite(g, FALSE_NODE, TRUE_NODE);
        self.ite(f, not_g, g)
    }

    /// Simultaneous Shannon cofactor by a partial assignment: `node|{v=value, …}` — fix every
    /// variable the `assignment` pins (a `Some` field) to that constant in **one pass**, restricting
    /// the function to the subspace the minterm denotes. Equal to chaining a single-variable cofactor
    /// over the same fixed variables in any order, but without re-walking `node` per variable. A
    /// variable the assignment leaves free (a `None` field, or an index at or past its length) stays
    /// free; an all-free assignment is a no-op.
    ///
    /// `assignment` is **positional over `VarId`**: index `v` is the value forced on the variable with
    /// id `v`. A caller holding a label-keyed [`Minterm`] aligns it onto this manager's variable order
    /// first (see `encoding::restrict_to`).
    ///
    /// A per-call `memo` keyed on the **original** node id collapses the shared sub-DAG so the walk
    /// stays linear in the number of distinct reachable nodes, and each node's `(var, low, high)` is
    /// read under a **single short-lived shared borrow** dropped before the rebuilt node is interned
    /// via [`make_node`](Self::make_node) — no shared borrow is ever live when `make_node` may take its
    /// exclusive borrow (the discipline the [`LocalCell`](super::manager_cell::LocalCell)'s `RefCell`
    /// requires and the [`SyncCell`](super::manager_cell::SyncCell)'s `RwLock` requires). Evaluated
    /// with an explicit work-stack rather than recursion, so a tall BDD cannot overflow the call
    /// stack.
    ///
    /// Unlike [`compose_map`](Self::compose_map) there is **no** guarded structural fast path and no
    /// ITE recombination: fixing a variable to a constant only *shrinks* the support, it can never
    /// hoist a variable, so at every `Build` both restricted children keep top variables strictly
    /// greater than `v` and the bare `make_node` preserves ordering and canonicity. (compose_map's
    /// guard exists only because substituted *functions* can hoist a variable to or above the node
    /// being rebuilt.)
    fn restrict_to(&self, node: NodeId, assignment: &Minterm<Anonymous>) -> NodeId {
        /// One unit of work. `Visit` reads a node's shape and schedules its restriction; `Forward`
        /// copies a mapped variable's chosen cofactor result onto the node that tested it; `Build`
        /// re-interns a node that tests an unmapped variable once both restricted children are
        /// resolved.
        enum Work {
            Visit(NodeId),
            Forward {
                node: NodeId,
                child: NodeId,
            },
            Build {
                node: NodeId,
                var: VarId,
                low: NodeId,
                high: NodeId,
            },
        }

        let mut memo: HashMap<NodeId, NodeId> = HashMap::new();
        let mut stack = vec![Work::Visit(node)];
        while let Some(work) = stack.pop() {
            match work {
                Work::Visit(n) => {
                    if memo.contains_key(&n) {
                        continue;
                    }
                    // Read this node's shape under one short-lived shared borrow, dropped before any
                    // make_node may take the exclusive borrow.
                    let shape = {
                        let manager = self.read();
                        match manager.expect_node(n) {
                            // Terminals carry no variable: restricting cannot change a constant.
                            BddNode::Terminal(_) => None,
                            BddNode::Decision { var: v, low, high } => Some((*v, *low, *high)),
                        }
                    };
                    match shape {
                        None => {
                            memo.insert(n, n);
                        }
                        Some((v, low, high)) => {
                            // `assignment` is positional over VarId; a variable past its length or with
                            // a `None` field is left free.
                            let fixed = if v < assignment.num_vars() {
                                assignment.value_at(v)
                            } else {
                                None
                            };
                            match fixed {
                                Some(value) => {
                                    // This node tests a fixed variable: collapse to the matching
                                    // cofactor and *continue* restricting it — deeper nodes may test
                                    // OTHER fixed variables, so the chosen child must still be walked.
                                    let chosen = if value { high } else { low };
                                    stack.push(Work::Forward {
                                        node: n,
                                        child: chosen,
                                    });
                                    stack.push(Work::Visit(chosen));
                                }
                                None => {
                                    // `v` is not fixed: restrict both children, then re-intern.
                                    stack.push(Work::Build {
                                        node: n,
                                        var: v,
                                        low,
                                        high,
                                    });
                                    stack.push(Work::Visit(high));
                                    stack.push(Work::Visit(low));
                                }
                            }
                        }
                    }
                }
                Work::Forward { node: n, child } => {
                    // A shared node can be scheduled more than once; the first result wins.
                    if memo.contains_key(&n) {
                        continue;
                    }
                    let result = *memo
                        .get(&child)
                        .expect("forwarded cofactor restricted before combine");
                    memo.insert(n, result);
                }
                Work::Build {
                    node: n,
                    var: v,
                    low,
                    high,
                } => {
                    if memo.contains_key(&n) {
                        continue;
                    }
                    // If neither child changed, `make_node` returns the canonical id for the same
                    // triple, so an unaffected subgraph rebuilds to itself (the no-op guarantee).
                    let new_low = *memo.get(&low).expect("low child restricted before combine");
                    let new_high = *memo
                        .get(&high)
                        .expect("high child restricted before combine");
                    let result = self.make_node(v, new_low, new_high);
                    memo.insert(n, result);
                }
            }
        }

        *memo
            .get(&node)
            .expect("root node restricted after the iterative walk")
    }

    /// Functional substitution: `f[var := g]` — replace every test of `var` in `f` with the
    /// function `g`, managing the cell's borrow itself.
    ///
    /// This is **not** `ite(g, restrict(f, var, true), restrict(f, var, false))` composed from
    /// separate passes: it is a single fused traversal over the `(f, g)` node pair that computes
    /// the same result in one walk, sharing its ITE-shaped merge step with [`ite`](Self::ite)
    /// instead of calling it as a black box.
    ///
    /// Evaluated **iteratively** with an explicit work-stack, in the same style as the iterative
    /// [`ite`](Self::ite) and [`restrict_to`](Self::restrict_to), so a tall BDD (deep variable ordering)
    /// cannot overflow the call stack. Each step takes its own short-lived shared borrow to read
    /// (resolve/expand a pair, or read a splice's resolved ITE triple) and, only when a result must
    /// be committed, a separate short-lived exclusive borrow to intern the node and record its
    /// cache entries — a shared borrow is never live when the exclusive borrow may be taken, the
    /// discipline the [`LocalCell`](super::manager_cell::LocalCell)'s `RefCell` requires (panics on
    /// overlap) and the [`SyncCell`](super::manager_cell::SyncCell)'s `RwLock` requires (deadlocks
    /// on overlap).
    ///
    /// The walk co-cofactors `f` and `g` together, splitting on their global minimum tested
    /// variable, until it reaches a node of `f` that tests `var` itself. At that point `var`'s
    /// subtree needs no further composition (an ordered BDD cannot re-test a variable below where
    /// it already appears), so the low/high children of that node are spliced in directly via
    /// `ite(g, f_high, f_low)` — composing `f` at `var` with `g` is exactly selecting `f`'s `var =
    /// true` branch where `g` holds and its `var = false` branch where `g` doesn't, which is what
    /// `ite` computes. That inline ITE is driven by the very same `ite_solve_step` /
    /// `ite_combine_step` helpers `ite` itself uses, scheduled as extra work items on this
    /// traversal's stack rather than through a nested call to `ite`, so the whole computation stays
    /// one iterative loop.
    ///
    /// **Ordering lemma:** splitting on `top = min(var(f), var(g))` and co-cofactoring both `f` and
    /// `g` on `top` (a side that doesn't test `top` cofactors to itself) guarantees each child pair
    /// still supports only variables strictly greater than `top` — including when `g` interleaves
    /// with or sits above `var`, or itself tests `var`. So `insert_node`'s ordering precondition
    /// holds at every `Combine`, and the splice's inline `ite(g, f_high, f_low)` is well founded
    /// even though `g` is used whole (untouched by composition) at the splice point.
    ///
    /// **Canonicity:** every node produced here — by `Combine`'s `insert_node` call and by the
    /// splice's inline `ite` — is minted through the same hash-consed `make_node`/`insert_node`
    /// path every other operation uses, so results remain canonical and safely comparable /
    /// cacheable by NodeId alone.
    ///
    /// **Memoisation** has three tiers: the persistent `compose_cache` (`(f, var, g) ->
    /// f[var := g]`, shared across calls, checked first), a per-call `HashMap<(NodeId, NodeId),
    /// NodeId>` pair memo keyed on `(f, g)` (`var` is constant for the whole traversal, so it is
    /// omitted from the key) that collapses the shared sub-DAG within this walk, and the inline
    /// ITE's own `ite_cache` (seeded at each splice so a later top-level `ite` call, or another
    /// `compose` that reaches the same splice, hits the cache directly).
    fn compose(&self, f: NodeId, var: VarId, g: NodeId) -> NodeId {
        /// One unit of work. `Solve` resolves a compose pair (expanding it, or scheduling a
        /// splice, as needed); `Combine` runs after a pair's two structural children are resolved
        /// and builds the result node for it; `Splice` runs after the inline ITE for a `var`-node's
        /// children has resolved and write-through-caches the result; `IteSolve`/`IteCombine` are
        /// the shared ITE machine's steps, driving the splice's `ite(g, f_high, f_low)` on this
        /// same stack.
        enum Work {
            Solve(NodeId, NodeId),
            Combine {
                pair: (NodeId, NodeId),
                top: VarId,
                low: (NodeId, NodeId),
                high: (NodeId, NodeId),
            },
            Splice {
                pair: (NodeId, NodeId),
                triple: NodeTriple,
            },
            IteSolve(NodeId, NodeId, NodeId),
            IteCombine {
                triple: NodeTriple,
                top_var: VarId,
                low: NodeTriple,
                high: NodeTriple,
            },
        }

        /// What a `Solve` step's single shared-borrow read block decided, to be acted on after the
        /// borrow is dropped.
        enum Action {
            Done(NodeId),
            Splice(NodeId, NodeId),
            Expand(VarId, (NodeId, NodeId), (NodeId, NodeId)),
        }

        let mut memo: HashMap<(NodeId, NodeId), NodeId> = HashMap::new();
        let mut stack = vec![Work::Solve(f, g)];
        while let Some(work) = stack.pop() {
            match work {
                Work::Solve(f, g) => {
                    if memo.contains_key(&(f, g)) {
                        continue;
                    }
                    // Read this pair's shape under one short-lived shared borrow, dropped before
                    // any push/write below.
                    let action = {
                        let manager = self.read();
                        if let Some(&r) = manager.compose_cache.get(&(f, var, g)) {
                            Action::Done(r)
                        } else {
                            let f_node = manager.expect_node(f);
                            let top_f = node_var(f_node);
                            if top_f > var {
                                // `f` doesn't reach `var` on this branch (this also covers `f`
                                // being a terminal, whose node_var is usize::MAX) — composition is
                                // a no-op. Not written to compose_cache, matching how `ite` treats
                                // its own terminal cases.
                                Action::Done(f)
                            } else if top_f == var {
                                // `f` tests `var` here. An ordered BDD cannot re-test `var` below
                                // this point, so the children need no further composition — splice
                                // them in verbatim via an inline `ite(g, f_high, f_low)`. `g` is
                                // used whole, even if it sits above `var` or tests `var` itself;
                                // the ordering lemma on this doc covers both.
                                match f_node {
                                    BddNode::Decision { low, high, .. } => {
                                        Action::Splice(*low, *high)
                                    }
                                    BddNode::Terminal(_) => unreachable!(
                                        "terminal node cannot match a real substituted variable"
                                    ),
                                }
                            } else {
                                // Still above `var` on both sides: split on the global minimum and
                                // co-cofactor both `f` and `g` (a side not testing `top` cofactors
                                // to itself).
                                let g_node = manager.expect_node(g);
                                let top_g = node_var(g_node);
                                let top = top_f.min(top_g);
                                let (f_low, f_high) = cofactors(f_node, top_f, top, f);
                                let (g_low, g_high) = cofactors(g_node, top_g, top, g);
                                Action::Expand(top, (f_low, g_low), (f_high, g_high))
                            }
                        }
                    };
                    match action {
                        Action::Done(r) => {
                            memo.insert((f, g), r);
                        }
                        Action::Splice(f_low, f_high) => {
                            // Combine pushed first → pops last, after the inline ITE resolves it.
                            stack.push(Work::Splice {
                                pair: (f, g),
                                triple: (g, f_high, f_low),
                            });
                            stack.push(Work::IteSolve(g, f_high, f_low));
                        }
                        Action::Expand(top, low, high) => {
                            // Combine pushed first → pops last (LIFO).
                            stack.push(Work::Combine {
                                pair: (f, g),
                                top,
                                low,
                                high,
                            });
                            stack.push(Work::Solve(high.0, high.1));
                            stack.push(Work::Solve(low.0, low.1));
                        }
                    }
                }
                Work::Combine {
                    pair,
                    top,
                    low,
                    high,
                } => {
                    if memo.contains_key(&pair) {
                        continue;
                    }
                    let low_id = *memo
                        .get(&low)
                        .expect("compose child resolved before combine - BDD scheduling bug");
                    let high_id = *memo
                        .get(&high)
                        .expect("compose child resolved before combine - BDD scheduling bug");
                    // Commit under one short-lived exclusive borrow: intern the node and record
                    // its compose_cache entry as one transaction, re-checking in case another
                    // thread committed it meanwhile.
                    let key = (pair.0, var, pair.1);
                    let result = {
                        let mut mgr = self.write();
                        match mgr.compose_cache.get(&key) {
                            Some(&r) => r,
                            None => {
                                let r = mgr.insert_node(top, low_id, high_id);
                                mgr.compose_cache.insert(key, r);
                                r
                            }
                        }
                    };
                    memo.insert(pair, result);
                }
                Work::Splice { pair, triple } => {
                    if memo.contains_key(&pair) {
                        continue;
                    }
                    // The inline ITE for this splice has already run to completion on the stack
                    // (scheduled before this Splice item), so its triple is resolved.
                    let r = self
                        .read()
                        .ite_resolved(triple.0, triple.1, triple.2)
                        .expect("splice ITE resolved before compose combine - BDD scheduling bug");
                    // Write-through the compose_cache entry for this pair and seed the ite_cache
                    // for the triple, as one exclusive transaction.
                    {
                        let mut mgr = self.write();
                        mgr.compose_cache.entry((pair.0, var, pair.1)).or_insert(r);
                        mgr.ite_cache.entry(triple).or_insert(r);
                    }
                    memo.insert(pair, r);
                }
                Work::IteSolve(a, b, c) => {
                    let Some((top_var, low, high)) = ite_solve_step(self, a, b, c) else {
                        continue;
                    };
                    stack.push(Work::IteCombine {
                        triple: (a, b, c),
                        top_var,
                        low,
                        high,
                    });
                    stack.push(Work::IteSolve(high.0, high.1, high.2));
                    stack.push(Work::IteSolve(low.0, low.1, low.2));
                }
                Work::IteCombine {
                    triple,
                    top_var,
                    low,
                    high,
                } => ite_combine_step(self, triple, top_var, low, high),
            }
        }

        *memo.get(&(f, g)).expect(
            "top-level compose pair resolved after iterative evaluation - BDD scheduling bug",
        )
    }

    /// Simultaneous multi-variable substitution: `f[v1 := g1, v2 := g2, ...]` for every `(v,
    /// g)` in `map`, managing the cell's borrow itself.
    ///
    /// This is **not** repeated single-variable [`compose`](Self::compose) calls: it walks `f`'s
    /// original graph exactly once. Each original node's variable is substituted exactly once, at
    /// its own level; the substituting functions `g_v` never get traversed themselves — they enter
    /// the computation only as ITE selectors at the level of the variable they replace. That is
    /// what makes the substitution simultaneous rather than sequential: composing with a swap map
    /// `{x := y, y := x}` exchanges the two variables in one pass, which sequential composition
    /// (`f[x := y]` then `[y := x]`) cannot do (the first step would already have destroyed the
    /// original `x`-tests that the second step needs to swap in).
    ///
    /// Evaluated **iteratively** with an explicit work-stack, in the same style as
    /// [`ite`](Self::ite), [`restrict_to`](Self::restrict_to), and [`compose`](Self::compose), so a tall
    /// BDD cannot overflow the call stack. Each step takes its own short-lived shared borrow to
    /// read (a node's shape, or a resolved ITE triple) and, only when a result must be committed, a
    /// separate short-lived exclusive borrow to intern the node — a shared borrow is never live
    /// when the exclusive borrow may be taken, the discipline the
    /// [`LocalCell`](super::manager_cell::LocalCell)'s `RefCell` requires (panics on overlap) and
    /// the [`SyncCell`](super::manager_cell::SyncCell)'s `RwLock` requires (deadlocks on overlap).
    ///
    /// At a node testing an unmapped variable, both children are recombined through a **guarded**
    /// structural fast path: if both composed children still test variables strictly greater than
    /// `v` (the ordering `make_node` requires), they are re-interned directly with `v` as the
    /// splitting variable. That guard is not optional — a substitution applied further down the
    /// graph can hoist a variable to or above `v` (e.g. mapping some `w > v` to a function of `v`
    /// itself), and a bare `make_node(v, low, high)` in that case would either violate ordering or
    /// mint a non-canonical node. When the guard fails, the two children are recombined instead
    /// through `ite(v_projection, high, low)`, where `v_projection = make_node(v, FALSE, TRUE)` is
    /// `v` reintroduced as its own two-valued selector — correct regardless of where the composed
    /// children's variables now sit, at the cost of running the shared inline ITE machine
    /// (`ite_solve_step`/`ite_combine_step`, the same steps [`ite`](Self::ite) and
    /// [`compose`](Self::compose) use) instead of a single `make_node` call. A node testing a
    /// *mapped* variable `v` always takes this ITE recombination path, with the substituting
    /// function `g_v` itself as the selector: `ite(g_v, high, low)`.
    ///
    /// **Canonicity:** every node produced here — by the guarded fast path's `make_node` and by the
    /// ITE recombination's `insert_node` — is minted through the same hash-consed
    /// `make_node`/`insert_node` path every other operation uses, so results remain canonical and
    /// safely comparable by NodeId alone.
    ///
    /// **Memoisation** is a per-call `HashMap<NodeId, NodeId>` keyed on `f`'s original node id only
    /// (there is no persistent cache analogous to `compose_cache`: `map` is not a stable,
    /// hashable, reusable key the way a single `(var, g)` pair is). The per-call memo still
    /// collapses `f`'s shared sub-DAG so the walk stays linear in the number of `f`'s distinct
    /// reachable nodes, and the inline ITE recombinations still hit the persistent `ite_cache`
    /// across calls.
    fn compose_map(&self, f: NodeId, map: &HashMap<VarId, NodeId>) -> NodeId {
        /// One unit of work. `Solve` reads an original `f`-node's shape and schedules its
        /// children; `Combine` runs after a node's two original children are resolved (composed)
        /// and either rebuilds the node directly (guarded fast path) or schedules the ITE
        /// recombination; `Finish` reads back a recombination's resolved ITE triple and records it
        /// as the node's result; `IteSolve`/`IteCombine` are the shared ITE machine's steps,
        /// driving the recombination `ite(selector, high, low)` on this same stack.
        enum Work {
            Solve(NodeId),
            Combine {
                node: NodeId,
                var: VarId,
                low: NodeId,
                high: NodeId,
            },
            Finish {
                node: NodeId,
                triple: NodeTriple,
            },
            IteSolve(NodeId, NodeId, NodeId),
            IteCombine {
                triple: NodeTriple,
                top_var: VarId,
                low: NodeTriple,
                high: NodeTriple,
            },
        }

        let mut memo: HashMap<NodeId, NodeId> = HashMap::new();
        let mut stack = vec![Work::Solve(f)];
        while let Some(work) = stack.pop() {
            match work {
                Work::Solve(n) => {
                    if memo.contains_key(&n) {
                        continue;
                    }
                    // Read this node's shape under one short-lived shared borrow, dropped before
                    // any push/write below.
                    let shape = {
                        let manager = self.read();
                        match manager.expect_node(n) {
                            // Terminals carry no variable: composition cannot change a constant.
                            BddNode::Terminal(_) => None,
                            BddNode::Decision { var: v, low, high } => Some((*v, *low, *high)),
                        }
                    };
                    match shape {
                        None => {
                            memo.insert(n, n);
                        }
                        Some((v, low, high)) => {
                            // Schedule both original children and a Combine that runs once they're
                            // resolved (Combine pushed first → pops last, LIFO).
                            stack.push(Work::Combine {
                                node: n,
                                var: v,
                                low,
                                high,
                            });
                            stack.push(Work::Solve(high));
                            stack.push(Work::Solve(low));
                        }
                    }
                }
                Work::Combine {
                    node: n,
                    var: v,
                    low,
                    high,
                } => {
                    if memo.contains_key(&n) {
                        continue;
                    }
                    // The two children have already been composed from f's original subgraph.
                    let e = *memo.get(&low).expect(
                        "compose_map low child resolved before combine - BDD scheduling bug",
                    );
                    let t = *memo.get(&high).expect(
                        "compose_map high child resolved before combine - BDD scheduling bug",
                    );
                    match map.get(&v) {
                        Some(&g_v) => {
                            // `v` is substituted: splice in via ite(g_v, high, low) — g_v enters
                            // only as a selector, never traversed.
                            stack.push(Work::Finish {
                                node: n,
                                triple: (g_v, t, e),
                            });
                            stack.push(Work::IteSolve(g_v, t, e));
                        }
                        None => {
                            // `v` is unmapped: try the guarded structural fast path first.
                            let safe = {
                                let mgr = self.read();
                                node_var(mgr.expect_node(e)) > v && node_var(mgr.expect_node(t)) > v
                            };
                            if safe {
                                let r = self.make_node(v, e, t);
                                memo.insert(n, r);
                            } else {
                                // A substitution below hoisted a variable to or above `v`: fall
                                // back to recombining through v's own projection as a selector.
                                let proj = self.make_node(v, FALSE_NODE, TRUE_NODE);
                                stack.push(Work::Finish {
                                    node: n,
                                    triple: (proj, t, e),
                                });
                                stack.push(Work::IteSolve(proj, t, e));
                            }
                        }
                    }
                }
                Work::Finish { node: n, triple } => {
                    if memo.contains_key(&n) {
                        continue;
                    }
                    // The inline ITE for this recombination has already run to completion on the
                    // stack (scheduled before this Finish item), so its triple is resolved.
                    let r = self
                        .read()
                        .ite_resolved(triple.0, triple.1, triple.2)
                        .expect("compose_map merge ITE resolved - BDD scheduling bug");
                    memo.insert(n, r);
                }
                Work::IteSolve(a, b, c) => {
                    let Some((top_var, low, high)) = ite_solve_step(self, a, b, c) else {
                        continue;
                    };
                    stack.push(Work::IteCombine {
                        triple: (a, b, c),
                        top_var,
                        low,
                        high,
                    });
                    stack.push(Work::IteSolve(high.0, high.1, high.2));
                    stack.push(Work::IteSolve(low.0, low.1, low.2));
                }
                Work::IteCombine {
                    triple,
                    top_var,
                    low,
                    high,
                } => ite_combine_step(self, triple, top_var, low, high),
            }
        }

        *memo.get(&f).expect(
            "top-level compose_map node resolved after iterative evaluation - BDD scheduling bug",
        )
    }
}

impl<C: ManagerCell> BddOps for C {}

/// Get the variable of a node (usize::MAX for terminals)
fn node_var(node: &BddNode) -> VarId {
    match node {
        BddNode::Terminal(_) => usize::MAX,
        BddNode::Decision { var, .. } => *var,
    }
}

/// Get cofactors (low and high children) for Shannon expansion
fn cofactors(
    node: &BddNode,
    node_var: VarId,
    split_var: VarId,
    node_id: NodeId,
) -> (NodeId, NodeId) {
    if node_var == split_var {
        match node {
            BddNode::Decision { low, high, .. } => (*low, *high),
            // A terminal's `node_var` is `usize::MAX`; `split_var` is always a real variable,
            // so `node_var == split_var` cannot hold for a terminal node.
            BddNode::Terminal(_) => {
                unreachable!("terminal node cannot match a real split variable")
            }
        }
    } else {
        // Variable doesn't appear in this branch
        (node_id, node_id)
    }
}

/// Resolve-or-expand one ITE triple under a single short-lived shared borrow.
///
/// `None` when `(f, g, h)` is already resolved (a terminal rule or an `ite_cache` hit);
/// `Some((top_var, low_triple, high_triple))` when it needs its two child triples solved and then a
/// combine. This is `ite`'s Solve step, shared with the compose engines' embedded ITE machines. It
/// reads through a single short-lived shared borrow it acquires and releases itself, calling the
/// [`BddManager`] leaf methods [`ite_resolved`](BddManager::ite_resolved) /
/// [`ite_expand`](BddManager::ite_expand) on the resulting guard.
fn ite_solve_step<C: ManagerCell>(cell: &C, f: NodeId, g: NodeId, h: NodeId) -> Option<IteSplit> {
    let manager = cell.read();
    if manager.ite_resolved(f, g, h).is_some() {
        None
    } else {
        Some(manager.ite_expand(f, g, h))
    }
}

/// Combine one ITE triple once its two child triples are resolved: read the children under a shared
/// borrow (skipping if the triple is already cached — a diamond can schedule the same combine twice),
/// then intern the node and record its `ite_cache` entry in one exclusive transaction. This is `ite`'s
/// Combine step, shared with the compose engines. Both borrows are acquired and released here; the
/// shared borrow is dropped before the exclusive one is taken, so the two never overlap.
fn ite_combine_step<C: ManagerCell>(
    cell: &C,
    triple: NodeTriple,
    top_var: VarId,
    low: NodeTriple,
    high: NodeTriple,
) {
    // Read the resolved children under one short-lived shared borrow. A diamond can
    // schedule the same Combine twice; the first caches the result, so skip if it is
    // already there.
    let children = {
        let manager = cell.read();
        if manager.ite_cache.contains_key(&triple) {
            None
        } else {
            let low_id = manager
                .ite_resolved(low.0, low.1, low.2)
                .expect("ITE low child unresolved at combine time - BDD scheduling bug");
            let high_id = manager
                .ite_resolved(high.0, high.1, high.2)
                .expect("ITE high child unresolved at combine time - BDD scheduling bug");
            Some((low_id, high_id))
        }
    };
    let Some((low_id, high_id)) = children else {
        return;
    };
    // Commit under a separate short-lived exclusive borrow — taken only after the shared
    // borrow above has been dropped, so the two never overlap. Intern the node and record
    // its cache entry as one transaction, re-checking in case another thread committed it
    // meanwhile.
    let mut manager = cell.write();
    if !manager.ite_cache.contains_key(&triple) {
        let result = manager.insert_node(top_var, low_id, high_id);
        manager.ite_cache.insert(triple, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bdd::manager_cell::{LocalCell, SyncCell};

    /// Build `(a & b) | (a ^ c)` over an arbitrary cell, exercising `make_var`, `make_node`, `ite`, and
    /// `xor` and returning the root and the variable arity. Used to assert both cells produce the same
    /// canonical structure through the *single* generic engine.
    fn build_sample<C: ManagerCell>(cell: &C) -> (NodeId, usize) {
        let a = cell.make_var("a");
        let b = cell.make_var("b");
        let c = cell.make_var("c");
        let a_node = cell.make_node(a, FALSE_NODE, TRUE_NODE);
        let b_node = cell.make_node(b, FALSE_NODE, TRUE_NODE);
        let c_node = cell.make_node(c, FALSE_NODE, TRUE_NODE);

        let and = cell.ite(a_node, b_node, FALSE_NODE);
        let xor = cell.xor(a_node, c_node);
        let or = cell.ite(and, TRUE_NODE, xor);
        (or, cell.read().id_to_var.len())
    }

    /// The `RefCell`-backed [`LocalCell`] must drive the engine without panicking — the borrow
    /// discipline (no shared borrow live when an exclusive borrow is taken) is the riskiest part of the
    /// engine abstraction, and a `RefCell` panics on overlap where the `RwLock` would deadlock.
    #[test]
    fn engine_runs_on_local_cell() {
        let cell = LocalCell::new_empty();
        let (root, arity) = build_sample(&cell);
        assert_eq!(arity, 3);
        // Re-deriving an identical expression must hit the caches and yield the same canonical root.
        let (root2, _) = build_sample(&cell);
        assert_eq!(root, root2);
    }

    /// The `RwLock`-backed [`SyncCell`] must produce the *same* canonical root and arity as the
    /// `LocalCell` — the two cells share one generic engine, so structure must be identical.
    #[test]
    fn both_cells_agree() {
        let local = LocalCell::new_empty();
        let sync = SyncCell::new_empty();
        let (local_root, local_arity) = build_sample(&local);
        let (sync_root, sync_arity) = build_sample(&sync);
        assert_eq!(local_root, sync_root);
        assert_eq!(local_arity, sync_arity);
    }

    /// Build a positional assignment over `n` VarIds for [`BddOps::restrict_to`]: each `(v, b)` in
    /// `fixed` pins VarId `v` to `b`; every other position is left free (`None`).
    fn assign(n: usize, fixed: &[(VarId, bool)]) -> Minterm<Anonymous> {
        let mut vals = vec![None; n];
        for &(v, b) in fixed {
            vals[v] = Some(b);
        }
        Minterm::anonymous(&vals)
    }

    /// `restrict_to` must implement Shannon cofactor by assignment over the engine: `(a & b)|a=1 == b`,
    /// `(a & b)|a=0 == FALSE`, and restricting an absent variable is a no-op. Driven over the
    /// `RefCell`-backed [`LocalCell`] to exercise the borrow discipline of the iterative walk.
    #[test]
    fn restrict_cofactors_on_local_cell() {
        let cell = LocalCell::new_empty();
        let a = cell.make_var("a");
        let b = cell.make_var("b");
        let a_node = cell.make_node(a, FALSE_NODE, TRUE_NODE);
        let b_node = cell.make_node(b, FALSE_NODE, TRUE_NODE);
        let and = cell.ite(a_node, b_node, FALSE_NODE); // a & b

        // (a & b)|a=true == b
        assert_eq!(cell.restrict_to(and, &assign(2, &[(a, true)])), b_node);
        // (a & b)|a=false == FALSE
        assert_eq!(cell.restrict_to(and, &assign(2, &[(a, false)])), FALSE_NODE);

        // Restricting a variable absent from the function is a no-op.
        let c = cell.make_var("c");
        assert_eq!(cell.restrict_to(and, &assign(3, &[(c, true)])), and);
    }

    /// `restrict_many` must agree with chaining single-variable `restrict` over the same
    /// assignments, in any chaining order and for every partial subset of the fixed variables.
    /// Exercised over all 27 `{a, b, c}` partial assignments (each variable fixed to true, false,
    /// or left free) against `f = (a & b) | (a ^ c)`.
    #[test]
    fn restrict_many_agrees_with_chained_restrict() {
        let cell = LocalCell::new_empty();
        let (f, _) = build_sample(&cell);
        let a = cell.read().var_id("a").unwrap();
        let b = cell.read().var_id("b").unwrap();
        let c = cell.read().var_id("c").unwrap();

        // Each variable independently: free (None), fixed false, or fixed true.
        let states = [None, Some(false), Some(true)];
        for &sa in &states {
            for &sb in &states {
                for &sc in &states {
                    let fixed: Vec<(VarId, bool)> = [(a, sa), (b, sb), (c, sc)]
                        .into_iter()
                        .filter_map(|(id, s)| s.map(|v| (id, v)))
                        .collect();
                    let many = cell.restrict_to(f, &assign(3, &fixed));

                    // Chain single-variable restricts in a→b→c order, skipping free variables.
                    let chained_abc = [(a, sa), (b, sb), (c, sc)]
                        .into_iter()
                        .filter_map(|(id, s)| s.map(|v| (id, v)))
                        .fold(f, |acc, (id, v)| {
                            cell.restrict_to(acc, &assign(3, &[(id, v)]))
                        });
                    // ...and in the reverse c→b→a order: order must not matter.
                    let chained_cba = [(c, sc), (b, sb), (a, sa)]
                        .into_iter()
                        .filter_map(|(id, s)| s.map(|v| (id, v)))
                        .fold(f, |acc, (id, v)| {
                            cell.restrict_to(acc, &assign(3, &[(id, v)]))
                        });

                    assert_eq!(many, chained_abc);
                    assert_eq!(many, chained_cba);
                }
            }
        }
    }

    /// A full assignment over the support must collapse to a terminal matching the truth table and,
    /// crucially, must intern **no new node** — restrict only ever reduces, so the node table length
    /// is unchanged before and after (the zero-write witness).
    #[test]
    fn restrict_many_full_assignment_zero_new_nodes() {
        let cell = LocalCell::new_empty();
        let (f, _) = build_sample(&cell); // (a & b) | (a ^ c)
        let a = cell.read().var_id("a").unwrap();
        let b = cell.read().var_id("b").unwrap();
        let c = cell.read().var_id("c").unwrap();

        for av in [false, true] {
            for bv in [false, true] {
                for cv in [false, true] {
                    let expected = (av && bv) || (av ^ cv);
                    let assignment = assign(3, &[(a, av), (b, bv), (c, cv)]);

                    let nodes_before = cell.read().nodes.len();
                    let result = cell.restrict_to(f, &assignment);
                    assert_eq!(cell.read().nodes.len(), nodes_before);
                    assert_eq!(result, if expected { TRUE_NODE } else { FALSE_NODE });
                }
            }
        }
    }

    /// `restrict_to` must be iterative: fixing the *bottom* variable of a very deep AND chain walks
    /// through every node above it, which a recursive implementation would overflow on. The chain is
    /// built directly with `make_node` bottom-up (O(n)) so it is deep without the O(n^2) cost of
    /// folding it with `ite`.
    #[test]
    fn restrict_deep_chain_no_overflow() {
        let cell = LocalCell::new_empty();
        let n = 50_000;
        let ids: Vec<VarId> = (0..n).map(|i| cell.make_var(&format!("v{i}"))).collect();
        // f = v0 & v1 & ... & v(n-1), built bottom-up: each node's low = FALSE, high = the child.
        let mut node = TRUE_NODE;
        for &id in ids.iter().rev() {
            node = cell.make_node(id, FALSE_NODE, node);
        }
        // Fixing the bottom variable to false collapses the whole conjunction to false; the walk
        // descends through all n-1 nodes above it without overflowing the stack.
        assert_eq!(
            cell.restrict_to(node, &assign(n, &[(ids[n - 1], false)])),
            FALSE_NODE
        );
        // Fixing it to true drops just that variable, leaving a still-non-constant conjunction.
        let dropped = cell.restrict_to(node, &assign(n, &[(ids[n - 1], true)]));
        assert_ne!(dropped, FALSE_NODE);
        assert_ne!(dropped, TRUE_NODE);
    }

    /// An all-free assignment is a no-op, and an assignment pinning only variables absent from the
    /// function is likewise a no-op — restricting nothing the function tests leaves it unchanged.
    #[test]
    fn restrict_empty_and_absent() {
        let cell = LocalCell::new_empty();
        let (f, _) = build_sample(&cell);

        // All-free assignment: no-op.
        assert_eq!(cell.restrict_to(f, &assign(3, &[])), f);

        // Assignment pinning only a fresh variable the function never tests: no-op.
        let d = cell.make_var("d");
        assert_eq!(cell.restrict_to(f, &assign(4, &[(d, true)])), f);
    }

    /// A deeply nested chain must not overflow the stack on either cell (the engine is iterative) and
    /// must not trip the `RefCell`'s borrow discipline.
    #[test]
    fn deep_chain_on_local_cell() {
        let cell = LocalCell::new_empty();
        let names: Vec<String> = (0..400).map(|i| format!("v{i}")).collect();
        let mut acc = {
            let id = cell.make_var(&names[0]);
            cell.make_node(id, FALSE_NODE, TRUE_NODE)
        };
        for name in &names[1..] {
            let id = cell.make_var(name);
            let node = cell.make_node(id, FALSE_NODE, TRUE_NODE);
            acc = cell.ite(acc, node, FALSE_NODE); // acc & node
        }
        assert_ne!(acc, FALSE_NODE);
    }

    /// `compose` must implement functional substitution: composing `(a & b)` with `b := c` must equal
    /// `ite(a, c, FALSE)` (the same result as building `a & c` directly). Composing with a variable
    /// absent from the function is a no-op, and composing with the constant `TRUE` must degenerate to
    /// `restrict(.., true)` (substituting a variable with the constant true is exactly setting it true).
    #[test]
    fn compose_substitutes_on_local_cell() {
        let cell = LocalCell::new_empty();
        let a = cell.make_var("a");
        let b = cell.make_var("b");
        let c = cell.make_var("c");
        let a_node = cell.make_node(a, FALSE_NODE, TRUE_NODE);
        let b_node = cell.make_node(b, FALSE_NODE, TRUE_NODE);
        let c_node = cell.make_node(c, FALSE_NODE, TRUE_NODE);
        let f = cell.ite(a_node, b_node, FALSE_NODE); // a & b

        // (a & b)[b := c] == a & c == ite(a, c, FALSE)
        let composed = cell.compose(f, b, c_node);
        let oracle = cell.ite(a_node, c_node, FALSE_NODE);
        assert_eq!(composed, oracle);

        // Composing a variable absent from f is a no-op.
        let d = cell.make_var("d");
        assert_eq!(cell.compose(f, d, c_node), f);

        // Composing with the constant TRUE degenerates to restricting `b` to true.
        let n = cell.read().id_to_var.len();
        assert_eq!(
            cell.compose(f, b, TRUE_NODE),
            cell.restrict_to(f, &assign(n, &[(b, true)]))
        );
    }

    /// The persistent `compose_cache` must actually be consulted: repeating an identical `compose`
    /// call must return the same canonical id *and* must not allocate any new node — a genuine
    /// cache-hit witness, not just an equal-by-canonicity coincidence.
    #[test]
    fn compose_cache_reuses_nodes() {
        let cell = LocalCell::new_empty();
        let a = cell.make_var("a");
        let b = cell.make_var("b");
        let c = cell.make_var("c");
        let a_node = cell.make_node(a, FALSE_NODE, TRUE_NODE);
        let b_node = cell.make_node(b, FALSE_NODE, TRUE_NODE);
        let c_node = cell.make_node(c, FALSE_NODE, TRUE_NODE);
        let f = cell.ite(a_node, b_node, FALSE_NODE); // a & b

        let first = cell.compose(f, b, c_node);
        let nodes_after_first = cell.read().nodes.len();

        let second = cell.compose(f, b, c_node);
        assert_eq!(first, second);
        assert_eq!(cell.read().nodes.len(), nodes_after_first);
    }

    /// `compose` and `compose_map` must both be iterative: substituting the *bottom* variable of a
    /// very deep AND chain walks through every node above it, which a recursive implementation would
    /// overflow on. Mirrors `restrict_deep_chain_no_overflow`'s chain construction.
    #[test]
    fn compose_deep_chain_no_overflow() {
        let cell = LocalCell::new_empty();
        let n = 50_000;
        let ids: Vec<VarId> = (0..n).map(|i| cell.make_var(&format!("v{i}"))).collect();
        // f = v0 & v1 & ... & v(n-1), built bottom-up: each node's low = FALSE, high = the child.
        let mut node = TRUE_NODE;
        for &id in ids.iter().rev() {
            node = cell.make_node(id, FALSE_NODE, node);
        }
        let fresh = cell.make_var("fresh");
        let fresh_node = cell.make_node(fresh, FALSE_NODE, TRUE_NODE);

        // Substituting the bottom variable with FALSE collapses the whole conjunction to false; the
        // walk descends through all n-1 nodes above it without overflowing the stack.
        assert_eq!(cell.compose(node, ids[n - 1], FALSE_NODE), FALSE_NODE);
        // Substituting it with a fresh variable's node leaves a still-non-constant conjunction.
        let substituted = cell.compose(node, ids[n - 1], fresh_node);
        assert_ne!(substituted, FALSE_NODE);
        assert_ne!(substituted, TRUE_NODE);

        // Same two cases through compose_map with a singleton map.
        let mut to_false = HashMap::new();
        to_false.insert(ids[n - 1], FALSE_NODE);
        assert_eq!(cell.compose_map(node, &to_false), FALSE_NODE);

        let mut to_fresh = HashMap::new();
        to_fresh.insert(ids[n - 1], fresh_node);
        let mapped = cell.compose_map(node, &to_fresh);
        assert_ne!(mapped, FALSE_NODE);
        assert_ne!(mapped, TRUE_NODE);
    }

    /// `compose_map` must stay canonical even when a substitution hoists a variable above the node
    /// being rebuilt. `f = b & c` with `map = {c := a}` hoists `a` (ordered before `b`) into `b`'s
    /// subtree, so the guarded structural fast path must fall back to the ITE recombination — the
    /// result must equal the canonical BDD for `a & b` built directly via `ite`, not some
    /// non-canonical restructuring.
    #[test]
    fn compose_map_hoist_stays_canonical() {
        let cell = LocalCell::new_empty();
        let a = cell.make_var("a"); // id 0
        let b = cell.make_var("b"); // id 1
        let c = cell.make_var("c"); // id 2
        let a_node = cell.make_node(a, FALSE_NODE, TRUE_NODE);
        let b_node = cell.make_node(b, FALSE_NODE, TRUE_NODE);
        let c_node = cell.make_node(c, FALSE_NODE, TRUE_NODE);

        let f = cell.make_node(b, FALSE_NODE, c_node); // b & c

        let mut map = HashMap::new();
        map.insert(c, a_node);
        let result = cell.compose_map(f, &map);

        let expected = cell.ite(a_node, b_node, FALSE_NODE); // a & b
        assert_eq!(result, expected);
    }
}
