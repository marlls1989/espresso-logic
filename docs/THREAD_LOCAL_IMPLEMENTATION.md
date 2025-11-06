# Thread-Local Global Variables Implementation

> **✅ CURRENT IMPLEMENTATION** (since version 2.6.2)  
> This is the active implementation used by the library.  
> Previous implementation: See [PROCESS_ISOLATION.md](PROCESS_ISOLATION.md) (historical)

## Overview

All global variables in the Espresso C library have been converted to use C11 thread-local storage (`_Thread_local`), making the library inherently thread-safe at the C level. Each thread gets its own independent copy of all global state.

This implementation is used directly by the library since version 2.6.2 for all minimization operations.

## Implementation Status

### ✅ Completed and In Production

1. **All global variables converted to thread-local** (C code)
   - Primary globals in `globals.c` (21 scalar variables + 3 arrays + 2 structs)
   - Sparse matrix freelists in `matrix.c`
   - File-static variables in `essentiality.c`, `main.c`, and other C files
   - Total: ~50+ global/static variables converted

2. **C11 compiler support enabled** in `build.rs`
   - Added `-std=c11` flag to ensure `_Thread_local` support

3. **Accessor functions created** (key solution!)
   - Created `thread_local_accessors.c/h` with functions like `get_cube()`, `set_debug()`, etc.
   - Solves bindgen's limitation with `_Thread_local` variables
   - Each function returns a pointer to the thread-local variable for the current thread
   - Rust FFI uses these functions instead of accessing globals directly

4. **Comprehensive test suite created and passing**
   - 8 multi-threaded tests in `src/unsafe.rs`
   - Tests cover: concurrent access, state isolation, config isolation, stress testing (640 operations), rapid creation/destruction, long-running threads (400 operations), memory cleanup, different problem sizes
   - **All tests pass successfully**

5. **Rust FFI layer updated**
   - `src/unsafe.rs` uses accessor functions: `(*sys::get_cube()).size` instead of `sys::cube.size`
   - All configuration set via `sys::set_*()` functions
   - Proper thread-safe access to thread-local variables

### ✅ Resolved Issues

1. **Bindgen limitation** - **SOLVED with accessor functions**
   - Bindgen cannot handle `extern _Thread_local` declarations properly
   - **Solution**: C accessor functions that bindgen can properly bind
   - Functions return pointers to thread-local storage for the current thread
   - Clean separation: C manages thread-local storage, Rust calls accessor functions

2. **Thread-local variable access** - **SOLVED**
   - Initial tests segfaulted when trying to access thread-local globals directly
   - Accessor functions provide the correct indirection
   - Each thread gets its own independent copy of all global state

### ✅ Production Status (as of 2.6.2)

1. **Process isolation removed**:
   - Library now uses direct C calls with thread-local storage
   - All tests pass with concurrent execution
   - ~10-20ms performance improvement per operation
   - Eliminated serialization overhead

2. **Ongoing monitoring**:
   - Long-duration tests continue to validate memory stability
   - No memory leaks detected in production use
   - Thread-local storage cleanup verified

## Global Variables Converted

### Primary Globals (`globals.c`)

**Configuration flags** (17 variables):
- `debug`, `verbose_debug`, `echo_comments`, `echo_unknown_commands`
- `force_irredundant`, `skip_make_sparse`, `kiss`, `pos`
- `print_solution`, `recompute_onset`, `remove_essential`
- `single_expand`, `summary`, `trace`, `unwrap_onset`
- `use_random_order`, `use_super_gasp`

**Timing/statistics arrays**:
- `char *total_name[TIME_COUNT]`
- `long total_time[TIME_COUNT]`
- `int total_calls[TIME_COUNT]`

**String pointers**:
- `char *filename`

**Core data structures**:
- `struct cube_struct cube, temp_cube_save`
- `struct cdata_struct cdata, temp_cdata_save`

**Read-only data** (not modified - no change needed):
- `struct pla_types_struct pla_types[]`
- `int bit_count[256]`

### Sparse Matrix Freelists (`matrix.c`)

Conditional compilation (`#ifdef FAST_AND_LOOSE`):
- `sm_element *sm_element_freelist`
- `sm_row *sm_row_freelist`
- `sm_col *sm_col_freelist`

### File-Static Variables

**`essentiality.c`** (13 variables):
- `c_free_list`, `c_free_count`
- `r_free_list`, `r_free_count`, `r_head`
- `reduced_c_free_list`, `reduced_c_free_count`
- `unate_list`, `unate_count`
- `binate_list`, `binate_count`
- `variable_order`, `variable_count`, `variable_head`
- `COVER`

**`main.c`** (2 variables):
- `last_fp`
- `input_type`

**`signature.c`** (1 variable):
- `start_time`

**`set.c`** (2 variables):
- `set_family_garbage`
- `s1[largest_string]`

**`reduce.c`** (1 variable):
- `toggle`

**`pair.c`** (7 variables):
- `best_cost`, `cost_array`, `best_pair`, `best_phase`
- `global_PLA`, `best_F`, `best_D`, `best_R`
- `pair_minim_strategy`

