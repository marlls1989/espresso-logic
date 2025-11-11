//! Conversions between BoolExpr and Bdd

use super::BoolExpr;
use crate::bdd::Bdd;

// ============================================================================
// Conversions to/from Bdd (enables blanket Minimizable implementation)
// ============================================================================

/// Convert `BoolExpr` to `Bdd`
///
/// This enables the blanket `Minimizable` implementation for `BoolExpr`.
impl From<BoolExpr> for Bdd {
    fn from(expr: BoolExpr) -> Self {
        expr.to_bdd()
    }
}

/// Convert `&BoolExpr` to `Bdd`
///
/// This enables the blanket `Minimizable` implementation to work with references
/// without requiring a clone of the entire expression.
impl From<&BoolExpr> for Bdd {
    fn from(expr: &BoolExpr) -> Self {
        expr.to_bdd()
    }
}

/// Convert `Bdd` to `BoolExpr`
///
/// Since BoolExpr now uses BDD as primary storage, this is a simple wrapper.
/// The AST will be reconstructed lazily when needed for display or fold operations.
impl From<Bdd> for BoolExpr {
    fn from(bdd: Bdd) -> Self {
        BoolExpr {
            bdd,
            ast_cache: std::sync::OnceLock::new(),
        }
    }
}
