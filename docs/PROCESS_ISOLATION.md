# Process Isolation Architecture

> **⚠️ HISTORICAL DOCUMENT**  
> This document describes the **previous implementation** used before version 2.6.2.  
> **Current implementation** (since 2.6.2): See [THREAD_LOCAL_IMPLEMENTATION.md](THREAD_LOCAL_IMPLEMENTATION.md)  
> The library now uses C11 thread-local storage instead of process isolation for better performance.

## Overview

This document describes how the Espresso logic minimizer previously achieved **transparent thread-safety** through process isolation (versions 2.6.0-2.6.1). The global state problem inherent in the C library was hidden from users through process isolation.

## The Problem

The original Espresso C library uses extensive global state:
- Global `cube` structure for problem dimensions
- Global configuration variables
- Static buffers and caches

This makes concurrent execution **unsafe** - multiple threads cannot safely use Espresso simultaneously without explicit synchronization (mutexes), which serializes execution and defeats the purpose of parallelization.

## The Solution

The **transparent process isolation architecture**:

1. **Automatically isolates** each operation in a separate forked process
2. **Uses efficient shared memory** for inter-process communication
3. **Provides thread-safe API** - no special types or synchronization needed
4. **Automatically manages** worker process lifecycle
5. **Hides complexity** - users never see the implementation details

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        User Code                             │
│                                                               │
│  ┌─────────────────────┐        ┌──────────────────────┐   │
│  │   Espresso API      │◄─────► │   (thread-safe!)     │   │
│  └──────────┬──────────┘        └──────────────────────┘   │
│             │ minimize()                                     │
└─────────────┼────────────────────────────────────────────────┘
              │
              │ (transparent fork + IPC)
              │
    ┌─────────┼─────────────────────────────────┐
    │         ▼                                  │
    │  ┌──────────────┐        Isolated         │
    │  │    Worker    │◄───── Process ──────►   │
    │  │  (UnsafeESP) │       (safe here!)      │
    │  └──────────────┘                         │
    │   Shared Memory                            │
    └────────────────────────────────────────────┘
```

### Key Components

**Public API:**
1. **`Espresso`** (`src/lib.rs`)
   - Main user-facing API
   - Thread-safe by default
   - Transparently uses process isolation
   - Cloneable and shareable across threads

2. **`CoverBuilder`** (`src/lib.rs`)
   - Builds covers programmatically
   - Handles initialization internally
   - No manual setup required

**Internal Implementation (hidden from users):**

3. **`UnsafeEspresso`** (`src/unsafe_espresso.rs`)
   - Direct wrapper around C API with global state
   - Only used by isolated worker processes
   - Never exposed to public API

4. **`worker`** module (`src/worker.rs`)
   - Worker process main loop
   - Runs in forked child process
   - Processes requests via shared memory

5. **`process_pool`** (`src/process_pool.rs`)
   - Manages worker lifecycle
   - Uses `fork()` to create isolated processes
   - Handles shared memory allocation

6. **IPC Protocol** (`src/ipc.rs`)
   - Serializable message format (`serde` + `bincode`)
   - Shared memory layout with status flags
   - Request/response pattern

7. **Conversion Layer** (`src/conversion.rs`)
   - Serializes/deserializes `Cover` objects
   - Preserves binary cube representation

## Usage

### Basic Usage

```rust
use espresso_logic::{Espresso, CoverBuilder};

// Just use Espresso - thread-safe by default!
let esp = Espresso::new(2, 1);

// Create a cover (no manual initialization needed)
let mut builder = CoverBuilder::new(2, 1);
builder.add_cube(&[0, 1], &[1]);
builder.add_cube(&[1, 0], &[1]);
let f = builder.build();

// Minimize (returns Result, automatically uses process isolation)
let result = esp.minimize(f, None, None)?;
```

### Concurrent Usage

```rust
use espresso_logic::{Espresso, CoverBuilder};
use std::sync::Arc;
use std::thread;

// Create Espresso - automatically thread-safe!
let esp = Arc::new(Espresso::new(2, 1));

// Spawn multiple threads - just works!
let handles: Vec<_> = (0..4).map(|_| {
    let esp_clone = Arc::clone(&esp);
    thread::spawn(move || {
        let mut builder = CoverBuilder::new(2, 1);
        builder.add_cube(&[0, 1], &[1]);
        builder.add_cube(&[1, 0], &[1]);
        let f = builder.build();
        
        // Each call runs in an isolated process automatically
        esp_clone.minimize(f, None, None)
    })
}).collect();

// Wait for all threads
for handle in handles {
    let result = handle.join().unwrap()?;
    println!("Result: {:?}", result);
}
```

## Technical Details

### Shared Memory IPC

Communication uses POSIX shared memory (`shm_open`/`mmap`):

**Memory Layout:**
```
[0..8]:    Message length (u64)
[8..16]:   Status flags (u64)
           - bit 0: REQUEST_READY
           - bit 1: RESPONSE_READY
           - bit 2: WORKER_ERROR
