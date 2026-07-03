//! Minimizable trait and implementations for Boolean function minimisation
//!
//! This module provides the [`Minimizable`] trait which defines a uniform interface
//! for minimising Boolean functions using the Espresso algorithm, along with implementations
//! for [`Cover`] and [`BoolExpr`].

use super::cubes::{Cube, CubeType};
use super::label::{Anonymous, Label};
use super::minterm::Minterm;
use super::output_set::OutputSet;
use super::symbols::Symbols;
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

    // Pre-minimisation normalise + partition: split cubes into F, D, R sets, dropping any vacuous cube
    // (one with an empty `00` input field). A vacuous cube covers no minterm, so removing it leaves
    // the function unchanged — and keeps it away from Espresso's `expand`, which mishandles it (it is
    // the very thing C's own verification flags).
    let mut f_cubes: Vec<&Cube<I, O>> = Vec::new();
    let mut d_cubes: Vec<&Cube<I, O>> = Vec::new();
    let mut r_cubes: Vec<&Cube<I, O>> = Vec::new();
    for cube in cover.cubes.iter() {
        if cube.inputs().is_vacuous() {
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
    let rehome = |cube: Cube<Anonymous, Anonymous>| -> Cube<I, O> {
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
        cubes: f_cubes.chain(d_cubes).chain(r_cubes).map(rehome).collect(),
        cover_type: cover.cover_type,
    })
}

/// Generate the complete prime-implicant set of one cube-set, re-homed onto the caller's symbols.
///
/// Shared engine behind [`Cover::primes`] and the projection path of [`Cover::over_vars`]. It
/// marshals `f_cubes` (the ON-set, taken relative to the optional `d_cubes` don't-care set) into
/// Espresso, runs [`Espresso::primes`](crate::espresso::Espresso::primes), and re-homes every
/// returned prime onto `input_symbols`/`output_symbols`, tagging each with `tag`. Vacuous cubes
/// (empty `00` input field) are dropped first: they cover no minterm, and the prime generator
/// mishandles them — mirroring the pre-minimisation filter in [`minimize_cover_with`].
///
/// Infallible: an Espresso instance conflict (a live low-level instance of different dimensions on
/// this thread) or a C fatal is a usage error here and panics, matching the panicking `minimize`.
pub(crate) fn primes_cubes<I, O>(
    input_symbols: &Arc<Symbols<I>>,
    output_symbols: &Arc<Symbols<O>>,
    f_cubes: &[&Cube<I, O>],
    d_cubes: &[&Cube<I, O>],
    tag: CubeType,
) -> Vec<Cube<I, O>> {
    use crate::espresso::EspressoCover;

    let ni = input_symbols.arity();
    let no = output_symbols.arity();

    // Drop vacuous cubes up front, as `minimize_cover_with` does.
    let f: Vec<&Cube<I, O>> = f_cubes
        .iter()
        .copied()
        .filter(|c| !c.inputs().is_vacuous())
        .collect();
    let d: Vec<&Cube<I, O>> = d_cubes
        .iter()
        .copied()
        .filter(|c| !c.inputs().is_vacuous())
        .collect();

    // Arity-0 fast path: with no inputs each output is a constant, and Espresso is not trusted with a
    // zero-input cover. Output `j` is constant-1 iff some non-vacuous F/D cube asserts it; the single
    // prime is the empty cube asserting exactly those outputs.
    if ni == 0 {
        let asserted: Vec<bool> = (0..no)
            .map(|j| f.iter().chain(d.iter()).any(|c| c.asserts(j)))
            .collect();
        if asserted.iter().any(|&a| a) {
            return vec![Cube::new(
                Minterm::from_symbols(Arc::clone(input_symbols), std::iter::empty()),
                OutputSet::from_symbols(Arc::clone(output_symbols), asserted),
                tag,
            )];
        }
        return Vec::new();
    }

    // Marshal each set by copying the cubes' packed input words plus a per-output assertion bit,
    // exactly as `minimize_cover_with` does. The Espresso instance is created first so that
    // `from_packed_cubes` (which reads the thread's current instance) sees the right dimensions.
    let esp = panic_on_instance_conflict(crate::espresso::Espresso::try_new(ni, no, None))
        .unwrap_or_else(|e| panic!("Espresso prime generation failed: {e}"));
    let to_cover = |cubes: &[&Cube<I, O>]| -> EspressoCover {
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
        panic_on_instance_conflict(EspressoCover::from_packed_cubes(&refs, ni, no))
            .unwrap_or_else(|e| panic!("Espresso prime generation failed: {e}"))
    };

    let f_cover = to_cover(&f);
    let d_cover = if d.is_empty() {
        None
    } else {
        Some(to_cover(&d))
    };
    let result = panic_on_instance_conflict(esp.try_primes(&f_cover, d_cover.as_ref()))
        .unwrap_or_else(|e| panic!("Espresso prime generation failed: {e}"));

    // Re-home the anonymous positional primes onto the caller's real symbol tables (same arity, same
    // packed layout), tagging each with `tag`.
    result
        .to_cubes(ni, no, tag)
        .map(|cube| {
            let im = Minterm::from_packed_words(
                Arc::clone(input_symbols),
                Arc::clone(cube.inputs().packed()),
            );
            let om = OutputSet::from_packed_bits(
                Arc::clone(output_symbols),
                Arc::clone(cube.outputs().packed()),
            );
            Cube::new(im, om, tag)
        })
        .collect()
}

