//! Iterator types for covers
//!
//! This module provides iterator types for traversing cubes and converting
//! outputs to boolean expressions.

use super::Cover;
use crate::expression::BoolExpr;
use std::sync::Arc;

/// Iterator over filtered cubes with generic yield type
///
/// This iterator wraps a filtered cube iterator and can yield different types
/// depending on how the cubes are transformed (references, owned data, etc.).
pub struct CubesIter<'a, T> {
    pub(super) iter: Box<dyn Iterator<Item = T> + 'a>,
}

impl<'a, T> Iterator for CubesIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Iterator over output expressions from a Cover
///
/// This iterator uses the visitor pattern to generate boolean expressions
/// on-demand for each output in the cover. It maintains state (current index)
/// and calls the cover's conversion method during iteration.
pub struct ToExprs<'a> {
    pub(super) cover: &'a Cover,
    pub(super) current_idx: usize,
}

impl<'a> Iterator for ToExprs<'a> {
    type Item = (Arc<str>, BoolExpr);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_idx >= self.cover.num_outputs {
            return None;
        }
        let idx = self.current_idx;
        self.current_idx += 1;

        // Use provided label or generate default
        let name = if let Some(label) = self.cover.output_labels.get(idx) {
            Arc::clone(label)
        } else {
            Arc::from(format!("y{}", idx).as_str())
        };

        let expr = self
            .cover
            .to_expr_by_index(idx)
            .unwrap_or_else(|_| BoolExpr::constant(false));
        Some((name, expr))
    }
}
