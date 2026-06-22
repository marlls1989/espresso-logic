//! Minimizable trait and implementations for Boolean function minimisation
//!
//! This module provides the [`Minimizable`] trait which defines a uniform interface
//! for minimising Boolean functions using the Espresso algorithm, along with implementations
//! for [`Cover`] and [`BoolExpr`].

use super::cubes::{Cube, CubeType};
use super::label::Anonymous;
use super::Cover;
use crate::espresso::error::MinimizationError;
use crate::expression::BoolExpr;
use crate::EspressoConfig;
use crate::Symbol;
use std::sync::Arc;

/// Public trait for types that can be minimised using Espresso
///
/// This trait provides a **transparent, uniform interface** for minimising boolean functions
/// using the Espresso algorithm. All methods take `&self` and return a new minimised instance,
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
/// # Transparent Minimisation
///
/// The beauty of this trait is that minimisation works the same way regardless of input type.
/// Just call `.minimize()` on any supported type and get back a minimised version of the same type:
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
/// - **[`Cover`]**: Direct implementation - minimises cubes directly with Espresso
/// - **[`BoolExpr`]**: Extracts the expression's product terms (from its internal BDD) into a
///   single-output [`Cover`], minimises it with Espresso, then rebuilds an expression from the
///   minimised product terms. Workflow: Expression → Cover → Espresso → minimised Cover → Expression
///
/// This trait is **not sealed**: it is intentionally open so downstream crates may implement it for
/// their own types (for example, to forward minimisation through a wrapper). An implementation only
/// needs to provide [`try_minimize_with_config`](Self::try_minimize_with_config) and
/// [`try_minimize_exact_with_config`](Self::try_minimize_exact_with_config); the remaining methods
/// have defaults.
///
/// [`BoolExpr`]: crate::expression::BoolExpr
/// [`Cover`]: crate::Cover
///
/// # Immutable Design
///
/// All minimisation methods preserve the original and return a new minimised instance:
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
/// // Can continue using original (already a BDD internally)
/// println!("Original has {} BDD nodes", original.node_count());
/// # Ok(())
/// # }
/// ```
pub trait Minimizable {
    /// Minimise using the heuristic Espresso algorithm, surfacing an instance conflict as an error.
    ///
    /// Like [`minimize`](Self::minimize), but returns [`MinimizationError::Instance`] instead of
    /// panicking when a low-level [`Espresso`](crate::espresso::Espresso) instance of *different*
    /// dimensions is already live on this thread. Prefer this when you deliberately mix the
    /// low-level [`espresso`](crate::espresso) API with the high-level covers on one thread and want
    /// to handle the conflict rather than crash.
    ///
    /// Defaults to [`try_minimize_with_config`](Self::try_minimize_with_config) with the default
    /// configuration.
    fn try_minimize(&self) -> Result<Self, MinimizationError>
    where
        Self: Sized,
    {
        self.try_minimize_with_config(&EspressoConfig::default())
    }

    /// [`try_minimize`](Self::try_minimize) with a custom configuration.
    ///
    /// This is one of the two primary methods an implementation must provide.
    fn try_minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError>
    where
        Self: Sized;

    /// Exact counterpart of [`try_minimize`](Self::try_minimize): never panics on an instance
    /// conflict, returning [`MinimizationError::Instance`] instead.
    ///
    /// Defaults to [`try_minimize_exact_with_config`](Self::try_minimize_exact_with_config) with the
    /// default configuration.
    fn try_minimize_exact(&self) -> Result<Self, MinimizationError>
    where
        Self: Sized,
    {
        self.try_minimize_exact_with_config(&EspressoConfig::default())
    }

    /// [`try_minimize_exact`](Self::try_minimize_exact) with a custom configuration.
    ///
    /// This is one of the two primary methods an implementation must provide.
    fn try_minimize_exact_with_config(
        &self,
        config: &EspressoConfig,
    ) -> Result<Self, MinimizationError>
    where
        Self: Sized;

    /// Minimise using the heuristic Espresso algorithm.
    ///
    /// Returns a new minimised instance without modifying the original. Fast and near-optimal
    /// (~99% optimal in practice).
    ///
    /// # Panics
    ///
    /// Panics if a low-level [`Espresso`](crate::espresso::Espresso) / `EspressoCover` of a
    /// *different* dimension is already live on the current thread — a usage error, since the
    /// high-level API otherwise manages the instance lifecycle automatically. Use
    /// [`try_minimize`](Self::try_minimize) to handle that case as a recoverable error instead.
    fn minimize(&self) -> Result<Self, MinimizationError>
    where
        Self: Sized,
    {
        self.minimize_with_config(&EspressoConfig::default())
    }

