//! Iterator types for covers
//!
//! This module provides iterator types for traversing cubes and converting
//! outputs to boolean expressions.

use super::label::StringLabel;
use super::Cover;
use crate::expression::BoolExpr;
use std::fmt;

/// Iterator over filtered cubes with generic yield type
///
/// This iterator wraps a filtered cube iterator and can yield different types
/// depending on how the cubes are transformed (references, owned data, etc.).
pub struct CubesIter<'a, T> {
    pub(super) iter: Box<dyn Iterator<Item = T> + 'a>,
}

/// The wrapped trait-object iterator can't be introspected, so this is opaque.
impl<T> fmt::Debug for CubesIter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CubesIter").finish_non_exhaustive()
    }
}

impl<'a, T> Iterator for CubesIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Iterator over output expressions from a [`Cover`], created by [`Cover::to_exprs`].
///
/// Generates boolean expressions on-demand for each output, yielding the output label (borrowed from
/// the cover) paired with the rebuilt expression. Generic over the cover's input label `I` (which must
/// be string-like, any [`StringLabel`], to name the variables) and output label `O`.
pub struct ToExprs<'a, I, O> {
    pub(super) cover: &'a Cover<I, O>,
    pub(super) current_idx: usize,
}

/// Reports progress without requiring the label types to be `Debug` (the borrowed cover is elided).
impl<I, O> fmt::Debug for ToExprs<'_, I, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToExprs")
            .field("current_idx", &self.current_idx)
            .field("num_outputs", &self.cover.num_outputs())
            .finish_non_exhaustive()
    }
}

impl<'a, I: StringLabel, O> Iterator for ToExprs<'a, I, O> {
    type Item = (&'a O, BoolExpr<I>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_idx >= self.cover.num_outputs() {
            return None;
        }
        let idx = self.current_idx;
        self.current_idx += 1;

        // The output label at this position (one label per output — `Symbols` is never partial).
        let label = &self.cover.output_symbols().labels()[idx];
        let expr = self
            .cover
            .to_expr_by_index(idx)
            .unwrap_or_else(|_| BoolExpr::constant(false));
        Some((label, expr))
    }
}
