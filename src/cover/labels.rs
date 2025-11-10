//! Internal label management utilities
//!
//! This module provides the [`LabelManager`] type for managing variable labels
//! with automatic conflict resolution and efficient lookup.

use std::collections::HashMap;
use std::sync::Arc;

/// Generic label manager for input/output variables with configurable prefix
///
/// Maintains both ordered labels (Vec) and fast name->index lookup (HashMap).
/// Handles conflict resolution by finding next available sequential label.
#[derive(Clone, Debug)]
pub(super) struct LabelManager<const PREFIX: char> {
    /// Ordered labels by position
    pub(super) labels: Vec<Arc<str>>,
    /// Fast lookup: label name -> position index
    label_map: HashMap<Arc<str>, usize>,
}

impl<const PREFIX: char> LabelManager<PREFIX> {
    /// Create a new empty label manager
    pub(super) fn new() -> Self {
        Self {
            labels: Vec::new(),
            label_map: HashMap::new(),
        }
    }

    /// Create from existing labels
    pub(super) fn from_labels(labels: Vec<Arc<str>>) -> Self {
        let label_map = labels
            .iter()
            .enumerate()
            .map(|(i, label)| (Arc::clone(label), i))
            .collect();
        Self { labels, label_map }
    }

    /// Check if empty
    pub(super) fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }

    /// Get label at position
    pub(super) fn get(&self, index: usize) -> Option<&Arc<str>> {
        self.labels.get(index)
    }

    /// Get labels slice
    pub(super) fn as_slice(&self) -> &[Arc<str>] {
        &self.labels
    }

    /// Find position by label name (O(1) lookup)
    pub(super) fn find_position(&self, name: &str) -> Option<usize> {
        let key: Arc<str> = Arc::from(name);
        self.label_map.get(&key).copied()
    }

    /// Check if label exists
    pub(super) fn contains(&self, name: &str) -> bool {
        let key: Arc<str> = Arc::from(name);
        self.label_map.contains_key(&key)
    }

    /// Find the next available sequential label index starting from `start`
    /// E.g., if x0, x1, x3 exist and start=2, returns 2 (first available from start)
    fn next_available_index(&self, start: usize) -> usize {
        let mut n = start;
        loop {
            let candidate = Arc::from(format!("{}{}", PREFIX, n).as_str());
            if !self.label_map.contains_key(&candidate) {
                return n;
            }
            n += 1;
        }
    }

    /// Add a label at the given position, checking for conflicts
    /// If conflict, finds next available sequential label starting from position
    pub(super) fn add_with_conflict_resolution(&mut self, position: usize) {
        // Try natural label first (e.g., x2 for position 2)
        let natural_label = Arc::from(format!("{}{}", PREFIX, position).as_str());
        let label = if !self.label_map.contains_key(&natural_label) {
            natural_label
        } else {
            // Conflict - find next available sequential label starting from position
            let n = self.next_available_index(position);
            Arc::from(format!("{}{}", PREFIX, n).as_str())
        };
        self.label_map.insert(Arc::clone(&label), position);
        self.labels.push(label);
    }

    /// Add a specific label at the given position
    pub(super) fn add(&mut self, label: Arc<str>, position: usize) {
        self.label_map.insert(Arc::clone(&label), position);
        self.labels.push(label);
    }

    /// Backfill missing labels up to target size
    pub(super) fn backfill_to(&mut self, target_size: usize) {
        while self.labels.len() < target_size {
            let position = self.labels.len();
            self.add_with_conflict_resolution(position);
        }
    }
}
