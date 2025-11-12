//! Boolean expression types with operator overloading and parsing support
//!
//! This module provides a boolean expression representation that can be constructed
//! programmatically using operator overloading, the `expr!` macro, or parsed from strings.
//! Expressions can be minimised using the Espresso algorithm.
//!
//! # Main Types
//!
//! - [`BoolExpr`] - A boolean expression that supports three construction methods:
//!   1. Method API: `a.and(&b).or(&c)`
//!   2. Operator overloading: `&a * &b + &c`
//!   3. **`expr!` macro**: `expr!(a * b + c)` - Recommended!
//!   
//!   **Important (v3.1.1+):** All `BoolExpr` instances use BDD as their internal representation,
//!   providing canonical form, efficient operations, and automatic simplification.
//!
//! - [`Bdd`] - Type alias for [`BoolExpr`] (unified in v3.1.1).
//!   Previously a separate type, now exists only for backwards compatibility.
//!
//! # Unified BDD Architecture (v3.1.1+)
//!
//! Starting in v3.1.1, `BoolExpr` and `Bdd` are the same type. All boolean expressions
//! use Binary Decision Diagrams as their canonical internal representation, providing:
//!
//! - **Canonical representation**: Equivalent expressions have identical internal structure
//! - **Efficient operations**: Polynomial-time AND/OR/NOT via hash consing and memoisation
//! - **Memory efficiency**: Structural sharing across all operations
//! - **Automatic simplification**: Redundancy elimination during construction
//! - **Fast equality checks**: O(1) pointer comparison for equivalent expressions
//!
//! **Deprecated methods:**
//! - `to_bdd()` - Returns `self.clone()` (expression IS a BDD)
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
//! ## Minimizing and Evaluating
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
//! // Minimize it (returns new minimized instance)
//! let minimized = redundant.minimize()?;
//! println!("Minimized: {}", minimized);  // Output: a * b
//!
//! // Check logical equivalence
//! let redundant2 = expr!(a * b + a * b * c);
//! assert!(redundant2.equivalent_to(&minimized));
//! # Ok(())
//! # }
//! ```

// Submodules
mod ast;
mod bdd;
mod conversions;
mod display;
pub mod error;
mod eval;
pub(crate) mod factorization;
mod manager;
mod minimize;
mod operators;
mod parser;

pub use error::{ExpressionParseError, ParseBoolExprError};

// Re-export AST types
pub(crate) use ast::BoolExprAst;
pub use ast::ExprNode;

// Re-export manager types for internal use
use manager::{BddManager, NodeId, FALSE_NODE, TRUE_NODE};

use std::sync::{Arc, OnceLock, RwLock};

/// A boolean expression that can be manipulated programmatically
///
/// This type represents boolean expressions as Binary Decision Diagrams (BDDs) for efficient
/// operations and canonical representation. It also maintains an optional AST cache for
/// display and tree traversal operations.
///
/// Uses `Arc` internally for efficient cloning. Provides a fluent method-based API
/// and an `expr!` macro for clean syntax.
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
/// ```
///
/// ## Using operator overloading (requires explicit &)
/// ```  
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = &a * &b + &(&a).not() * &(&b).not();
/// ```
#[derive(Clone)]
pub struct BoolExpr {
    /// BDD manager (shared across all BoolExprs)
    manager: Arc<RwLock<BddManager>>,
    /// Root node ID in the BDD
    root: NodeId,
    /// Cached DNF (cubes) for this BDD
    /// This avoids expensive BDD traversal when converting to DNF
    /// Uses OnceLock for lazy initialization
    dnf_cache: OnceLock<crate::cover::Dnf>,
    /// Cached AST representation (reconstructed lazily when needed for display/fold)
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
/// `Bdd` and `BoolExpr` are now the same type. This alias exists to ensure
/// that code using the `Bdd` name continues to compile seamlessly.
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
/// Can continue to work unchanged, or be updated to:
/// ```
/// use espresso_logic::BoolExpr;
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let result = a.and(&b);
/// ```
pub type Bdd = BoolExpr;

#[cfg(test)]
mod tests;
