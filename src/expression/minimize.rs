//! Minimization implementation for BoolExpr
//!
//! This module provides the specialised `Minimizable` trait implementation
//! for `BoolExpr` that converts through DNF for minimization.

use super::BoolExpr;
use crate::cover::Dnf;
use crate::espresso::error::MinimizationError;
use crate::{EspressoConfig, Minimizable};

/// Specialised implementation of Minimizable for BoolExpr
///
/// Minimizes the DNF representation and rebuilds the BDD from it.
/// Since BDDs are canonical, the resulting BoolExpr will have the same
/// NodeId as the original, but with a minimised DNF cached.
///
/// # Workflow
///
/// 1. Extract DNF from BoolExpr (uses local cache if available)
/// 2. Minimize the DNF via Cover/Espresso
/// 3. Convert minimised DNF back to BoolExpr
/// 4. The minimised DNF is cached in the new instance
///
/// This ensures the minimised BoolExpr has the minimised DNF cached,
/// producing cleaner AST output when displayed.
impl Minimizable for BoolExpr {
    fn minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError> {
        // Get DNF from BoolExpr (uses local cache if available)
        let dnf: Dnf = self.into();

        // Minimize via cover using the heuristic algorithm
        let minimised_dnf = crate::cover::minimize_via_cover(&dnf, config, |cover, cfg| {
            cover.minimize_with_config(cfg)
        })?;

        // Convert back to BoolExpr (rebuilds BDD, caches minimised DNF)
        Ok(BoolExpr::from(minimised_dnf))
    }

    fn minimize_exact_with_config(
        &self,
        config: &EspressoConfig,
    ) -> Result<Self, MinimizationError> {
        // Get DNF from BoolExpr (uses local cache if available)
        let dnf: Dnf = self.into();

        // Minimize via cover using the exact algorithm
        let minimised_dnf = crate::cover::minimize_via_cover(&dnf, config, |cover, cfg| {
            cover.minimize_exact_with_config(cfg)
        })?;

        // Convert back to BoolExpr (rebuilds BDD, caches minimised DNF)
        Ok(BoolExpr::from(minimised_dnf))
    }
}
