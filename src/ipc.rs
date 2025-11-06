//! Inter-process communication protocol for process-isolated Espresso execution
//!
//! This module provides shared memory-based IPC to safely execute Espresso
//! in isolated processes, avoiding the global state issues in the C library.

use serde::{Deserialize, Serialize};

/// Represents a serializable cube in a cover
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedCube {
    /// Raw cube data (bit-packed)
    pub data: Vec<u32>,
}

/// Represents a serializable cover
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedCover {
    /// Number of cubes in the cover
    pub count: usize,
    /// Size of each cube in words (u32)
    pub wsize: usize,
    /// Cube size in bits
    pub sf_size: usize,
    /// Cube data (flattened)
    pub cubes: Vec<SerializedCube>,
}

/// Configuration for Espresso execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcConfig {
    pub debug: bool,
    pub verbose_debug: bool,
    pub trace: bool,
    pub summary: bool,
    pub remove_essential: bool,
    pub force_irredundant: bool,
    pub unwrap_onset: bool,
    pub single_expand: bool,
    pub use_super_gasp: bool,
    pub use_random_order: bool,
}

impl Default for IpcConfig {
    fn default() -> Self {
        IpcConfig {
            debug: false,
            verbose_debug: false,
            trace: false,
            summary: false,
            remove_essential: true,
            force_irredundant: true,
            unwrap_onset: true,
            single_expand: false,
            use_super_gasp: false,
            use_random_order: false,
        }
    }
}
