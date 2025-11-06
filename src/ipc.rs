//! Inter-process communication protocol for process-isolated Espresso execution
//!
//! This module provides shared memory-based IPC to safely execute Espresso
//! in isolated processes, avoiding the global state issues in the C library.

use serde::{Deserialize, Serialize};
use std::io;

/// Maximum size for shared memory segment (16 MB)
pub const MAX_SHARED_MEMORY_SIZE: usize = 16 * 1024 * 1024;

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

/// Request sent to worker process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerRequest {
    /// Initialize cube structure
    Initialize {
        num_inputs: usize,
        num_outputs: usize,
        config: IpcConfig,
    },
    /// Minimize a Boolean function
    Minimize {
        f: SerializedCover,
        d: Option<SerializedCover>,
        r: Option<SerializedCover>,
    },
    /// Exact minimization
    MinimizeExact {
        f: SerializedCover,
        d: Option<SerializedCover>,
        r: Option<SerializedCover>,
    },
    /// Minimize from builder data (builds covers in worker)
    MinimizeFromBuilder {
        f_cubes: Vec<(Vec<u8>, Vec<u8>)>,
        d_cubes: Option<Vec<(Vec<u8>, Vec<u8>)>>,
        r_cubes: Option<Vec<(Vec<u8>, Vec<u8>)>>,
    },
    /// Process a PLA file
    ProcessPla {
        pla_content: Vec<u8>,
        minimize: bool,
    },
    /// Get PLA statistics
    GetPlaStats {
        pla_content: Vec<u8>,
    },
    /// Shutdown the worker
    Shutdown,
}

/// Response from worker process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerResponse {
    /// Initialization successful
    Initialized,
    /// Minimization result
    MinimizeResult {
        f_cover: SerializedCover,
        r_cover: Option<SerializedCover>,
    },
    /// PLA processing result
    PlaResult {
        pla_content: Vec<u8>,
    },
    /// PLA statistics
    PlaStats {
        num_cubes_f: usize,
        num_cubes_d: usize,
        num_cubes_r: usize,
    },
    /// Error occurred
    Error(String),
    /// Shutdown acknowledged
    ShutdownAck,
}

/// Shared memory message wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShmMessage {
    /// Message ID for tracking
    pub msg_id: u64,
    /// Request or response
    pub payload: MessagePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    Request(WorkerRequest),
    Response(WorkerResponse),
}

impl ShmMessage {
    /// Serialize to bytes
    pub fn to_bytes(&self) -> io::Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        bincode::deserialize(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

/// Shared memory layout
///
/// ```text
/// [0..8]: Message length (u64, little-endian)
/// [8..16]: Status flags (u64)
///          bit 0: request_ready
///          bit 1: response_ready
///          bit 2: worker_error
/// [16..MAX_SHARED_MEMORY_SIZE]: Message data
/// ```
pub struct ShmLayout;

impl ShmLayout {
    pub const LEN_OFFSET: usize = 0;
    pub const LEN_SIZE: usize = 8;
    pub const FLAGS_OFFSET: usize = 8;
    pub const FLAGS_SIZE: usize = 8;
    pub const DATA_OFFSET: usize = 16;
    pub const HEADER_SIZE: usize = Self::DATA_OFFSET;

    // Status flags
    pub const FLAG_REQUEST_READY: u64 = 1 << 0;
    pub const FLAG_RESPONSE_READY: u64 = 1 << 1;
    pub const FLAG_WORKER_ERROR: u64 = 1 << 2;

    /// Write message length
    pub fn write_len(mem: &mut [u8], len: usize) {
        let len_bytes = (len as u64).to_le_bytes();
        mem[Self::LEN_OFFSET..Self::LEN_OFFSET + Self::LEN_SIZE].copy_from_slice(&len_bytes);
    }

    /// Read message length
    pub fn read_len(mem: &[u8]) -> usize {
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&mem[Self::LEN_OFFSET..Self::LEN_OFFSET + Self::LEN_SIZE]);
        u64::from_le_bytes(len_bytes) as usize
    }

    /// Write status flags
    pub fn write_flags(mem: &mut [u8], flags: u64) {
        let flag_bytes = flags.to_le_bytes();
        mem[Self::FLAGS_OFFSET..Self::FLAGS_OFFSET + Self::FLAGS_SIZE].copy_from_slice(&flag_bytes);
    }

    /// Read status flags
    pub fn read_flags(mem: &[u8]) -> u64 {
        let mut flag_bytes = [0u8; 8];
        flag_bytes.copy_from_slice(&mem[Self::FLAGS_OFFSET..Self::FLAGS_OFFSET + Self::FLAGS_SIZE]);
        u64::from_le_bytes(flag_bytes)
    }

    /// Set a flag
    pub fn set_flag(mem: &mut [u8], flag: u64) {
        let current = Self::read_flags(mem);
        Self::write_flags(mem, current | flag);
    }

    /// Clear a flag
    pub fn clear_flag(mem: &mut [u8], flag: u64) {
        let current = Self::read_flags(mem);
        Self::write_flags(mem, current & !flag);
    }

    /// Check if flag is set
    pub fn has_flag(mem: &[u8], flag: u64) -> bool {
        Self::read_flags(mem) & flag != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shm_layout() {
        let mut mem = vec![0u8; 1024];

        // Test length read/write
        ShmLayout::write_len(&mut mem, 42);
        assert_eq!(ShmLayout::read_len(&mem), 42);

        // Test flags
        ShmLayout::set_flag(&mut mem, ShmLayout::FLAG_REQUEST_READY);
        assert!(ShmLayout::has_flag(&mem, ShmLayout::FLAG_REQUEST_READY));
        assert!(!ShmLayout::has_flag(&mem, ShmLayout::FLAG_RESPONSE_READY));

        ShmLayout::set_flag(&mut mem, ShmLayout::FLAG_RESPONSE_READY);
        assert!(ShmLayout::has_flag(&mem, ShmLayout::FLAG_REQUEST_READY));
        assert!(ShmLayout::has_flag(&mem, ShmLayout::FLAG_RESPONSE_READY));

        ShmLayout::clear_flag(&mut mem, ShmLayout::FLAG_REQUEST_READY);
        assert!(!ShmLayout::has_flag(&mem, ShmLayout::FLAG_REQUEST_READY));
        assert!(ShmLayout::has_flag(&mem, ShmLayout::FLAG_RESPONSE_READY));
    }

    #[test]
    fn test_message_serialization() {
        let msg = ShmMessage {
            msg_id: 123,
            payload: MessagePayload::Request(WorkerRequest::Initialize {
                num_inputs: 4,
                num_outputs: 2,
                config: IpcConfig::default(),
            }),
        };

        let bytes = msg.to_bytes().unwrap();
        let decoded = ShmMessage::from_bytes(&bytes).unwrap();

        assert_eq!(msg.msg_id, decoded.msg_id);
    }
}

