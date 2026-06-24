//! Boolean expression types with operator overloading and parsing support
//!
//! This module provides a high-level boolean expression representation that can be constructed
//! programmatically using operator overloading, the `expr!` macro, or parsed from strings.
//! Expressions can be minimised directly using the Espresso algorithm.
//!
//! # Main Types
//!
//! - [`BoolExpr`] - A boolean expression supporting three construction methods:
//!   1. **`expr!` macro**: `expr!(a * b + c)` - Recommended
//!   2. Method API: `a.and(&b).or(&c)`
//!   3. Operator overloading: `&a * &b + &c`
//!   
//!   Expressions can be minimised directly with `.minimize()` or `.minimize_exact()`.
//!
//! # Implementation Details
//!
//! Starting in v3.1.1, `BoolExpr` uses Binary Decision Diagrams (BDDs) internally for
//! efficient representation and operations. This is an implementation detail that enables:
//!
//! - **Efficient operations**: Polynomial-time AND/OR/NOT operations
//! - **Canonical form**: Equivalent expressions have identical internal structure
//! - **Memory efficiency**: Structural sharing via global singleton manager
//! - **Automatic simplification**: Redundancy elimination during construction
//!
//! The BDD implementation makes the high-level API practical for complex expressions,
//! but the primary purpose remains **Boolean function minimisation via Espresso**.
//!
//! # Quick Start
//!
//! ## Using the `expr!` Macro (Recommended)
//!
//! The `expr!` macro provides a concise syntax with three usage styles:
//!
//! ```
//! use espresso_logic::{BoolExpr, expr};
//!
//! // Style 1: String literals (most concise - no variable declarations)
//! let xor = expr!("a" * !"b" + !"a" * "b");
//! println!("{}", xor);  // Output: a * ~b + ~a * b
//!
//! // Style 2: Existing BoolExpr variables
//! let a = BoolExpr::variable("a");
//! let b = BoolExpr::variable("b");
//! let expr = expr!(a * b + !a * !b);
//!
//! // Style 3: Mix both
//! let result = expr!(a * "temp" + b);
//! ```
//!
//! ## Parsing from Strings
//!
//! ```
//! use espresso_logic::BoolExpr;
//!
//! # fn main() -> std::io::Result<()> {
//! let expr = BoolExpr::parse("a * b + ~a * ~b")?;
//! let complex = BoolExpr::parse("(a + b) * (c + d)")?;
//! println!("{}", expr);  // Minimal parentheses: a * b + ~a * ~b
//! # Ok(())
//! # }
//! ```
//!
//! ## Minimising and Evaluating
//!
//! ```
//! use espresso_logic::{BoolExpr, expr, Minimizable};
//! use std::collections::HashMap;
//! use std::sync::Arc;
//!
//! # fn main() -> std::io::Result<()> {
//! let a = BoolExpr::variable("a");
//! let b = BoolExpr::variable("b");
//! let c = BoolExpr::variable("c");
//!
//! // Redundant expression
//! let redundant = expr!(a * b + a * b * c);
//!
//! // Evaluate with specific values
//! let mut assignment = HashMap::new();
//! assignment.insert(Arc::from("a"), true);
//! assignment.insert(Arc::from("b"), true);
//! assignment.insert(Arc::from("c"), false);
//! let result = redundant.evaluate(&assignment);
//! assert_eq!(result, true);
//!
//! // Minimise it (returns new minimised instance)
//! let minimised = redundant.minimize()?;
//! println!("Minimised: {}", minimised);  // Output: a * b
//!
//! // Check logical equivalence
//! let redundant2 = expr!(a * b + a * b * c);
//! assert!(redundant2.equivalent_to(&minimised));
//! # Ok(())
//! # }
//! ```
//!
//! # Comprehensive Guide
//!
//! For a complete guide to the Boolean expression API including detailed examples,
//! composition patterns, BDD architecture, performance considerations, and best practices,
//! see the embedded documentation below.
#![doc = include_str!("../../docs/BOOLEAN_EXPRESSIONS.md")]

// Submodules
mod ast;
mod bdd;
mod builder;
mod context;
mod display;
pub mod error;
mod eval;
pub(crate) mod factorization;
mod manager;
mod operators;
mod parser;

