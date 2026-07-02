# Memory Safety Analysis

## Overview

This document analyses the memory safety of the Rust-C FFI interface, specifically how we handle C-allocated memory from the Espresso library.

## C Memory Management

The Espresso C code uses custom allocation/deallocation:

```c
pset_family sf_new(int num, int size);   // Allocate new cover
pset_family sf_save(pset_family A);      // Clone a cover (allocates new memory)
void sf_free(pset_family A);             // Free a cover
pset_family sf_append(A, B);             // Appends B to A, FREES B
```

## Rust Wrappers

### EspressoCover

```rust
pub struct EspressoCover {
    ptr: sys::pset_family,           // Raw C pointer
    _espresso: Rc<InnerEspresso>,    // Keeps Espresso alive; also makes the type !Send + !Sync
}
```

The `Rc<InnerEspresso>` field both keeps the thread's Espresso instance alive and — because `Rc`
is neither `Send` nor `Sync` — makes `EspressoCover` `!Send + !Sync`, pinning it to its thread. No
separate `PhantomData` marker is needed.

**Memory Rules:**
- **Drop**: Calls `sf_free(self.ptr)` if ptr is not null
- **Clone**: Calls `sf_save(self.ptr)` to create independent C copy
- **into_raw()**: Transfers C ownership out of Rust (sets ptr to null before drop)

### Critical Function: minimize()

```rust
pub fn minimize(
    &self,
    f: &EspressoCover,
    d: Option<&EspressoCover>,
    r: Option<&EspressoCover>,
) -> (EspressoCover, EspressoCover, EspressoCover)
```

The inputs are **borrowed**; the cloning below happens internally so the caller's covers are left
intact. `minimize()` is the infallible variant; `try_minimize()` has the same memory flow but
returns `Result<…, MinimizationError>`, surfacing a C `fatal()` caught by the thread-local recovery
guard as `MinimizationError::EspressoFatal` instead of aborting the process (`minimize()` delegates to
it and panics on that error).

**Memory Flow:**

1. **Input F**:
   ```rust
   let f_ptr = f.clone().into_raw();
   ```
   - `clone()` allocates new C memory via `sf_save()`
   - `into_raw()` extracts pointer and prevents Rust from freeing it
   - C `espresso()` function takes ownership of `f_ptr`
   - C may free `f_ptr` and return different pointer
   - Returned pointer is wrapped in new `EspressoCover`

2. **Input D**:
   ```rust
   let d_ptr = d.as_ref()
       .map(|c| c.clone().into_raw())
       .unwrap_or_else(|| sf_new(0, size));
   ```
   - If Some: clone and extract (same as F)
   - If None: allocate empty cover with `sf_new()`
   - C `espresso()` uses but does NOT free D
   - We wrap `d_ptr` in `EspressoCover` for cleanup

3. **Input R**:
   ```rust
   let r_ptr = r.as_ref()
       .map(|c| c.clone().into_raw())
       .unwrap_or_else(|| complement(cube2list(f_ptr, d_ptr)));
   ```
   - If Some: clone and extract
   - If None: compute complement (allocates new C memory) via the guarded trampoline, so a C
     `fatal()` on a malformed cover is caught and returned as an error rather than aborting
   - C `espresso()` uses but does NOT free R
   - We wrap `r_ptr` in `EspressoCover` for cleanup

4. **Returns**:
   ```rust
   (
       EspressoCover::from_raw(f_result, espresso),  // Minimised F
       EspressoCover::from_raw(d_ptr, espresso),     // D (same pointer)
       EspressoCover::from_raw(r_ptr, espresso),     // R (same pointer)
   )
   ```
   - All wrapped in `EspressoCover`
   - When dropped, `sf_free()` is called on each

## Memory Safety Properties

### ✅ No Memory Leaks

Each C allocation is wrapped in a Rust type that calls `sf_free()` on drop:
- Created via `sf_new()` → wrapped → dropped → freed
- Created via `sf_save()` → wrapped → dropped → freed  
- Returned from C function → wrapped → dropped → freed

One deliberate exception: when the recovery guard catches a C `fatal()` mid-pipeline, the in-flight
cover pointers are intentionally leaked rather than freed. `espresso()` frees and replaces covers as
it runs, so after a `longjmp` their state is indeterminate and freeing them could double-free; leaking
them is the safe choice on this already-exceptional error path.

### ✅ No Double-Free

The `into_raw()` method sets `ptr` to null before dropping, preventing double-free:
```rust
pub(crate) fn into_raw(self) -> sys::pset_family {
    let ptr = self.ptr;
    let mut temp = self;
    temp.ptr = std::ptr::null_mut();  // Prevents sf_free() in Drop
    drop(temp);
    ptr
}
```

The original ptr is transferred to C or re-wrapped elsewhere.

### ✅ No Use-After-Free

- `EspressoCover` holds `Rc<InnerEspresso>`, keeping Espresso alive
- Espresso can't be dropped while covers exist
- Global C state (cube structure) remains valid

### ⚠️ Thread Safety

- Espresso uses thread-local storage for global state
- `EspressoCover` is `!Send + !Sync` (via its non-`Send`/`Sync` `Rc<InnerEspresso>` field)
- Covers cannot be shared between threads
- Each thread has independent C state

## Testing

See `tests/test_memory_safety.rs` for memory safety tests:
- Basic lifecycle tests
- Clone independence tests
- Repeated operations (leak amplification)
- Multiple threads (isolation)

To check for actual leaks, run with valgrind or AddressSanitizer:

```bash
# Using valgrind (Linux)
cargo build --tests
valgrind --leak-check=full --show-leak-kinds=all \
    ./target/debug/deps/test_memory_safety-*

# Using AddressSanitizer (requires nightly)
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test --tests

# Using heaptrack (Linux)
heaptrack cargo test --test test_memory_safety
```

## Potential Issues

### Issue 1: Global C State

The C code uses thread-local global variables (`cube`, `cdata`, etc.). These are cleaned up when the
last `Rc<InnerEspresso>` is dropped — `Drop` lives on `InnerEspresso` (not `Espresso`, which is just a
ref-counted handle) and delegates to the shared `teardown_cube_state()` helper:

```rust
impl Drop for InnerEspresso {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { teardown_cube_state(); }  // setdown_cube() + free/null part_size
        }
    }
}
```

This is safe because:
- Each thread has its own C globals
- `EspressoCover` holds `Rc<InnerEspresso>`
- C globals can't be freed while covers exist

## Conclusion

The memory management is correct, with these safeguards:
- All C allocations are freed
- No double-frees
- No use-after-free
- Thread-safe via thread-local storage
- Lifetime management via Rc

The key insight is that `into_raw()` transfers Rust ownership to C, and returned C pointers are re-wrapped in Rust ownership for proper cleanup.