    /// [`minimize`](Self::minimize) with a custom configuration.
    ///
    /// # Panics
    ///
    /// See [`minimize`](Self::minimize).
    fn minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError>
    where
        Self: Sized,
    {
        panic_on_instance_conflict(self.try_minimize_with_config(config))
    }

    /// Minimise using exact minimisation.
    ///
    /// Returns a new minimised instance without modifying the original. Guarantees a minimal result
    /// but can be slower for large inputs.
    ///
    /// # Panics
    ///
    /// See [`minimize`](Self::minimize); use [`try_minimize_exact`](Self::try_minimize_exact) to
    /// handle an instance conflict as an error.
    fn minimize_exact(&self) -> Result<Self, MinimizationError>
    where
        Self: Sized,
    {
        self.minimize_exact_with_config(&EspressoConfig::default())
    }

    /// [`minimize_exact`](Self::minimize_exact) with a custom configuration.
    ///
    /// # Panics
    ///
    /// See [`minimize`](Self::minimize).
    fn minimize_exact_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError>
    where
        Self: Sized,
    {
        panic_on_instance_conflict(self.try_minimize_exact_with_config(config))
    }
}

/// Convert an instance-conflict error into a panic, passing every other result through unchanged.
///
/// Backs the panicking `minimize*` methods: a [`MinimizationError::Instance`] means a low-level
/// Espresso of different dimensions is live on this thread — a usage error, not a recoverable
/// condition — so it is raised loudly. All other errors (cube validation, IO) are returned as-is.
fn panic_on_instance_conflict<T>(
    result: Result<T, MinimizationError>,
) -> Result<T, MinimizationError> {
    if let Err(MinimizationError::Instance(e)) = &result {
        panic!(
            "Espresso instance conflict during minimisation: {e}. A low-level Espresso/EspressoCover \
             of different dimensions is live on this thread; drop it first, or use the try_minimize* \
             methods to handle this as an error."
        );
    }
    result
}

/// Private helper function to minimise a Cover using either heuristic or exact algorithm.
///
/// The caller constructs the [`Espresso`](crate::espresso::Espresso) instance (via `new` to panic on
/// an instance conflict, or `try_new` to surface it as an error) and passes it in — keeping the
/// panic-vs-error policy at the trait boundary, not buried here. `esp` must stay live for the whole
/// call since [`EspressoCover::from_cubes`] reads the thread's current instance.
fn minimize_cover_with<F, I, O>(
    cover: &Cover<I, O>,
    esp: &crate::espresso::Espresso,
    minimize_fn: F,
) -> Result<Cover<I, O>, MinimizationError>
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
    use crate::espresso::EspressoCover;

    // Split cubes into F, D, R sets based on cube type
    let mut f_cubes = Vec::new();
    let mut d_cubes = Vec::new();
    let mut r_cubes = Vec::new();

    for cube in cover.cubes.iter() {
        let input_vec: Vec<u8> = cube
            .inputs()
            .iter()
            .map(|opt| match opt {
                Some(false) => 0,
                Some(true) => 1,
                None => 2,
            })
            .collect();

        // Output mask: 1 where this cube asserts the output, 0 otherwise.
        let output_vec: Vec<u8> = (0..cover.num_outputs())
            .map(|i| if cube.asserts(i) { 1 } else { 0 })
            .collect();

        // Send to appropriate set based on the cube's set.
        match cube.cube_type() {
            CubeType::F => f_cubes.push((input_vec, output_vec)),
            CubeType::D => d_cubes.push((input_vec, output_vec)),
            CubeType::R => r_cubes.push((input_vec, output_vec)),
        }
    }

    // `esp` (the thread's Espresso instance) is supplied by the caller. Direct C calls below are
    // thread-safe via thread-local storage.

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
        minimize_fn(esp, &f_cover, d_cover.as_ref(), r_cover.as_ref());

    // Extract minimised cubes back onto the cover's shared symbol tables.
    let ni = cover.num_inputs();
    let no = cover.num_outputs();
    let input_symbols = Arc::clone(cover.input_symbols());
    let output_symbols = Arc::clone(cover.output_symbols());
    // Espresso returns anonymous positional cubes (`Cube<Anonymous, Anonymous>`); re-point each onto the
    // cover's real `Symbols<L>` tables by reading values positionally (variable order is preserved across
    // the boundary). Same operation as building any cover from anonymous cubes — see `repoint`.
    let rehome = |cube: &Cube<Anonymous, Anonymous>| -> Cube<I, O> {
        super::repoint(cube, &input_symbols, &output_symbols)
    };

    // Keep all three computed sets. They are NOT inert: the cover carries them so it can later be
    // written in a richer output format than its declared `.type` (e.g. a CLI `-o fdr` on an FD cover
    // emits the computed OFF-set). `cubes()`/`num_cubes()` filter by cover type for reads, but the PLA
    // writer emits by the requested format, which is why the D/R cubes must be retained.
    let f_cubes = f_result.to_cubes(ni, no, CubeType::F);
    let d_cubes = d_result.to_cubes(ni, no, CubeType::D);
    let r_cubes = r_result.to_cubes(ni, no, CubeType::R);

    // Build new cover with minimised cubes - reuse the cover's symbol tables (Arc, cheap)
    Ok(Cover {
        input_symbols: Arc::clone(cover.input_symbols()),
        output_symbols: Arc::clone(cover.output_symbols()),
        cubes: f_cubes
            .iter()
            .chain(d_cubes.iter())
            .chain(r_cubes.iter())
            .map(rehome)
            .collect(),
        cover_type: cover.cover_type,
    })
}