pub use error::{ExpressionParseError, ParseBoolExprError};

// Re-export AST types
pub(crate) use ast::BoolExprAst;
pub use ast::ExprNode;
pub use builder::{Bdd, BddBuilder};
pub use context::{BddContext, Brand, Global};

// Re-export manager types for internal use
use crate::Symbol;
use manager::{BddManager, NodeId, Store, FALSE_NODE, TRUE_NODE};

use std::sync::{Arc, OnceLock};

/// A boolean expression for logic minimisation
///
/// `BoolExpr` provides a high-level interface for building and minimising Boolean functions.
/// Expressions can be constructed programmatically, parsed from strings, or composed from
/// existing expressions, then minimised using the Espresso algorithm.
///
/// # Construction Methods
///
/// Several ways to build expressions, all producing the same canonical BDD:
///
/// 1. **`expr!` macro** (recommended) - Clean syntax without explicit references
/// 2. **Method API** - Explicit `.and()`, `.or()`, `.not()`, `.xor()`, `.ite()` calls
/// 3. **Operator overloading** - `*`/`&` (AND), `+`/`|` (OR), `^` (XOR), `!`/`~` (NOT); requires `&`
///    references on the owned forms
/// 4. **String parsing** - [`BoolExpr::parse`] (and `str::parse`) for the same operators in text form
/// 5. **Low-level builder** - [`BoolExpr::build`] composes a whole expression from node handles; see
///    [Implementation Details](#implementation-details) and the [builder example](#low-level-builder)
///
/// # Minimisation
///
/// Expressions support direct minimisation via:
/// - `.minimize()` - Fast heuristic algorithm (~99% optimal)
/// - `.minimize_exact()` - Slower but guaranteed minimal result
///
/// # Brands
///
/// `BoolExpr<B = Global>` is parameterised by a **brand** that selects which BDD manager backs it.
/// The default, `Global`, is the process-global manager that all the free constructors
/// ([`variable`](Self::variable), [`parse`](Self::parse), [`build`](Self::build), the `expr!` macro)
/// use — bare `BoolExpr` means `BoolExpr<Global>`. A scoped [`BddContext`](crate::BddContext) (created
/// with [`bdd_context!`](crate::bdd_context)) mints its own brand and a private, independent manager;
/// expressions of two distinct brands cannot be combined (a compile error). Both storage models are
/// `Arc<RwLock<…>>`, so `BoolExpr` is `Send`/`Sync` for every brand.
///
/// # Implementation Details
///
/// **Internal representation:** Uses Binary Decision Diagrams (BDDs) for canonical form.
///
/// Each `BoolExpr` contains:
/// - **Node ID** - Index into its manager's BDD node table
/// - **Storage handle** - Owned `Arc<RwLock<BddManager>>` (`Global` shares one process-global manager;
///   each scoped context has its own)
/// - **Cube Cache** - Lazily cached cubes for Espresso minimisation
/// - **AST Cache** - Lazily cached syntax tree for display
///
/// Within one brand, the manager provides structural sharing and canonical representation: every
/// operation hash-conses through it.
///
/// **Canonical form.** Because every operation hash-conses through the shared manager, two expressions
/// that denote the same Boolean function have the *same* BDD root. Equality, equivalence, and hashing are
/// therefore O(1) on the root, and structurally different but logically equal inputs (e.g. `a*b` built
/// three different ways, or `a+b` versus `b+a`) compare equal.
///
/// **One construction primitive.** Every constructor and operator funnels through [`BoolExpr::build`].
/// The macro lowers to a `build` closure; the string parser realises its parse through `build`; and
/// `.and()`/`.or()`/`.not()`/`.xor()`/`.ite()` graft their operands into a `build` closure. `build`
/// exposes a [`BddBuilder`] whose methods build [`Bdd`] node handles in the manager.
///
/// # Cloning
///
/// Cloning bumps the storage handle's refcount and copies the node ID. `OnceLock::clone()` copies
/// the cached content (Arc pointers), so clones share the actual cached data. The BDD structure
/// itself is shared via the manager.
///
/// # Thread Safety
///
/// `BoolExpr` is `Send`/`Sync` for every brand: each manager is `Arc<RwLock<…>>`-backed, so multiple
/// threads can safely create and manipulate expressions concurrently. A scoped
/// [`BddContext`](crate::BddContext) gives **isolation and locality** — its own node table, with no
/// lock contention or cache pollution from unrelated global expressions — not a different
/// concurrency model.
///
/// # Memory
///
/// The shared manager's node table and operation caches **grow monotonically and are never evicted**
/// while any `BoolExpr` is alive (node IDs must stay stable for lock-free traversal). The manager is
/// dropped — reclaiming everything — only once the last live `BoolExpr` is dropped. A long-running
/// program that builds very many distinct expressions over its lifetime will therefore see the
/// manager's memory grow until that point; this is intentional, not a leak.
///
/// # Using as a `HashMap` / `HashSet` key
///
/// `BoolExpr` is a sound map key: its `Eq`/`Hash` use only the canonical BDD root (a stable value).
/// Clippy's [`clippy::mutable_key_type`] lint *will* fire at such call sites, because a `BoolExpr`
/// embeds inline `OnceLock` cache cells (the lazily-filled cube and AST caches) — interior mutability
/// directly in the value. (The `Arc`-shared BDD manager does *not* count: interior mutability behind a
/// pointer is fine.) Here the lint is a **false positive**: the hash never reads those caches (only the
/// stable `(manager pointer, root)` pair), so a key's hash cannot change while it sits in the map. It
/// is therefore safe to `#[allow(clippy::mutable_key_type)]` at those sites.
///
/// [`clippy::mutable_key_type`]: https://rust-lang.github.io/rust-clippy/master/index.html#mutable_key_type
///
/// # Examples
///
/// ## Method-based API
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = a.and(&b).or(&a.not().and(&b.not()));
/// println!("{}", expr);  // Uses factored display
/// ```
///
/// ## Using `expr!` macro (recommended)
/// ```
/// use espresso_logic::{BoolExpr, expr};
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// // No & references needed!
/// let expr = expr!(a * b + !a * !b);
/// println!("{}", expr);
/// ```
///
/// ## Low-level builder
/// ```
/// use espresso_logic::BoolExpr;
///
/// // Build (a ^ b) | (b & c) by composing node handles.
/// let expr = BoolExpr::build(|f| {
///     let a = f.var("a");
///     let b = f.var("b");
///     let c = f.var("c");
///     f.or(f.xor(a, b), f.and(b, c))
/// });
///
/// // Identical to the equivalent operator/method expression (canonical BDD).
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let c = BoolExpr::variable("c");
/// assert_eq!(expr, a.xor(&b).or(&b.and(&c)));
/// ```
///
/// The builder shines when the shape is dynamic — e.g. folding a slice of variables — since the whole
/// result is built with one lock and no intermediate `BoolExpr` allocations:
/// ```
/// use espresso_logic::BoolExpr;
///
/// // OR of a runtime-sized set of variables.
/// let names = ["x0", "x1", "x2", "x3"];
/// let any = BoolExpr::build(|f| {
///     names
///         .iter()
///         .map(|n| f.var(n))
///         .reduce(|acc, v| f.or(acc, v))
///         .unwrap_or_else(|| f.constant(false))
/// });
/// assert_eq!(any.var_count(), 4);
///
/// // Existing expressions can be grafted in with `graft`.
/// let sub = BoolExpr::parse("a * b").unwrap();
/// let guarded = BoolExpr::build(|f| {
///     let sub = f.graft(&sub);
///     let c = f.var("c");
///     f.and(sub, c)
/// });
/// assert_eq!(guarded, BoolExpr::parse("a * b * c").unwrap());
/// ```
///
/// ## BDD Operations
/// ```
/// use espresso_logic::BoolExpr;
///
/// let expr = BoolExpr::parse("a * b + b * c").unwrap();
///
/// // Query BDD properties
/// println!("BDD nodes: {}", expr.node_count());
/// println!("Variables: {}", expr.var_count());
///
/// // All operations are BDD operations
/// let vars = expr.collect_variables();
/// println!("Variables: {:?}", vars);
/// ```
pub struct BoolExpr<B: Brand = Global> {
    /// Owned `Arc<RwLock<BddManager>>` handle to this expression's BDD manager. The [`Global`] brand
    /// shares one process-global manager; each scoped [`BddContext`](crate::BddContext) brand has its
    /// own. The brand `B` only distinguishes namespaces — it does not change the storage.
    store: Store,
    /// Root node ID in the BDD
    root: NodeId,
    /// Cached product-term cubes (input minterms) for this BDD.
    /// This avoids expensive BDD traversal when extracting cubes or minimising.
    /// OnceLock's Clone copies the content, so clones share cached data via Arc.
    cube_cache: OnceLock<Arc<[crate::cover::Minterm<crate::Symbol>]>>,
    /// Cached AST representation (reconstructed lazily when needed for display/fold)
    /// OnceLock's Clone copies the content, so clones share cached data via Arc
    pub(crate) ast_cache: OnceLock<Arc<BoolExprAst>>,
    /// Invariant brand marker (the storage carries the actual handle).
    _brand: std::marker::PhantomData<fn() -> B>,
}