[16..MAX]: Message data (bincode-serialized)
```

**Protocol:**
1. Parent writes request to shared memory
2. Parent sets `REQUEST_READY` flag
3. Worker processes request
4. Worker writes response to shared memory
5. Worker sets `RESPONSE_READY` flag
6. Parent reads response

### Process Forking

We use `fork()` instead of spawning separate binaries:

**Advantages:**
- No binary path resolution issues
- Faster startup (no exec overhead)
- Simpler deployment (one binary)
- Code sharing between parent and child

**Implementation:**
```rust
match unsafe { fork() } {
    Ok(ForkResult::Parent { child }) => {
        // Parent: store child PID, continue
    }
    Ok(ForkResult::Child) => {
        // Child: run worker_main(), never return
        crate::worker::worker_main(&shm_name);
    }
    Err(e) => {
        // Handle error
    }
}
```

### Thread Safety Considerations

**Safe:**
- ✓ Multiple threads calling `ProcessIsolatedEspresso::minimize()`
- ✓ Cloning `ProcessIsolatedEspresso` across threads
- ✓ Concurrent operations on different instances

**Note:**
- Each operation spawns a fresh worker process
- Workers are isolated - no shared state
- Parent process coordination is minimal

## Performance Characteristics

### Overhead

**Process Creation:**
- ~10-20ms per worker spawn (fork + initialization)
- Amortized across operation duration

**IPC:**
- Shared memory is very fast (~microseconds)
- Serialization overhead depends on cover size

**Memory:**
- Each worker: ~5-10 MB (Espresso + cube structure)
- Shared memory: 16 MB per worker

### When Process Isolation Matters

**Benefits:**
- ✓ Thread-safety without manual synchronization
- ✓ No global state conflicts
- ✓ Simple, safe API
- ✓ Works for both single-threaded and concurrent code

**Trade-offs:**
- ~10-20ms overhead per operation for process spawning
- Worth it for safety and simplicity in most cases
- For microsecond-level performance needs, consider batching operations

## Alternative: Thread-Local Implementation

### Thread-Local Global Variables (Available)

An alternative thread-safety approach is now available using C11 `_Thread_local` storage:

**Status**: ✅ Implemented and fully working
- All ~50+ global variables converted to `_Thread_local`
- C accessor functions provide clean Rust FFI (`get_cube()`, `set_debug()`, etc.)
- All 102 tests pass including 8 new multi-threaded stress tests
- 640 concurrent operations validated in stress testing

**How it works**:
- Each thread gets its own copy of all global state
- C11 `_Thread_local` provides native thread safety
- Accessor functions solve bindgen's limitation with thread-local variables
- Direct function calls - no process spawning or IPC overhead

**Trade-offs vs Process Isolation**:

| Aspect | Process Isolation | Thread-Local |
|--------|------------------|--------------|
| Overhead | ~10-20ms per operation | Microseconds |
| Memory | 5-10 MB per worker | ~Few KB per thread |
| Thread safety | Guaranteed by OS | Guaranteed by C11 TLS |
| Setup complexity | Process spawn + IPC | Direct calls |
| Platform support | Unix only | All platforms (C11) |
| Validation status | Production-proven | Tested, ready for use |

**When to use thread-local**:
- ✓ High-frequency operations (thousands per second)
- ✓ Memory-constrained environments  
- ✓ Windows platform support needed
- ✓ Lower latency requirements

**When to use process isolation**:
- ✓ Maximum isolation and safety
- ✓ Paranoid about memory corruption
- ✓ Legacy/proven approach preferred

See `docs/THREAD_LOCAL_IMPLEMENTATION.md` for complete details.

## Limitations and Future Work

### Current Limitations (Process Isolation)

1. **Fork from multi-threaded processes**
   - Forking from a multi-threaded process can be problematic
   - Only the calling thread is duplicated in the child
   - This is a general Unix limitation

2. **No process pooling yet**
   - Each operation spawns a new worker
   - Future: maintain a pool of warm workers

3. **Fixed shared memory size**
   - 16 MB limit per message
   - Very large covers may exceed this

### Future Improvements

- [ ] Make thread-local the default (after memory leak validation)
- [ ] Keep process isolation as optional feature
- [ ] Implement worker process pooling for reuse (if keeping process isolation)
- [ ] Dynamic shared memory sizing
- [ ] Performance tuning and benchmarks
- [ ] Support for platforms without `fork()` (Windows) - now possible with thread-local
- [ ] Async/await API

## Examples

See:
- `examples/simple_isolated_test.rs` - Basic usage
- `examples/concurrent_minimization.rs` - Concurrent execution demo
- `tests/test_process_isolation.rs` - Test suite

## Dependencies

- **nix**: Unix system calls (`fork`, `shm_open`, etc.)
- **serde**: Serialization framework
- **bincode**: Binary serialization format
- **memmap2**: Memory-mapped file handling

## Platform Support

- ✓ Linux
- ✓ macOS
- ✓ BSD variants
- ✗ Windows (requires different approach without `fork`)

## References

- Espresso Algorithm: R. K. Brayton et al., "Logic Minimization Algorithms for VLSI Synthesis"
- POSIX Shared Memory: IEEE Std 1003.1
- Process Forking: Stevens & Rago, "Advanced Programming in the UNIX Environment"

