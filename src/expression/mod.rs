//! Boolean expression types with operator overloading and parsing support
//!
//! This module provides a high-level boolean expression representation that can be constructed
//! programmatically using operator overloading, the `expr!` macro, or parsed from strings.
//! Expressions can be minimised directly using the Espresso algorithm.
//!
//! # Main Types
//!
//! - [`BoolExpr`] - A boolean expression supporting three construction methods:
//!   1. **`expr!` macro**: `expr!(a * b + c)` - Recommended!
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
//! The `expr!` macro provides the cleanest syntax with three usage styles:
//!
//! ```
//! use espresso_logic::{BoolExpr, expr};
//!
//! // Style 1: String literals (most concise - no variable declarations!)
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
//! # 📚 Comprehensive Guide
//!
//! For a complete guide to the Boolean expression API including detailed examples,
//! composition patterns, BDD architecture, performance considerations, and best practices,
//! see the embedded documentation below.
#![doc = include_str!("../../docs/BOOLEAN_EXPRESSIONS.md")]

// Submodules
mod ast;
mod bdd;
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

// Re-export manager types for internal use
use crate::Symbol;
use manager::{BddManager, NodeId, FALSE_NODE, TRUE_NODE};

use std::sync::{Arc, OnceLock, RwLock};

/// A boolean expression for logic minimisation
///
/// `BoolExpr` provides a high-level interface for building and minimising Boolean functions.
/// Expressions can be constructed programmatically, parsed from strings, or composed from
/// existing expressions, then minimised using the Espresso algorithm.
///
/// # Construction Methods
///
/// Three ways to build expressions:
///
/// 1. **`expr!` macro** (recommended) - Clean syntax without explicit references
/// 2. **Method API** - Explicit `.and()`, `.or()`, `.not()` calls
/// 3. **Operator overloading** - Requires `&` references
///
/// # Minimisation
///
/// Expressions support direct minimisation via:
/// - `.minimize()` - Fast heuristic algorithm (~99% optimal)
/// - `.minimize_exact()` - Slower but guaranteed minimal result
///
/// # Implementation Details
///
/// **Internal representation:** Uses Binary Decision Diagrams (BDDs) for efficient
/// operations and canonical form. This is an implementation detail that makes the
/// high-level API practical for complex expressions.
///
/// Each `BoolExpr` contains:
/// - **Node ID** - Reference to BDD structure in global manager
/// - **Manager Reference** - Shared access to global singleton BDD manager
/// - **Cube Cache** - Lazily cached cubes for Espresso minimisation
/// - **AST Cache** - Lazily cached syntax tree for display
///
/// The BDD manager is a global singleton (protected by `RwLock`) shared by all
/// expressions, providing structural sharing and canonical representation.
///
/// # Cloning
///
/// Cloning is very cheap - copies Arc references and node ID. `OnceLock::clone()` copies
/// the cached content (Arc pointers), so clones share the actual cached data. The BDD
/// structure itself is shared via the global manager.
///
/// # Thread Safety
///
/// Thread-safe via global BDD manager with `RwLock` protection. Multiple threads
/// can safely create and manipulate expressions concurrently.
///
/// # Memory
///
/// The shared manager's node table and operation caches **grow monotonically and are never evicted**
/// while any `BoolExpr` is alive (node IDs must stay stable for lock-free traversal). The manager is
/// dropped — reclaiming everything — only once the last live `BoolExpr` is dropped. A long-running
/// program that builds very many distinct expressions over its lifetime will therefore see the
/// manager's memory grow until that point; this is intentional, not a leak.
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
/// // All operations are efficient BDD operations
/// let vars = expr.collect_variables();
/// println!("Variables: {:?}", vars);
/// ```
#[derive(Clone)]
pub struct BoolExpr {
    /// BDD manager (shared across all BoolExprs)
    manager: Arc<RwLock<BddManager>>,
    /// Root node ID in the BDD
    root: NodeId,
    /// Cached product-term cubes (input minterms) for this BDD.
    /// This avoids expensive BDD traversal when extracting cubes or minimising.
    /// OnceLock's Clone copies the content, so clones share cached data via Arc.
    cube_cache: OnceLock<Arc<[crate::cover::Minterm<crate::Symbol>]>>,
    /// Cached AST representation (reconstructed lazily when needed for display/fold)
    /// OnceLock's Clone copies the content, so clones share cached data via Arc
    pub(crate) ast_cache: OnceLock<Arc<BoolExprAst>>,
}

