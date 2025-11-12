//! Minimization implementation for BoolExpr
//!
//! This module provides the specialised `Minimizable` trait implementation
//! for `BoolExpr` that efficiently updates the DNF cache without rebuilding
//! the BDD (since BDDs are canonical).

use super::BoolExpr;
use crate::cover::Dnf;
use crate::espresso::error::MinimizationError;
use crate::{EspressoConfig, Minimizable};

/// Specialised implementation of Minimizable for BoolExpr
///
/// This is more efficient than a generic implementation because it avoids
/// rebuilding the BDD from the minimised DNF. Since BDDs are canonical,
/// the minimised version has the same NodeId as the original, so we just
/// create a fresh BoolExpr with the same root and cache the minimised DNF.
///
/// # Workflow
///
/// 1. Extract DNF from BoolExpr (uses cache if available)
/// 2. Minimize the DNF via Cover/Espresso
/// 3. Create new BoolExpr with same root but fresh caches
/// 4. Cache the minimised DNF in the new instance
///
/// This avoids the expensive BDD reconstruction that would happen with
/// `BoolExpr::from(minimised_dnf)`, and ensures the new instance doesn't
/// carry over the old AST cache (which would display the non-minimised form).
impl Minimizable for BoolExpr {
    fn minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError> {
        // Get DNF from BoolExpr (uses cache if available)
        let dnf: Dnf = self.into();

        // Minimize via cover using the heuristic algorithm
        let minimised_dnf = crate::cover::minimize_via_cover(&dnf, config, |cover, cfg| {
            cover.minimize_with_config(cfg)
        })?;

        // Create new BoolExpr with same BDD root/NodeId but fresh caches
        // This avoids copying the old AST cache
        let result = self.with_fresh_caches();

        // Cache the minimised DNF
        result.cache_dnf(minimised_dnf);

        Ok(result)
    }

    fn minimize_exact_with_config(
        &self,
        config: &EspressoConfig,
    ) -> Result<Self, MinimizationError> {
        // Get DNF from BoolExpr (uses cache if available)
        let dnf: Dnf = self.into();

        // Minimize via cover using the exact algorithm
        let minimised_dnf = crate::cover::minimize_via_cover(&dnf, config, |cover, cfg| {
            cover.minimize_exact_with_config(cfg)
        })?;

        // Create new BoolExpr with same BDD root/NodeId but fresh caches
        // This avoids copying the old AST cache
        let result = self.with_fresh_caches();

        // Cache the minimised DNF
        result.cache_dnf(minimised_dnf);

        Ok(result)
    }
}