/// Cloning copies the storage handle (a refcount bump) and the node ID; `OnceLock::clone` copies the
/// cached content (Arc pointers), so clones share the cached data. Hand-written so it does not require
/// `B: Clone` (a brand is a zero-sized marker that need not implement `Clone`).
impl<B: Brand> Clone for BoolExpr<B> {
    fn clone(&self) -> Self {
        BoolExpr {
            store: self.store.clone(),
            root: self.root,
            cube_cache: self.cube_cache.clone(),
            ast_cache: self.ast_cache.clone(),
            _brand: std::marker::PhantomData,
        }
    }
}

impl<B: Brand> BoolExpr<B> {
    /// Build a `BoolExpr` from a storage handle and root node, with fresh (empty) caches. The single
    /// place the struct is constructed, so the cache fields are initialised in exactly one spot.
    pub(crate) fn from_store(store: Store, root: NodeId) -> Self {
        BoolExpr {
            store,
            root,
            cube_cache: OnceLock::new(),
            ast_cache: OnceLock::new(),
            _brand: std::marker::PhantomData,
        }
    }

    /// Create a variable expression in the given store (creating the variable on first use).
    pub(crate) fn var_in(store: Store, name: &str) -> Self {
        let var_id = BddManager::make_var(&store, name);
        let node = BddManager::make_node(&store, var_id, FALSE_NODE, TRUE_NODE);
        BoolExpr::from_store(store, node)
    }