impl BoolExpr {
    /// Build a `BoolExpr` from a manager and root node, with fresh (empty) caches. The single place the
    /// struct is constructed, so the cache fields are initialised in exactly one spot.
    fn from_root(manager: Arc<RwLock<BddManager>>, root: NodeId) -> Self {
        BoolExpr {
            manager,
            root,
            cube_cache: OnceLock::new(),
            ast_cache: OnceLock::new(),
        }
    }

    /// Apply a BDD operation to this expression's root under one write lock, returning the result as a
    /// fresh `BoolExpr` sharing the same manager. Shared by `and`/`or`/`not`.
    fn op(&self, f: impl FnOnce(&mut BddManager, NodeId) -> NodeId) -> BoolExpr {
        let manager = Arc::clone(&self.manager);
        let root = f(&mut manager.write().unwrap(), self.root);
        BoolExpr::from_root(manager, root)
    }

    /// Create a variable expression with the given name
    #[must_use]
    pub fn variable(name: &str) -> Self {
        let manager = BddManager::get_or_create();
        let node = {
            let mut mgr = manager.write().unwrap();
            let var_id = mgr.get_or_create_var(name);
            mgr.make_node(var_id, FALSE_NODE, TRUE_NODE)
        };
        BoolExpr::from_root(manager, node)
    }

    /// Create a constant expression (true or false)
    #[must_use]
    pub fn constant(value: bool) -> Self {
        let manager = BddManager::get_or_create();
        let root = if value { TRUE_NODE } else { FALSE_NODE };
        BoolExpr::from_root(manager, root)
    }

    /// Get or create the cached product-term cubes with local caching.
    ///
    /// Extracts cubes (input [`Minterm`](crate::cover::Minterm)s) from the BDD on first
    /// access and caches them for subsequent calls.
    pub(crate) fn get_or_create_cubes(&self) -> Arc<[crate::cover::Minterm<crate::Symbol>]> {
        // Check local cache
        if let Some(cubes) = self.cube_cache.get() {
            return Arc::clone(cubes); // Cheap Arc clone
        }

        // Not cached - extract from BDD
        let cubes: Arc<[crate::cover::Minterm<crate::Symbol>]> = self.extract_cubes_from_bdd();

        // Cache it locally
        let _ = self.cube_cache.set(Arc::clone(&cubes));

        cubes
    }

    /// Build a `BoolExpr` from a set of product-term cubes (sum of products).
    ///
    /// Each [`Minterm`](crate::cover::Minterm) becomes an AND of its fixed literals; the
    /// expression is their OR. An empty cube is a tautology (`true`); an empty cube set is
    /// `false`. The supplied cubes are cached on the result so later cube extraction /
    /// minimisation reporting reflects exactly these terms (e.g. an Espresso-minimised set).
    ///
    /// **Precondition:** the cubes must describe the same function as the built expression — i.e.
    /// the cubes must fix every variable the function depends on. This holds for the only caller
    /// (a single-output `Cover` derived from the same variable namespace); passing an unrelated cube
    /// set would desync [`to_cubes`](Self::to_cubes) from the BDD. Checked with a `debug_assert!`.
    pub(crate) fn from_cubes(cubes: Arc<[crate::cover::Minterm<crate::Symbol>]>) -> BoolExpr {
        if cubes.is_empty() {
            return BoolExpr::constant(false);
        }

        // OR the product terms, each an AND of its fixed literals (empty cube = tautology).
        let expr = cubes
            .iter()
            .map(|cube| {
                cube.vars()
                    .iter()
                    .zip(cube.iter())
                    .filter_map(|(name, val)| match val {
                        Some(true) => Some(BoolExpr::variable(name)),
                        Some(false) => Some(BoolExpr::variable(name).not()),
                        None => None,
                    })
                    .reduce(|acc, f| acc.and(&f))
                    .unwrap_or_else(|| BoolExpr::constant(true))
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
/// (`None`) are simply absent from the map.
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
impl PartialEq for BoolExpr {
    fn eq(&self, other: &Self) -> bool {
        // BDDs are equal if they share the same manager and have the same root node
        // The singleton manager ensures consistent representation across all BoolExprs
        Arc::ptr_eq(&self.manager, &other.manager) && self.root == other.root
    }
}

impl Eq for BoolExpr {}

/// Hashes the same identity the [`PartialEq`] impl compares — the shared manager's pointer and the BDD
/// root node — so the `Hash`/`Eq` contract holds and a `BoolExpr` can be a `HashMap`/`HashSet` key.
impl std::hash::Hash for BoolExpr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.manager) as *const ()).hash(state);
        self.root.hash(state);
    }
}

#[cfg(test)]
mod tests;
