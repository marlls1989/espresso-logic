//! Process isolation using self-exec worker pattern
//!
//! This module provides clean process execution where:
//! - Each operation spawns a fresh worker process (same binary)
//! - Worker mode is detected via ctor before main()
//! - Data passed via stdin/stdout (no shared state issues!)
//! - No fork() = no threading deadlocks!
//!
//! ## How It Works
//!
//! 1. Parent: Serialize request → spawn worker with "__ESPRESSO_WORKER__" arg
//! 2. Worker: ctor detects arg → enters worker_loop → reads stdin → processes → writes stdout → exits
//! 3. Parent: Reads response from worker's stdout
//!
//! ## Why This Works
//!
//! - No fork() = no inherited locks
//! - Fresh process = clean global state
//! - Works with any binary that uses the library
//! - Thread-safe by design

use crate::ipc::SerializedCover;
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};

/// Request types for worker processes
#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum WorkerRequest {
    Minimize {
        num_inputs: usize,
        num_outputs: usize,
        config: crate::ipc::IpcConfig,
        f_cubes: Vec<(Vec<u8>, Vec<u8>)>,
        d_cubes: Option<Vec<(Vec<u8>, Vec<u8>)>>,
        r_cubes: Option<Vec<(Vec<u8>, Vec<u8>)>>,
    },
}

/// Response types from worker processes
#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum WorkerResponse {
    MinimizeResult {
        f_cover: SerializedCover,
        d_cover: Option<SerializedCover>,
        r_cover: Option<SerializedCover>,
    },
    Error(String),
}

/// Worker utilities for process-isolated execution
pub struct Worker;

impl Worker {
    /// Execute a minimize operation in an isolated worker process
    /// Returns (F cover, optional D cover, optional R cover)
    pub fn execute_minimize(
        num_inputs: usize,
        num_outputs: usize,
        config: crate::ipc::IpcConfig,
        f_cubes: Vec<(Vec<u8>, Vec<u8>)>,
        d_cubes: Option<Vec<(Vec<u8>, Vec<u8>)>>,
        r_cubes: Option<Vec<(Vec<u8>, Vec<u8>)>>,
    ) -> io::Result<(SerializedCover, Option<SerializedCover>, Option<SerializedCover>)> {
        let request = WorkerRequest::Minimize {
            num_inputs,
            num_outputs,
            config,
            f_cubes,
            d_cubes,
            r_cubes,
        };

        let response = Self::spawn_worker_and_execute(request)?;

        match response {
            WorkerResponse::MinimizeResult { f_cover, d_cover, r_cover } => Ok((f_cover, d_cover, r_cover)),
            WorkerResponse::Error(e) => Err(io::Error::other(e)),
        }
    }

    /// Spawn a worker process and execute a request
    fn spawn_worker_and_execute(request: WorkerRequest) -> io::Result<WorkerResponse> {
        // Get current executable path
        let current_exe = std::env::current_exe().map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to get current exe: {}", e),
            )
        })?;

        // Spawn worker process with special argument
        let mut child = Command::new(current_exe)
            .arg("__ESPRESSO_WORKER__")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        // Serialize request
        let request_bytes = bincode::serialize(&request)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Send request length + data
        let mut stdin = child.stdin.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "Failed to get child stdin")
        })?;
        let len_bytes = (request_bytes.len() as u64).to_le_bytes();
        stdin.write_all(&len_bytes)?;
        stdin.write_all(&request_bytes)?;
        drop(stdin); // Close stdin to signal end of request

        // Read response
        let mut stdout = child.stdout.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "Failed to get child stdout")
        })?;
        let mut len_buf = [0u8; 8];
        stdout.read_exact(&mut len_buf)?;
        let response_len = u64::from_le_bytes(len_buf) as usize;

        if response_len > 100 * 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Response too large",
            ));
        }

        let mut response_bytes = vec![0u8; response_len];
        stdout.read_exact(&mut response_bytes)?;

        // Wait for child to exit
        let status = child.wait()?;
        if !status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Worker exited with status: {}", status),
            ));
        }

        // Deserialize response
        bincode::deserialize(&response_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

/// Worker loop - runs when process is spawned as a worker
/// This is called by the ctor before main()
pub(crate) fn run_worker_loop() {
    let result = worker_main();

    // Exit with appropriate code
    std::process::exit(match result {
        Ok(()) => 0,
        Err(_) => 1,
    });
}

/// Main worker logic
fn worker_main() -> io::Result<()> {
    // Read request from stdin
    let mut stdin = io::stdin();
    let mut len_buf = [0u8; 8];
    stdin.read_exact(&mut len_buf)?;
    let request_len = u64::from_le_bytes(len_buf) as usize;

    if request_len > 100 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Request too large",
        ));
    }

    let mut request_bytes = vec![0u8; request_len];
    stdin.read_exact(&mut request_bytes)?;

    // Deserialize request
    let request: WorkerRequest = bincode::deserialize(&request_bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Process request
    let response = match request {
        WorkerRequest::Minimize {
            num_inputs,
            num_outputs,
            config,
            f_cubes,
            d_cubes,
            r_cubes,
        } => {
            use crate::{unsafe_espresso::UnsafeEspresso, UnsafeCover};

            // CRITICAL: Redirect C code's stdout to stderr to prevent corrupting IPC
            // The C code prints debug/trace/verbose output to stdout, but we use stdout for IPC.
            // We save the original stdout fd, redirect stdout to stderr, run the C code,
            // then restore stdout for sending the IPC response.
            let saved_stdout_fd = unsafe {
                // Duplicate stdout (fd 1) to a new fd
                let saved_fd = libc::dup(1);
                if saved_fd == -1 {
                    return Err(io::Error::last_os_error());
                }

                // Redirect stdout (fd 1) to stderr (fd 2)
                // This makes all C printf() calls go to stderr instead of stdout
                if libc::dup2(2, 1) == -1 {
                    libc::close(saved_fd);
                    return Err(io::Error::last_os_error());
                }

                saved_fd
            };

            // Initialize Espresso with provided config
            let mut esp = UnsafeEspresso::new_with_config(num_inputs, num_outputs, config);

            // Build covers from cube data
            let f_cover = UnsafeCover::build_from_cubes(f_cubes, num_inputs, num_outputs);
            let d_cover =
                d_cubes.map(|cubes| UnsafeCover::build_from_cubes(cubes, num_inputs, num_outputs));
            let r_cover =
                r_cubes.map(|cubes| UnsafeCover::build_from_cubes(cubes, num_inputs, num_outputs));

            // Minimize (C code output goes to stderr now)
            let (f_result, d_result, r_result) = esp.minimize(f_cover, d_cover, r_cover);

            // Restore original stdout for IPC
            unsafe {
                if libc::dup2(saved_stdout_fd, 1) == -1 {
                    libc::close(saved_stdout_fd);
                    return Err(io::Error::last_os_error());
                }
                libc::close(saved_stdout_fd);
            }

            // Serialize F, D, and R results
            // D and R are always computed/processed by espresso
            WorkerResponse::MinimizeResult {
                f_cover: f_result.serialize(),
                d_cover: Some(d_result.serialize()),
                r_cover: Some(r_result.serialize()),
            }
        }
    };

    // Serialize response
    let response_bytes =
        bincode::serialize(&response).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Write response length + data to stdout
    let mut stdout = io::stdout();
    let len_bytes = (response_bytes.len() as u64).to_le_bytes();
    stdout.write_all(&len_bytes)?;
    stdout.write_all(&response_bytes)?;
    stdout.flush()?;

    Ok(())
}
