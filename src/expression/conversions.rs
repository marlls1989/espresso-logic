//! Conversions for BoolExpr
//!
//! This module provides conversion implementations between `BoolExpr` and `Dnf`.
//!
//! Note: `BoolExpr` and `Bdd` are now the same type (with `Bdd` being a type alias).
//! The From implementations below are identity conversions maintained for API compatibility.

use crate::cover::Dnf;
use crate::expression::BoolExpr;

// ============================================================================
// Blanket Conversions TO Dnf
// ============================================================================

/// Convert BoolExpr to DNF (via BDD for efficiency)
///
/// Ensures conversions go through BDD for canonical form and optimisations.
/// Uses caching to avoid expensive BDD traversal.
impl From<BoolExpr> for Dnf {
    fn from(expr: BoolExpr) -> Self {
        expr.get_or_create_dnf()
    }
}

/// Convert &BoolExpr to DNF (via BDD for efficiency)
///
/// Uses caching to avoid expensive BDD traversal.
impl From<&BoolExpr> for Dnf {
    fn from(expr: &BoolExpr) -> Self {
        expr.get_or_create_dnf()
    }
}

// Note: Bdd is now a type alias for BoolExpr, so From<Bdd> implementations
// are covered by the From<BoolExpr> implementations above.

// ============================================================================
// Conversions FROM Dnf
// ============================================================================

/// Convert DNF to BoolExpr
///
/// Reconstructs a boolean expression in DNF form (OR of AND terms).
///
/// # Examples
///
/// ```
/// use espresso_logic::{BoolExpr, Dnf};
///
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let expr = a.and(&b);
///
/// let dnf = Dnf::from(&expr);
/// let expr2 = BoolExpr::from(dnf);
///
/// assert!(expr.equivalent_to(&expr2));
/// ```
impl From<Dnf> for BoolExpr {
    fn from(dnf: Dnf) -> Self {
        if dnf.is_empty() {
            return BoolExpr::constant(false);
        }

        // Convert each cube to a product term
        let mut terms = Vec::new();
        for cube in dnf.cubes() {
            if cube.is_empty() {
                // Empty cube means tautology (all variables are don't-care)
                terms.push(BoolExpr::constant(true));
            } else {
                // Build AND of all literals in this cube
                let factors: Vec<BoolExpr> = cube
                    .iter()
                    .map(|(var, &polarity)| {
                        let v = BoolExpr::variable(var);
                        if polarity {
                            v
                        } else {
                            v.not()
                        }
                    })
                    .collect();

                let product = factors.into_iter().reduce(|acc, f| acc.and(&f)).unwrap();
                terms.push(product);
            }
        }

        // OR all terms together
        let expr = terms.into_iter().reduce(|acc, t| acc.or(&t)).unwrap();

        // Cache the source DNF (likely minimised from Espresso)
        expr.cache_dnf(dnf);

        expr
    }
}

// Note: Bdd is now a type alias for BoolExpr, so From<Dnf> for Bdd
// is covered by the From<Dnf> for BoolExpr implementation above.
