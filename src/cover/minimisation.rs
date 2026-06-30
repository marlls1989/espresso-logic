//! Minimizable trait and implementations for Boolean function minimisation
//!
//! This module provides the [`Minimizable`] trait which defines a uniform interface
//! for minimising Boolean functions using the Espresso algorithm, along with implementations
//! for [`Cover`] and [`BoolExpr`].

use super::cubes::{Cube, CubeType};
use super::label::Anonymous;
use super::minterm::Minterm;
use super::output_set::OutputSet;
use super::Cover;
use crate::espresso::error::MinimizationError;
use crate::EspressoConfig;
use std::sync::Arc;

/// Public trait for types that can be minimised using Espresso
///
/// This trait provides a **transparent, uniform interface** for minimising boolean functions
/// using the Espresso algorithm. All methods take `&self` and return a new minimised instance,
/// following an immutable functional style.
///
/// **Note (v3.1+):** You must explicitly import this trait to use its methods:
/// ```
/// use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};
///
/// let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
/// cover.push(Cube::anonymous(&[Some(true), Some(true)], &[true], CubeType::F));
/// let minimized = cover.minimize()?;  // Requires `use Minimizable`
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// # Transparent Minimisation
///
/// Call `.minimize()` on a supported type and get back a minimised value of the same type:
///
/// ```
/// use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};
///
/// # fn main() -> std::io::Result<()> {
/// let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
/// cover.push(Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F)); // 01 -> 1
/// cover.push(Cube::anonymous(&[Some(true), Some(false)], &[true], CubeType::F)); // 10 -> 1
///
/// let min_cover = cover.minimize()?;
/// println!("Minimized cover has {} cubes", min_cover.num_cubes());
/// # Ok(())
/// # }
/// ```
///
/// # Implementations
///
/// - **[`Cover`]**: Direct implementation — minimises cubes directly with Espresso.
///
/// To minimise a [`BoolExpr`](crate::BoolExpr), build it into a [`Bdd`](crate::bdd::Bdd) in a builder
/// and call [`Bdd::minimize`](crate::bdd::Bdd::minimize) (or
/// [`BddBuilder::minimize`](crate::bdd::BddBuilder::minimize)), which returns a [`Cover`].
///
/// This trait is **not sealed**: it is intentionally open so downstream crates may implement it for
/// their own types (for example, to forward minimisation through a wrapper). An implementation only
/// needs to provide [`try_minimize_with_config`](Self::try_minimize_with_config) and
/// [`try_minimize_exact_with_config`](Self::try_minimize_exact_with_config); the remaining methods
/// have defaults.
///
/// # Validation
///
/// A cover is validated before it is handed to the C core, so two inputs that would otherwise make
/// the C library abort the whole process (`exit(1)`) instead surface as a recoverable
/// [`MinimizationError`] from the `try_*` methods:
/// - [`NonOrthogonal`](MinimizationError::NonOrthogonal) — an `FR`/`FDR` cover whose ON-set and
///   OFF-set overlap (some minterm is asserted as both 1 and 0 for the same output).
/// - [`Instance`](MinimizationError::Instance) wrapping
///   [`DimensionTooLarge`](crate::espresso::InstanceError::DimensionTooLarge) — a dimension too large
///   for the C core's 32-bit cube indices.
///
/// A cube with an *empty* input field (the PLA `?` literal) covers no minterm, so it is dropped.
///
/// [`Cover`]: crate::Cover
///
/// # Immutable Design
///
/// All minimisation methods preserve the original and return a new minimised instance:
///
/// ```
/// use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};
///
/// # fn main() -> std::io::Result<()> {
/// let mut original = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
/// original.push(Cube::anonymous(&[Some(true), Some(true)], &[true], CubeType::F));
/// original.push(Cube::anonymous(&[Some(true), Some(true), Some(true)], &[true], CubeType::F));
///
/// let minimized = original.minimize()?;
///
/// // Original is unchanged.
/// println!("Original has {} cubes", original.num_cubes());
/// println!("Minimized has {} cubes", minimized.num_cubes());
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

    let no = cover.num_outputs();
    let ni = cover.num_inputs();

    // Pre-minimisation normalise + partition: split cubes into F, D, R sets, dropping any cube with an
    // empty (`00`) input field. An empty field means the cube covers no minterm, so removing it leaves
    // the function unchanged — and keeps the vacuous cube away from Espresso's `expand`, which
    // mishandles it (it is the very thing C's own verification flags).
    let mut f_cubes: Vec<&Cube<I, O>> = Vec::new();
    let mut d_cubes: Vec<&Cube<I, O>> = Vec::new();
    let mut r_cubes: Vec<&Cube<I, O>> = Vec::new();
    for cube in cover.cubes.iter() {
        if cube.inputs().has_empty_field() {
            continue;
        }
        match cube.cube_type() {
            CubeType::F => f_cubes.push(cube),
            CubeType::D => d_cubes.push(cube),
            CubeType::R => r_cubes.push(cube),
        }
    }

    // Sanity check: the ON-set and OFF-set must be orthogonal. If a minterm is asserted as both 1 and
    // 0 for the same output, the cover is contradictory and the C core's `expand` would `exit(1)` the
    // whole process; reject it as a recoverable error instead.
    if !f_cubes.is_empty() && !r_cubes.is_empty() {
        for fc in &f_cubes {
            for rc in &r_cubes {
                if !fc.inputs().is_disjoint_same_header(rc.inputs()) {
                    if let Some(output) = (0..no).find(|&o| fc.asserts(o) && rc.asserts(o)) {
                        return Err(MinimizationError::NonOrthogonal { output });
                    }
                }
            }
        }
    }

    // `esp` (the thread's Espresso instance) is supplied by the caller. Direct C calls below are
    // thread-safe via thread-local storage. Marshal each set by copying the cubes' packed input words
    // straight into the C cube (same 2-bit encoding) plus a per-output assertion bit.
    let to_cover = |cubes: &[&Cube<I, O>]| -> Result<EspressoCover, MinimizationError> {
        let data: Vec<(&[u64], Vec<bool>)> = cubes
            .iter()
            .map(|c| {
                (
                    c.inputs().raw_words(),
                    (0..no).map(|i| c.asserts(i)).collect(),
                )
            })
            .collect();
        let refs: Vec<(&[u64], &[bool])> = data.iter().map(|(w, o)| (*w, o.as_slice())).collect();
        EspressoCover::from_packed_cubes(&refs, ni, no)
    };

    let f_cover = to_cover(&f_cubes)?;
    let d_cover = if d_cubes.is_empty() {
        None
    } else {
        Some(to_cover(&d_cubes)?)
    };
    let r_cover = if r_cubes.is_empty() {
        None
    } else {
        Some(to_cover(&r_cubes)?)
    };

    // Call the provided minimize function (heuristic or exact)
    let (f_result, d_result, r_result) =
        minimize_fn(esp, &f_cover, d_cover.as_ref(), r_cover.as_ref());

    // Extract minimised cubes back onto the cover's shared symbol tables (`ni`/`no` from above).
    let input_symbols = Arc::clone(cover.input_symbols());
    let output_symbols = Arc::clone(cover.output_symbols());
    // Espresso returns anonymous positional cubes (`Cube<Anonymous, Anonymous>`) at exactly the cover's
    // arity, in the same packed layout. Re-home each onto the cover's real `Symbols<L>` tables by
    // cloning the packed-word `Arc`s (the packing is independent of the label type, and variable order
    // is preserved across the boundary) — no per-variable re-packing. Unlike the identity-union
    // re-home in `push`/`from_cubes`, this needs no padding/projection because the arities already match.
    let rehome = |cube: &Cube<Anonymous, Anonymous>| -> Cube<I, O> {
        let im = Minterm::from_packed_words(
            Arc::clone(&input_symbols),
            Arc::clone(cube.inputs().packed()),
        );
        let om = OutputSet::from_packed_bits(
            Arc::clone(&output_symbols),
            Arc::clone(cube.outputs().packed()),
        );
        Cube::new(im, om, cube.cube_type())
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