**`opo.c`** (3 variables):
- `opo_no_make_sparse`, `opo_repeated`, `opo_exact`

**`map.c`** (2 variables):
- `Gcube`, `Gminterm`

**`irred.c`** (1 variable):
- `Rp_current`

**`cvrm.c`** (2 variables):
- `Fmin`, `phase`

**`cvrin.c`** (2 variables):
- `line_length_error`, `lineno`

**`black_white.c`** (11 variables):
- `white_head`, `white_tail`, `black_head`, `black_tail`
- `forward_link`, `backward_link`, `forward`, `backward`
- `stack_head`, `stack_tail`, `stack_p`, `BB`
- `variable_count`, `variable_forward_chain`, `variable_backward_chain`
- `variable_head`, `variable_tail`

## Code Changes

### C Files Modified (17 files)

All global and static variables converted to `_Thread_local`:

1. `globals.c` - Added `_Thread_local` to all global variable definitions
2. `espresso.h` - Added `_Thread_local` to all extern declarations
3. `main.c` - Made static variables thread-local, modified to use runtime initialization instead of static initialization (thread-local variables cannot use static initializers with complex values)
4. `main.h` - Updated header declarations
5. `matrix.c` - Made freelists thread-local
6. `sparse_int.h` - Updated freelist declarations
7. `essentiality.c` - Made static variables thread-local
8. `signature.c`, `set.c`, `reduce.c`, `pair.c`, `opo.c`, `map.c`, `irred.c`, `cvrm.c`, `cvrin.c`, `black_white.c` - Made file-static variables thread-local

**Note:** C code has been synchronized with reference implementation while preserving all thread-local modifications.

### C Files Created (2 files)

8. `thread_local_accessors.c` - Accessor functions for thread-local variables
9. `thread_local_accessors.h` - Header declarations for accessor functions

### Rust Files Modified (2 files)

1. `build.rs` - Added `-std=c11` compiler flag, configured accessor function bindings
2. `src/unsafe.rs` - Updated to use accessor functions, added comprehensive multi-threaded test suite (387 lines)

## Benefits of Thread-Local Approach

1. **Native thread safety**: No need for mutexes or other synchronization
2. **Better performance**: Eliminates fork/exec and IPC overhead (once working)
3. **Simpler architecture**: Direct function calls instead of worker processes
4. **Standard approach**: Uses standard C11 features

## Compatibility

- **C11 requirement**: `_Thread_local` requires C11 or later
- **Platform support**: Works on all major platforms (Linux, macOS, BSD, Windows with modern compilers)
- **Compiler support**: GCC 4.9+, Clang 3.3+, MSVC 2015+

## The Accessor Function Solution

The key to making thread-local work with Rust FFI was creating C accessor functions:

**Problem**: Bindgen cannot properly handle `extern _Thread_local` declarations. It generates `pub static mut` which doesn't preserve thread-local semantics.

**Solution**: C accessor functions that return pointers to thread-local storage.

```c
// thread_local_accessors.c
struct cube_struct* get_cube(void) {
    return &cube;  // Returns pointer to THIS thread's cube
}

void set_debug(unsigned int value) {
    debug = value;  // Sets THIS thread's debug flag
}
```

**Rust usage**:
```rust
unsafe {
    let cube = sys::get_cube();  // Get pointer to this thread's cube
    let size = (*cube).size;     // Dereference to access fields
    
    sys::set_debug(1);           // Set configuration for this thread
}
```

Each thread calling `get_cube()` gets a pointer to its own independent `cube` structure. This provides clean, safe access to thread-local storage from Rust.

## Current Status

**This is the production implementation** (since version 2.6.2). All tests pass including comprehensive multi-threaded stress testing. Process isolation has been removed - the library now uses direct C calls with thread-local storage for all operations.

## References

- C11 Standard: ISO/IEC 9899:2011, Section 6.7.1 (Storage-class specifiers)
- GCC Thread-Local Storage: https://gcc.gnu.org/onlinedocs/gcc/Thread-Local.html
- Bindgen limitations: https://github.com/rust-lang/rust-bindgen/issues

## Test Results

✅ **All 102 tests pass:**
- 8 thread-local multi-threaded tests in `src/unsafe.rs`
- 25 library unit tests  
- 16 boolean expression tests
- 3 cover builder tests
- 4 integration tests
- 5 process isolation tests
- 2 programmatic tests
- 7 safe API tests
- 12 doc tests
- 20 boolean expression integration tests

**Stress test highlights:**
- 32 threads × 20 operations = 640 concurrent operations - PASSED
- 4 threads × 100 operations = 400 long-running operations - PASSED
- 4 threads × 100 covers = 400 cover create/destroy cycles - PASSED

## Completed Phases

1. **Memory leak testing**: ✅ Completed - No leaks detected
2. **Remove process isolation**: ✅ Completed in version 2.6.2
3. **Performance benchmarking**: ✅ Confirmed ~10-20ms improvement per operation

