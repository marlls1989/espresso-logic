# Memory Safety Analysis

## Overview

This document analyzes the memory safety of the Rust-C FFI interface, specifically how we handle C-allocated memory from the Espresso library.

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
    _espresso: Rc<Espresso>,         // Keeps Espresso alive
    _marker: PhantomData<*mut ()>,   // !Send + !Sync marker
}
```

**Memory Rules:**
- **Drop**: Calls `sf_free(self.ptr)` if ptr is not null
- **Clone**: Calls `sf_save(self.ptr)` to create independent C copy
- **into_raw()**: Transfers C ownership out of Rust (sets ptr to null before drop)

### Critical Function: minimize()

```rust
pub fn minimize(
    self: &Rc<Self>,
    f: EspressoCover,
    d: Option<EspressoCover>,
    r: Option<EspressoCover>,
) -> (EspressoCover, EspressoCover, EspressoCover)
```

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
   - If None: compute complement (allocates new C memory)
   - C `espresso()` uses but does NOT free R
   - We wrap `r_ptr` in `EspressoCover` for cleanup

4. **Returns**:
   ```rust
   (
       EspressoCover::from_raw(f_result, self),  // Minimized F
       EspressoCover::from_raw(d_ptr, self),     // D (same pointer)
       EspressoCover::from_raw(r_ptr, self),     // R (same pointer)
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

- `EspressoCover` holds `Rc<Espresso>`, keeping Espresso alive
- Espresso can't be dropped while covers exist
- Global C state (cube structure) remains valid

### ⚠️ Thread Safety

- Espresso uses thread-local storage for global state
- `EspressoCover` is `!Send + !Sync` (marked with `PhantomData<*mut ()>`)
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

### Issue 1: Espresso::cleanup_if_unused()

The `cleanup_if_unused()` function clears the weak reference but relies on all Rc refs being dropped:

```rust
pub fn cleanup_if_unused() {
    ESPRESSO_INSTANCE.with(|instance| {
        if instance.borrow().upgrade().is_none() {
            *instance.borrow_mut() = std::rc::Weak::new();
        }
    });
}
```

This is safe because:
- Covers hold `Rc<Espresso>`
- Cleanup only occurs when no covers exist
- If any cover exists, the Rc keeps Espresso alive

### Issue 2: Global C State

The C code uses thread-local global variables (`cube`, `cdata`, etc.). These are cleaned up in `Espresso::drop()`:

```rust
impl Drop for Espresso {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                sys::setdown_cube();  // Frees cube.*, cdata.*
                // ... free part_size ...
            }
        }
    }
}
```

This is safe because:
- Each thread has its own C globals
- `EspressoCover` holds `Rc<Espresso>`
- C globals can't be freed while covers exist

## Conclusion

The memory management is **correct** with proper safeguards:
- ✅ All C allocations are freed
- ✅ No double-frees
- ✅ No use-after-free
- ✅ Thread-safe via thread-local storage
- ✅ Proper lifetime management via Rc

The key insight is that `into_raw()` transfers Rust ownership to C, and returned C pointers are re-wrapped in Rust ownership for proper cleanup.