    /// Create a constant expression in the given store.
    pub(crate) fn constant_in(store: Store, value: bool) -> Self {
        let root = if value { TRUE_NODE } else { FALSE_NODE };
        BoolExpr::from_store(store, root)
    }

    /// The BDD root node id (for sibling modules such as the builder's `graft`).
    pub(crate) fn root_node(&self) -> NodeId {
        self.root
    }

    /// A stable identity for this expression's manager (for `graft` checks and `Eq`/`Hash`).
    pub(crate) fn store_ident(&self) -> *const () {
        Arc::as_ptr(&self.store).cast::<()>()
    }

    /// Clone this expression's storage handle (for building derived expressions in the same manager).
    pub(crate) fn store_cloned(&self) -> Store {
        self.store.clone()
    }
}

impl BoolExpr<Global> {
    /// Create a variable expression with the given name
    #[must_use]
    pub fn variable<S: AsRef<str>>(name: S) -> Self {
        BoolExpr::var_in(BddManager::get_or_create(), name.as_ref())
    }

    /// Create a constant expression (true or false)
    #[must_use]
    pub fn constant(value: bool) -> Self {
        BoolExpr::constant_in(BddManager::get_or_create(), value)
    }
}

impl<B: Brand> BoolExpr<B> {
    /// Get or create the cached product-term cubes with local caching.
    ///
    /// Extracts cubes (input [`Minterm`](crate::cover::Minterm)s) from the BDD on first
    /// access and caches them for subsequent calls.
    pub(crate) fn get_or_create_cubes(&self) -> Arc<[crate::cover::Minterm<crate::Symbol>]> {
        // Check local cache
        if let Some(cubes) = self.cube_cache.get() {
            return Arc::clone(cubes);
        }

        // Not cached - extract from BDD
        let cubes: Arc<[crate::cover::Minterm<crate::Symbol>]> = self.extract_cubes_from_bdd();

        // Cache it locally
        let _ = self.cube_cache.set(Arc::clone(&cubes));

        cubes
    }
}