// Implement public Minimizable trait for Cover (any label type — minimisation is positional).
//
// The fallible `try_*` primitives construct the thread's Espresso via `try_new` (instance conflict
// → error); the panicking `minimize*` methods are the trait defaults wrapping these.
impl<I, O> Minimizable for Cover<I, O> {
    fn try_minimize_with_config(&self, config: &EspressoConfig) -> Result<Self, MinimizationError> {
        let esp = crate::espresso::Espresso::try_new(
            self.num_inputs(),
            self.num_outputs(),
            Some(config),
        )?;
        minimize_cover_with(self, &esp, |esp, f, d, r| esp.minimize(f, d, r))
    }

    fn try_minimize_exact_with_config(
        &self,
        config: &EspressoConfig,
    ) -> Result<Self, MinimizationError> {
        let esp = crate::espresso::Espresso::try_new(
            self.num_inputs(),
            self.num_outputs(),
            Some(config),
        )?;
        minimize_cover_with(self, &esp, |esp, f, d, r| esp.minimize_exact(f, d, r))
    }
}

/// Minimize a `BoolExpr` by round-tripping through a single-output [`Cover`].
///
/// Workflow: `BoolExpr` → single-output `Cover` (product terms extracted from the internal
/// BDD) → Espresso minimisation → rebuild a `BoolExpr` from the minimised product terms. The
/// minimised cubes are cached on the result so subsequent cube extraction reflects them.
fn minimize_expr_with<F>(
    expr: &BoolExpr,
    config: &EspressoConfig,
    minimize_fn: F,
) -> Result<BoolExpr, MinimizationError>
where
    F: FnOnce(
        &Cover<Symbol, Anonymous>,
        &EspressoConfig,
    ) -> Result<Cover<Symbol, Anonymous>, MinimizationError>,
{
    // Build a single-output, anonymous-output cover from the expression (canonical via the BDD).
    let cover: Cover<Symbol, Anonymous> = expr.into();

    // Minimise it with the provided (heuristic or exact) algorithm.
    let minimized = minimize_fn(&cover, config)?;

    // Rebuild a BoolExpr from the minimised product terms of the single output.
    let terms = minimized.output_product_terms(0);
    Ok(BoolExpr::from_cubes(terms))
}

/// Implement the public [`Minimizable`] trait for [`BoolExpr`].
///
/// Boolean expressions minimise by extracting their product terms (from the internal BDD) into a
/// single-output [`Cover`], running Espresso, and reconstructing an expression from the result.
impl Minimizable for BoolExpr {
    fn try_minimize_with_config(
        &self,
        config: &crate::EspressoConfig,
    ) -> Result<Self, MinimizationError> {
        minimize_expr_with(self, config, |cover, cfg| {
            cover.try_minimize_with_config(cfg)
        })
    }

    fn try_minimize_exact_with_config(
        &self,
        config: &crate::EspressoConfig,
    ) -> Result<Self, MinimizationError> {
        minimize_expr_with(self, config, |cover, cfg| {
            cover.try_minimize_exact_with_config(cfg)
        })
    }
}
