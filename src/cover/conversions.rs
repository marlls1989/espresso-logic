//! Trait implementations for Cover
//!
//! This module provides conversions and trait implementations for [`Cover`],
//! including PLA I/O, Debug formatting, and conversions from expressions.

use super::cubes::{Cube, CubeType};
use super::label::Anonymous;
use super::minterm::{InputField, Minterm};
use super::output_set::OutputSet;
use super::symbols::Symbols;
use super::Cover;
use super::CoverType;
use crate::bdd::{Bdd, Brand, ManagerCell};
use crate::expression::BoolExpr;
use crate::Symbol;
use std::fmt;
use std::sync::Arc;

/// Raw parsed cube data from the PLA reader: `(input fields, output-membership mask, set)`. The input
/// side uses [`InputField`] (not `Option<bool>`) so the empty literal (`?`) survives into the minterm.
pub(crate) type RawCube = (Vec<InputField>, Vec<bool>, CubeType);

/// Build a positional [`Cover<Anonymous, Anonymous>`](Cover) from raw parsed PLA cubes. The PLA reader
/// then relabels the present sides (`.ilb`/`.ob`) to select a [`PlaCover`](super::pla::PlaCover) variant
/// — there are no synthesised placeholder names, so an unlabelled side stays `Anonymous`.
pub(crate) fn anonymous_cover_from_raw(
    num_inputs: usize,
    num_outputs: usize,
    cubes: Vec<RawCube>,
    cover_type: CoverType,
) -> Cover<Anonymous, Anonymous> {
    let input_symbols = Symbols::<Anonymous>::anonymous(num_inputs);
    let output_symbols = Symbols::<Anonymous>::anonymous(num_outputs);

    let cubes = cubes
        .into_iter()
        .map(|(mut inputs, mask, set)| {
            inputs.resize(num_inputs, InputField::DontCare);
            let im = Minterm::from_symbols_input_fields(Arc::clone(&input_symbols), inputs);
            let om = OutputSet::from_symbols(Arc::clone(&output_symbols), mask.iter().copied());
            Cube::new(im, om, set)
        })
        .collect();

    Cover {
        input_symbols,
        output_symbols,
        cubes,
        cover_type,
    }
}

/// The `Bdd → Cover` primitive: enumerate a handle's ON-set as a single-output, anonymous-output
/// [`Cover<Symbol, Anonymous>`](Cover) via [`Bdd::to_cubes`](crate::bdd::Bdd::to_cubes).
///
/// This is the single source of truth for materialising a BDD as a cover. The `BoolExpr` conversions
/// below funnel through it, and the named-output [`Cover::add_bdd`]/[`Cover::add_expr`] share the same
/// underlying [`Bdd::to_cubes`](crate::bdd::Bdd::to_cubes) extraction. The output is **anonymous**
/// (`O = Anonymous`) — a Boolean function has no output name; label it with
/// [`relabel_outputs`](Cover::relabel_outputs) if needed.
///
/// ```
/// use espresso_logic::{bdd_builder, Anonymous, Cover, Symbol};
///
/// let builder = bdd_builder!();
/// let f = builder.var("a") & builder.var("b");
/// let cover: Cover<Symbol, Anonymous> = f.into();
/// assert_eq!(cover.num_outputs(), 1);
/// ```
impl<B: Brand, C: ManagerCell> From<Bdd<B, C>> for Cover<Symbol, Anonymous> {
    fn from(bdd: Bdd<B, C>) -> Self {
        bdd.to_cubes()
    }
}

/// Borrowed counterpart of the `From<Bdd>` impl: [`Bdd::to_cubes`] already borrows `&self`, so this
/// defers straight to it.
impl<B: Brand, C: ManagerCell> From<&Bdd<B, C>> for Cover<Symbol, Anonymous> {
    fn from(bdd: &Bdd<B, C>) -> Self {
        bdd.to_cubes()
    }
}

/// Convert a `BoolExpr` into a single-output, anonymous-output [`Cover<Symbol, Anonymous>`](Cover).
///
/// A free [`BoolExpr`] has no cubes, so it is first built into a [`Bdd`] in a private,
/// temporary builder (which canonicalises it), then materialised through the `From<Bdd>` primitive
/// above — the same temporary-builder mediation as [`Cover::add_expr`]. The output is anonymous; an
/// expression has no output name.
///
/// Each conversion builds and drops a throwaway BDD manager. Building many expressions into one cover
/// goes through a single [`bdd_builder!`](crate::bdd_builder) plus
/// [`Cover::add_bdd`](crate::Cover::add_bdd), which share one manager across all of them.
///
/// ```
/// use espresso_logic::{Anonymous, BoolExpr, Cover, Symbol};
///
/// let expr = BoolExpr::var("a") & BoolExpr::var("b");
/// let cover: Cover<Symbol, Anonymous> = expr.into();
/// assert_eq!(cover.num_outputs(), 1);
/// ```
impl From<BoolExpr> for Cover<Symbol, Anonymous> {
    fn from(expr: BoolExpr) -> Self {
        Cover::from(&expr)
    }
}

/// Borrowed counterpart of the `From<BoolExpr>` impl: builds the expression in a temporary builder and
/// funnels through the `From<Bdd>` primitive, without taking ownership.
///
/// ```
/// use espresso_logic::{Anonymous, BoolExpr, Cover, Symbol};
///
/// let a = BoolExpr::var("a");
/// let cover = Cover::<Symbol, Anonymous>::from(&a);
/// assert_eq!(cover.num_outputs(), 1);
/// ```
impl From<&BoolExpr> for Cover<Symbol, Anonymous> {
    fn from(expr: &BoolExpr) -> Self {
        // The temporary builder lives for this call; the handle borrows it and is consumed by the
        // `Bdd → Cover` primitive before this function returns.
        let builder = crate::bdd_builder!();
        Cover::from(builder.build(expr))
    }
}

impl<I: fmt::Debug, O: fmt::Debug> fmt::Debug for Cover<I, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cover")
            .field("num_inputs", &self.num_inputs())
            .field("num_outputs", &self.num_outputs())
            .field("cover_type", &self.cover_type)
            .field("num_cubes", &self.num_cubes())
            .field("input_labels", &self.input_symbols().labels())
            .field("output_labels", &self.output_symbols().labels())
            .finish()
    }
}

/// Renders the cover as its sum-of-products body: one [`Cube`] per line, in order, each a
/// PLA-style `<inputs> <outputs>` row. No `.i`/`.o` header is emitted — use the
/// [`PLAWriter`](crate::PLAWriter) for a complete PLA file. Needs no bound on the label types.
impl<I, O> fmt::Display for Cover<I, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, cube) in self.cubes().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{cube}")?;
        }
        Ok(())
    }
}
