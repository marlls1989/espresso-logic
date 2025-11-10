//! Minimizable trait for Boolean function minimization
//!
//! This module provides the public [`Minimizable`] trait which defines
//! a uniform interface for minimizing Boolean functions using the Espresso algorithm.

use crate::error::MinimizationError;
use crate::EspressoConfig;

/// Public trait for types that can be minimized using Espresso
///
/// This trait provides a **transparent, uniform interface** for minimizing boolean functions
/// using the Espresso algorithm. All methods take `&self` and return a new minimized instance,
/// following an immutable functional style.
///
/// # Transparent Minimization
///
/// The beauty of this trait is that minimization works the same way regardless of input type.
/// Just call `.minimize()` on any supported type and get back a minimized version of the same type:
///
/// ```
/// use espresso_logic::{BoolExpr, Cover, CoverType, Minimizable};
///
/// # fn main() -> std::io::Result<()> {
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let c = BoolExpr::variable("c");
/// let redundant = a.and(&b).or(&a.and(&b).and(&c));
///
/// // Works on BoolExpr - returns BoolExpr
/// let min_expr = redundant.minimize()?;
/// println!("Minimized expression: {}", min_expr);
///
/// // Works on Cover - returns Cover
/// let mut cover = Cover::new(CoverType::F);
/// cover.add_expr(&redundant, "out")?;
/// let min_cover = cover.minimize()?;
/// println!("Minimized cover has {} cubes", min_cover.num_cubes());
///
/// // Both produce equivalent minimized results!
/// # Ok(())
/// # }
/// ```
///
/// # Implementations
///
/// - **[`Cover`]**: Direct implementation - minimizes cubes directly with Espresso
/// - **Blanket implementation** (v3.1+): For `T where &T: Into<Dnf>, T: From<Dnf>` (defined in `dnf` module)
///   - Automatically covers [`BoolExpr`] and [`expression::bdd::Bdd`]
///   - Workflow: Expression → Dnf (via BDD for canonical form) → Cover cubes → Espresso → minimized Cover → Dnf → Expression
///   - DNF serves as the intermediary representation, with BDD ensuring efficient conversion
///
/// [`expression::bdd::Bdd`]: crate::expression::bdd::Bdd
///
/// [`BoolExpr`]: crate::expression::BoolExpr
/// [`Cover`]: crate::Cover
///
/// # Immutable Design
///
/// All minimization methods preserve the original and return a new minimized instance:
///
/// ```
/// use espresso_logic::{BoolExpr, Minimizable};
///
/// # fn main() -> std::io::Result<()> {
/// let a = BoolExpr::variable("a");
/// let b = BoolExpr::variable("b");
/// let c = BoolExpr::variable("c");
///
/// let original = a.and(&b).or(&a.and(&b).and(&c));
/// let minimized = original.minimize()?;
///
/// // Original is unchanged
/// println!("Original: {}", original);
/// println!("Minimized: {}", minimized);
///
/// // Can continue using original
/// let bdd = original.to_bdd();
/// # Ok(())
/// # }
/// ```
pub trait Minimizable {
    /// Minimize using the heuristic Espresso algorithm
    ///
    /// Returns a new minimized instance without modifying the original.
    /// This is fast and produces near-optimal results (~99% optimal in practice).
    ///
    /// Default implementation calls `minimize_with_config` with default config.
    fn minimize(&self) -> Result<Self, MinimizationError>
    where
        Self: Sized,
    {
        let config = EspressoConfig::default();
        self.minimize_with_config(&config)
    }

    /// Minimize using the heuristic algorithm with custom configuration
    ///
    /// Returns a new minimized instance without modifying the original.
    ///
    /// This is the primary method that implementations must provide.
    fn minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError>
    where
        Self: Sized;

    /// Minimize using exact minimization
    ///
    /// Returns a new minimized instance without modifying the original.
    /// This guarantees minimal results but may be slower for large expressions.
    ///
    /// Default implementation calls `minimize_exact_with_config` with default config.
    fn minimize_exact(&self) -> Result<Self, MinimizationError>
    where
        Self: Sized,
    {
        let config = EspressoConfig::default();
        self.minimize_exact_with_config(&config)
    }

    /// Minimize using exact minimization with custom configuration
    ///
    /// Returns a new minimized instance without modifying the original.
    ///
    /// This is the primary method that implementations must provide.
    fn minimize_exact_with_config(
        &self,
        config: &EspressoConfig,
    ) -> Result<Self, MinimizationError>
    where
        Self: Sized;
}
