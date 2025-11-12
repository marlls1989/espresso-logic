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
/// **Note (v3.1.1+):** `BoolExpr` and `Bdd` are unified—all expressions are BDDs internally.
///
/// Extracts DNF from the internal BDD representation, minimizes it via Espresso,
/// then creates a new `BoolExpr` from the minimized DNF (which builds a new BDD).
///
/// # Workflow
///
/// 1. Extract DNF from BoolExpr's internal BDD (uses local cache if available)
/// 2. Minimize the DNF via Cover/Espresso
/// 3. Create new BoolExpr from minimised DNF (builds new BDD internally)
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
