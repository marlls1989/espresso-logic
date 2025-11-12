//! Minimizable trait and implementations for Boolean function minimization
//!
//! This module provides the [`Minimizable`] trait which defines a uniform interface
//! for minimizing Boolean functions using the Espresso algorithm, along with implementations
//! for [`Cover`] and a blanket implementation for types convertible to/from [`Dnf`].

use super::cubes::CubeType;
use super::dnf::Dnf;
use super::Cover;
use crate::espresso::error::MinimizationError;
use crate::EspressoConfig;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Public trait for types that can be minimized using Espresso
///
/// This trait provides a **transparent, uniform interface** for minimizing boolean functions
/// using the Espresso algorithm. All methods take `&self` and return a new minimized instance,
/// following an immutable functional style.
///
/// **Note (v3.1+):** You must explicitly import this trait to use its methods:
/// ```
/// use espresso_logic::{BoolExpr, Minimizable};
///
/// let expr = BoolExpr::parse("a * b + a * b * c")?;
/// let minimized = expr.minimize()?;  // Requires `use Minimizable`
/// # Ok::<(), std::io::Error>(())
/// ```
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
/// - **Blanket implementation** (v3.1+): For `T where &T: Into<Dnf>, T: From<Dnf>`
///   - Automatically covers [`BoolExpr`] and [`Bdd`]
///   - Workflow: Expression → Dnf (via BDD for canonical form) → Cover cubes → Espresso → minimized Cover → Dnf → Expression
///   - DNF serves as the intermediary representation, with BDD ensuring efficient conversion
///
/// [`Bdd`]: crate::Bdd
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

/// Private helper function to minimize a Cover using either heuristic or exact algorithm
fn minimize_cover_with<F>(
    cover: &Cover,
    config: &EspressoConfig,
    minimize_fn: F,
) -> Result<Cover, MinimizationError>
where
    F: FnOnce(
        &crate::espresso::Espresso,
        &crate::espresso::EspressoCover,
        Option<&crate::espresso::EspressoCover>,
        Option<&crate::espresso::EspressoCover>,
    ) -> (
        crate::espresso::EspressoCover,
        crate::espresso::EspressoCover,
        crate::espresso::EspressoCover,
    ),
{
    use crate::espresso::{Espresso, EspressoCover};

    // Split cubes into F, D, R sets based on cube type
    let mut f_cubes = Vec::new();
    let mut d_cubes = Vec::new();
    let mut r_cubes = Vec::new();

    for cube in cover.cubes.iter() {
        let input_vec: Vec<u8> = cube
            .inputs()
            .iter()
            .map(|&opt| match opt {
                Some(false) => 0,
                Some(true) => 1,
                None => 2,
            })
            .collect();

        // Convert outputs: true → 1, false → 0
        let output_vec: Vec<u8> = cube
            .outputs()
            .iter()
            .map(|&b| if b { 1 } else { 0 })
            .collect();

        // Send to appropriate set based on cube type
        match cube.cube_type() {
            CubeType::F => f_cubes.push((input_vec, output_vec)),
            CubeType::D => d_cubes.push((input_vec, output_vec)),
            CubeType::R => r_cubes.push((input_vec, output_vec)),
        }
    }

    // Direct C calls - thread-safe via thread-local storage
    let esp = Espresso::new(cover.num_inputs(), cover.num_outputs(), config);

    // Build covers from cube data - convert Vec to slices
    let f_cubes_refs: Vec<(&[u8], &[u8])> = f_cubes
        .iter()
        .map(|(i, o)| (i.as_slice(), o.as_slice()))
        .collect();
    let f_cover =
        EspressoCover::from_cubes(&f_cubes_refs, cover.num_inputs(), cover.num_outputs())?;

    let d_cover = if !d_cubes.is_empty() {
        let d_cubes_refs: Vec<(&[u8], &[u8])> = d_cubes
            .iter()
            .map(|(i, o)| (i.as_slice(), o.as_slice()))
            .collect();
        Some(EspressoCover::from_cubes(
            &d_cubes_refs,
            cover.num_inputs(),
            cover.num_outputs(),
        )?)
    } else {
        None
    };
    let r_cover = if !r_cubes.is_empty() {
        let r_cubes_refs: Vec<(&[u8], &[u8])> = r_cubes
            .iter()
            .map(|(i, o)| (i.as_slice(), o.as_slice()))
            .collect();
        Some(EspressoCover::from_cubes(
            &r_cubes_refs,
            cover.num_inputs(),
            cover.num_outputs(),
        )?)
    } else {
        None
    };

    // Call the provided minimize function (heuristic or exact)
    let (f_result, d_result, r_result) =
        minimize_fn(&esp, &f_cover, d_cover.as_ref(), r_cover.as_ref());

    // Extract minimized cubes
    let mut minimized_cubes = Vec::new();
    minimized_cubes.extend(f_result.to_cubes(cover.num_inputs(), cover.num_outputs(), CubeType::F));
    minimized_cubes.extend(d_result.to_cubes(cover.num_inputs(), cover.num_outputs(), CubeType::D));
    minimized_cubes.extend(r_result.to_cubes(cover.num_inputs(), cover.num_outputs(), CubeType::R));

    // Build new cover with minimized cubes - only clone labels (Arc, cheap)
    Ok(Cover {
        num_inputs: cover.num_inputs,
        num_outputs: cover.num_outputs,
        input_labels: cover.input_labels.clone(),
        output_labels: cover.output_labels.clone(),
        cubes: minimized_cubes,
        cover_type: cover.cover_type,
    })
}