impl<B: Brand> BoolExpr<B> {
    /// Build a `BoolExpr` in the given `store` from a set of product-term cubes (sum of products).
    ///
    /// Each [`Minterm`](crate::cover::Minterm) becomes an AND of its fixed literals; the
    /// expression is their OR. An empty cube is a tautology (`true`); an empty cube set is
    /// `false`. The supplied cubes are cached on the result so later cube extraction /
    /// minimisation reporting reflects exactly these terms (e.g. an Espresso-minimised set).
    ///
    /// **Precondition:** the cubes must describe the same function as the built expression — i.e.
    /// the cubes must fix every variable the function depends on. This holds for the only callers
    /// (a single-output `Cover` derived from the same variable namespace); passing an unrelated cube
    /// set would desync [`to_cubes`](Self::to_cubes) from the BDD. Checked with a `debug_assert!`.
    pub(crate) fn from_cubes_in(
        store: Store,
        cubes: Arc<[crate::cover::Minterm<crate::Symbol>]>,
    ) -> BoolExpr<B> {
        if cubes.is_empty() {
            return BoolExpr::constant_in(store, false);
        }

        // OR the product terms, each an AND of its fixed literals (empty cube = tautology).
        let expr = cubes
            .iter()
            .map(|cube| {
                cube.vars()
                    .iter()
                    .zip(cube.iter())
                    .filter_map(|(name, val)| match val {
                        Some(true) => Some(BoolExpr::var_in(store.clone(), name.as_ref())),
                        Some(false) => Some(BoolExpr::var_in(store.clone(), name.as_ref()).not()),
                        None => None,
                    })
                    .reduce(|acc, f| acc.and(&f))
                    .unwrap_or_else(|| BoolExpr::constant_in(store.clone(), true))
            })
            .reduce(|acc, t| acc.or(&t))
            .unwrap();

        // The cached cubes must cover every variable the resulting function depends on, otherwise
        // `to_cubes()` would report a SOP inconsistent with the BDD.
        debug_assert!(
            {
                let cube_vars: std::collections::BTreeSet<&str> =
                    cubes[0].vars().iter().map(|s| s.as_ref()).collect();
                expr.collect_variables()
                    .iter()
                    .all(|v| cube_vars.contains(v.as_ref()))
            },
            "from_cubes: cached cubes omit a variable the BDD depends on"
        );

        // Cache the source cubes (typically minimised from Espresso).
        let _ = expr.cube_cache.set(cubes);

        expr
    }
}

/// Collect a minterm's fixed literals as a `name -> polarity` map (don't-cares omitted).
///
/// This is the scratch format consumed by the algebraic factoriser; don't-care variables
/// (`None`) are absent from the map.
pub(crate) fn minterm_literals(
    cube: &crate::cover::Minterm<crate::Symbol>,
) -> std::collections::BTreeMap<Symbol, bool> {
    cube.vars()
        .iter()
        .zip(cube.iter())
        .filter_map(|(name, val)| val.map(|polarity| (name.clone(), polarity)))
        .collect()
}

/// PartialEq implementation that compares BDDs for canonical equality
///
/// Since BDDs are canonical, two BoolExprs are equal if and only if
/// they represent the same logical function.
impl<B: Brand> PartialEq for BoolExpr<B> {
    fn eq(&self, other: &Self) -> bool {
        // BDDs are equal if they share the same manager and have the same root node. Within one brand
        // (a single manager) this is exact canonical equality; the brand type parameter already keeps
        // expressions of different (anonymous) contexts from being compared at all.
        self.store_ident() == other.store_ident() && self.root == other.root
    }
}

impl<B: Brand> Eq for BoolExpr<B> {}

/// Hashes the same identity the [`PartialEq`] impl compares — the manager's pointer and the BDD root
/// node — so the `Hash`/`Eq` contract holds and a `BoolExpr` can be a `HashMap`/`HashSet` key.
impl<B: Brand> std::hash::Hash for BoolExpr<B> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.store_ident().hash(state);
        self.root.hash(state);
    }
}

#[cfg(test)]
mod tests;