impl<I: Label, O: Clone> Cover<I, O> {
    /// The complete set of prime implicants of the ON-set, taken relative to the don't-care set.
    ///
    /// Returns *every* prime implicant, not the reduced, irredundant cover that
    /// [`minimize`](Minimizable::minimize) produces — including consensus primes an irredundant cover
    /// discards. This is the [`Bdd`](crate::bdd::Bdd)-free counterpart of the reference tool's
    /// `-Dprimes` mode. Any don't-care (D) and OFF-set (R) cubes the cover carries are preserved
    /// unchanged, and the [`CoverType`](crate::CoverType) is retained.
    ///
    /// # Panics
    ///
    /// Panics if a low-level Espresso instance of different dimensions is live on this thread (drop it
    /// first), or if the C core reports a fatal condition.
    ///
    /// # Examples
    ///
    /// ```
    /// use espresso_logic::{Cover, CoverType, Cube, CubeType, Symbol};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // f = a·x + b·x̄ + a·b — the consensus prime a·b is redundant in a minimal cover, but it is a
    /// // genuine prime implicant, so `primes` keeps it.
    /// let f = Cover::<Symbol, Symbol>::from_cubes(
    ///     CoverType::F,
    ///     [
    ///         Cube::with_labels(&[("a", Some(true)), ("x", Some(true))], &[("o", true)], CubeType::F)?,
    ///         Cube::with_labels(&[("b", Some(true)), ("x", Some(false))], &[("o", true)], CubeType::F)?,
    ///         Cube::with_labels(&[("a", Some(true)), ("b", Some(true))], &[("o", true)], CubeType::F)?,
    ///     ],
    /// );
    /// let primes = f.primes();
    /// assert_eq!(primes.num_cubes(), 3);
    /// // The consensus prime a·b (don't-care on x) is present.
    /// assert!(primes.cubes().any(|c| c.inputs().value_of("a") == Some(true)
    ///     && c.inputs().value_of("b") == Some(true)
    ///     && c.inputs().value_of("x").is_none()));
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn primes(&self) -> Cover<I, O> {
        let f_refs: Vec<&Cube<I, O>> = self
            .cubes
            .iter()
            .filter(|c| c.cube_type() == CubeType::F)
            .collect();
        let d_refs: Vec<&Cube<I, O>> = self
            .cubes
            .iter()
            .filter(|c| c.cube_type() == CubeType::D)
            .collect();

        let mut cubes = primes_cubes(
            self.input_symbols(),
            self.output_symbols(),
            &f_refs,
            &d_refs,
            CubeType::F,
        );
        // Carry the don't-care (D) and OFF-set (R) cubes through unchanged, exactly as `minimize`
        // retains its computed sets, so the cover can still be read/written in its declared type.
        for c in &self.cubes {
            if matches!(c.cube_type(), CubeType::D | CubeType::R) {
                cubes.push(c.clone());
            }
        }

        Cover::from_parts(
            Arc::clone(self.input_symbols()),
            Arc::clone(self.output_symbols()),
            cubes,
            self.cover_type,
        )
    }
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
