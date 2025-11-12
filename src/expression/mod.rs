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
//! **Deprecated methods:**
//! - `to_bdd()` - Returns `self.clone()` (expression already uses BDD internally)
//! - `Bdd::from_expr()` - Returns `expr.clone()` (redundant conversion)
//! - `Bdd::to_expr()` - Returns `self.clone()` (redundant conversion)
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
mod conversions;
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
/// - **DNF Cache** - Lazily cached cubes for Espresso minimisation
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
    /// Cached DNF (cubes) for this BDD
    /// This avoids expensive BDD traversal when converting to DNF
    /// OnceLock's Clone copies the content, so clones share cached data via Arc
    dnf_cache: OnceLock<crate::cover::Dnf>,
    /// Cached AST representation (reconstructed lazily when needed for display/fold)
    /// OnceLock's Clone copies the content, so clones share cached data via Arc
    pub(crate) ast_cache: OnceLock<Arc<BoolExprAst>>,
}

impl BoolExpr {
    /// Create a variable expression with the given name
    pub fn variable(name: &str) -> Self {
        let manager = BddManager::get_or_create();
        let mut mgr = manager.write().unwrap();
        let var_id = mgr.get_or_create_var(name);
        let node = mgr.make_node(var_id, FALSE_NODE, TRUE_NODE);
        drop(mgr); // Explicitly release the lock
        BoolExpr {
            manager,
            root: node,
            dnf_cache: OnceLock::new(),
            ast_cache: OnceLock::new(),
        }
    }

    /// Create a constant expression (true or false)
    pub fn constant(value: bool) -> Self {
        let manager = BddManager::get_or_create();
        BoolExpr {
            manager,
            root: if value { TRUE_NODE } else { FALSE_NODE },
            dnf_cache: OnceLock::new(),
            ast_cache: OnceLock::new(),
        }
    }

    /// Convert this boolean expression to a Binary Decision Diagram
    ///
    /// **Deprecated:** `BoolExpr` is now implemented as a BDD internally.
    /// This method simply clones the expression and exists for backwards compatibility.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::BoolExpr;
    ///
    /// let a = BoolExpr::variable("a");
    /// let b = BoolExpr::variable("b");
    /// let expr = a.and(&b);
    ///
    /// let bdd = expr.to_bdd();  // Just returns a clone
    /// ```
    #[deprecated(
        since = "3.1.1",
        note = "BoolExpr is now a BDD internally. Use clone() instead."
    )]
    pub fn to_bdd(&self) -> Self {
        self.clone()
    }

    /// Create a BoolExpr from a BDD (actually the same type)
    ///
    /// **Deprecated:** `BoolExpr` and `Bdd` are now the same type.
    /// This method simply clones the expression and exists for backwards compatibility.
    #[deprecated(
        since = "3.1.1",
        note = "BoolExpr and Bdd are now the same type. Use clone() instead."
    )]
    pub fn from_expr(expr: &BoolExpr) -> Self {
        expr.clone()
    }

    /// Convert this BDD to a BoolExpr (actually the same type)
    ///
    /// **Deprecated:** `BoolExpr` and `Bdd` are now the same type.
    /// This method simply clones the expression and exists for backwards compatibility.
    #[deprecated(
        since = "3.1.1",
        note = "BoolExpr and Bdd are now the same type. Use clone() instead."
    )]
    pub fn to_expr(&self) -> Self {
        self.clone()
    }

    /// Get or create the DNF representation with local caching
    ///
    /// Extracts cubes from BDD on first access and caches for subsequent calls.
    fn get_or_create_dnf(&self) -> crate::cover::Dnf {
        // Check local cache
        if let Some(dnf) = self.dnf_cache.get() {
            return dnf.clone(); // Cheap Arc clone
        }

        // Not cached - extract from BDD
        let cubes = self.extract_cubes_from_bdd();
        let dnf = crate::cover::Dnf::from_cubes(&cubes);

        // Cache it locally
        let _ = self.dnf_cache.set(dnf.clone());

        dnf
    }
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

/// Type alias for backwards compatibility.
///
/// **Deprecated:** Use [`BoolExpr`] directly instead.
///
/// `Bdd` and `BoolExpr` are now the same type in the unified architecture (v3.1.1+).
/// This alias exists for backwards compatibility but should not be used in new code.
///
/// # Migration
///
/// Old code using `Bdd`:
/// ```
/// use espresso_logic::Bdd;
///
/// let a = Bdd::variable("a");
/// let b = Bdd::variable("b");
/// let result = a.and(&b);
/// ```
///
/// Should be updated to use [`BoolExpr`] directly:
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let result = a.and(&b);
/// ```
#[deprecated(
    since = "3.1.1",
    note = "Use `BoolExpr` directly. `Bdd` is now just a type alias for backwards compatibility."
)]
pub type Bdd = BoolExpr;

#[cfg(test)]
mod tests;
