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

/// Convert `Bdd` back to `BoolExpr`
///
/// This conversion extracts the cubes from the BDD and reconstructs a boolean expression.
/// The resulting expression will be in DNF (disjunctive normal form).
impl From<Bdd> for BoolExpr {
    fn from(bdd: Bdd) -> Self {
        let cubes = bdd.to_cubes();

        if cubes.is_empty() {
            return BoolExpr::constant(false);
        }

        let mut terms = Vec::new();

        for cube in cubes {
            if cube.is_empty() {
                // Empty cube = tautology (true)
                return BoolExpr::constant(true);
            }

            // Build product term from cube
            let mut factors: Vec<BoolExpr> = cube
                .iter()
                .map(|(var, &polarity)| {
                    let var_expr = BoolExpr::variable(var);
                    if polarity {
                        var_expr
                    } else {
                        var_expr.not()
                    }
                })
                .collect();

            // AND all factors together
            let product = factors.drain(..).reduce(|acc, f| acc.and(&f)).unwrap();
            terms.push(product);
        }

        // OR all terms together
        terms.into_iter().reduce(|acc, t| acc.or(&t)).unwrap()
    }
}