// Implement public Minimizable trait for Cover
impl Minimizable for Cover {
    fn minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError> {
        minimize_cover_with(self, config, |esp, f, d, r| esp.minimize(f, d, r))
    }

    fn minimize_exact_with_config(
        &self,
        config: &EspressoConfig,
    ) -> Result<Self, MinimizationError> {
        minimize_cover_with(self, config, |esp, f, d, r| esp.minimize_exact(f, d, r))
    }
}

/// Helper function to minimize via Cover conversion
///
/// Used by Minimizable implementations to convert DNF → Cover → minimize → DNF
pub(crate) fn minimize_via_cover<F>(
    dnf: &Dnf,
    config: &EspressoConfig,
    minimize_fn: F,
) -> Result<Dnf, MinimizationError>
where
    F: FnOnce(&Cover, &EspressoConfig) -> Result<Cover, MinimizationError>,
{
    // Use cached variables (already sorted alphabetically)
    let var_list = dnf.variables();
    let var_refs: Vec<&str> = var_list.iter().map(|s| s.as_ref()).collect();

    // Create cover with proper dimensions and labels
    let mut cover = crate::Cover::with_labels(crate::CoverType::F, &var_refs, &["out"]);

    // Add cubes to cover
    for cube in dnf.cubes() {
        let mut inputs = vec![None; var_list.len()];
        for (i, var) in var_list.iter().enumerate() {
            if let Some(&polarity) = cube.get(var) {
                inputs[i] = Some(polarity);
            }
        }
        cover.add_cube(&inputs, &[Some(true)]);
    }

    // Minimize the cover using the provided function
    let minimized_cover = minimize_fn(&cover, config)?;

    // Convert back to Dnf
    Ok(cover_to_dnf(&minimized_cover))
}

/// Helper function to convert a Cover back to Dnf
pub(crate) fn cover_to_dnf(cover: &Cover) -> Dnf {
    let mut cubes = Vec::new();

    for cube in cover.cubes() {
        let mut product = BTreeMap::new();

        // Get input labels
        let input_labels = cover.input_labels();

        for (i, &literal) in cube.inputs().iter().enumerate() {
            if let Some(polarity) = literal {
                let var_name = if i < input_labels.len() {
                    Arc::clone(&input_labels[i])
                } else {
                    Arc::from(format!("x{}", i).as_str())
                };
                product.insert(var_name, polarity);
            }
        }

        cubes.push(product);
    }

    Dnf::from_cubes(&cubes)
}

// Note: The Minimizable implementation for BoolExpr has been moved to
// src/expression/minimize.rs where it belongs organizationally.
