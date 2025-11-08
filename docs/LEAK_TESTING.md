# Memory Leak Testing Guide

This document explains how to properly test for memory leaks in the FFI C code wrapper.

## Understanding Memory Leak Testing

**Important:** Simply running operations and checking they don't panic is **NOT** a memory leak test. Real leak testing requires:

1. **External instrumentation** (valgrind, AddressSanitizer, Instruments)
2. **Memory measurement** (tracking RSS, heap size over time)
3. **Allocation tracking** (counting mallocs vs frees)

## The Problem with "Bogus" Tests

Many tests that claim to check for memory leaks are actually just functional tests:

```rust
// ❌ This is NOT a memory leak test!
#[test]
fn test_no_leak() {
    for _ in 0..100 {
        let cover = create_cover();
        minimize(cover);
        // If this doesn't panic, we assume no leak
    }
}
```

**Why this doesn't work:**
- Rust will drop the covers, but if `sf_free()` is never called, C memory leaks
- The test passes even with massive leaks because there's no measurement
- You need external tools to detect C-level leaks

## Proper Leak Testing Methods

### Method 1: Memory Measurement (Included in test_memory_safety.rs)

Measure process memory before and after many operations:

```rust
#[test]
fn test_memory_stability() {
    let baseline = get_memory_usage(); // RSS in KB
    
    // Warm up (stabilize allocators)
    for _ in 0..10 { /* operations */ }
    let baseline = get_memory_usage();
    
    // Perform many operations
    for _ in 0..1000 { /* operations */ }
    
    let after = get_memory_usage();
    let growth = after - baseline;
    
    // Assert growth is minimal (< 5MB for 1000 ops)
    assert!(growth < 5120, "Memory grew by {} KB - leak detected!", growth);
}
```

**Pros:**
- Works without external tools
- Detects significant leaks
- Fast to run

**Cons:**
- Less precise than dedicated tools
- May miss small leaks
- Can have false positives from allocator behavior

### Method 2: AddressSanitizer (ASan) - Most Reliable

ASan instruments all memory operations to detect leaks, use-after-free, double-free, etc.

**Setup (requires Rust nightly):**

```bash
# Install nightly toolchain (one-time setup)
rustup toolchain install nightly

# EASIEST: Use the provided script (handles macOS setup automatically)
./scripts/test_with_asan.sh

# MANUAL: Run tests with ASan
# On macOS, you need to set DYLD_INSERT_LIBRARIES:
export DYLD_INSERT_LIBRARIES=$HOME/.rustup/toolchains/nightly-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/lib/librustc-nightly_rt.asan.dylib
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test --test leak_detection_integration

# On Linux, just use RUSTFLAGS:
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test --test leak_detection_integration
```

**How it works:**
1. Rust nightly compiles with `-Z sanitizer=address` (instruments Rust code)
2. The `build.rs` automatically detects this and compiles C code with `-fsanitize=address`
3. Both Rust and C code are instrumented with AddressSanitizer
4. At program exit, ASan reports any memory leaks, use-after-free, or double-free issues

**macOS Limitations:**
- AddressSanitizer on macOS has known issues with Cargo's subprocess spawning
- Even with `DYLD_INSERT_LIBRARIES` set, it may not work reliably
- **Recommended for macOS**: Use Method 1 (Memory Measurement) or Method 3 (Valgrind on Linux VM)
- **For CI on macOS**: Memory measurement tests are the most reliable option

**Why this is important:**
- Without C code instrumentation, leaks in `malloc()`/`free()` from C would not be detected
- With instrumentation, **every** allocation in the C espresso library is tracked
- This catches leaks in `sf_new()`, `sf_save()`, `complement()`, etc.

**What ASan detects:**
- ✅ Memory leaks (unreachable allocations)
- ✅ Use-after-free
- ✅ Double-free
- ✅ Buffer overflows
- ✅ Stack use-after-return

**Example output on leak:**
```
=================================================================
==12345==ERROR: LeakSanitizer: detected memory leaks

Direct leak of 1024 byte(s) in 1 object(s) allocated from:
    #0 0x7f8b2c in malloc
    #1 0x4567ab in sf_new
    ...
```

### Method 3: Valgrind (Linux only)

Valgrind emulates your program and tracks every memory operation.

**Setup:**

```bash
# Install valgrind
sudo apt install valgrind

# Build test
cargo test --no-run --test leak_detection_integration

# Find test binary
TEST_BINARY=$(find target/debug/deps -name 'leak_detection_integration-*' -executable)

# Run with valgrind
valgrind \
  --leak-check=full \
  --show-leak-kinds=all \
  --track-origins=yes \
  --verbose \
  $TEST_BINARY --test-threads=1
```

**What Valgrind detects:**
- ✅ Memory leaks (definitely lost, possibly lost, still reachable)
- ✅ Invalid reads/writes
- ✅ Use of uninitialized values
- ✅ Double-free

**Example output on success:**
```
==12345== HEAP SUMMARY:
==12345==     in use at exit: 0 bytes in 0 blocks
==12345==   total heap usage: 1,234 allocs, 1,234 frees, 456,789 bytes allocated
==12345== 
==12345== All heap blocks were freed -- no leaks are possible
```

### Method 4: macOS Instruments / leaks

macOS provides built-in leak detection tools.

**Using MallocStackLogging:**

```bash
# Enable malloc tracking
export MallocStackLogging=1
export MallocStackLoggingNoCompact=1

# Run test
cargo test --test leak_detection_integration

# Check for leaks (while process is running)
leaks <PID>
```

**Using Instruments GUI:**

1. Build your test: `cargo test --no-run --test leak_detection_integration`
2. Find binary: `find target/debug/deps -name 'leak_detection_integration-*'`
3. Open Instruments.app
4. Choose "Leaks" template
5. Select the test binary and run
6. Instruments will show real-time leak detection

**Using the script:**

```bash
./scripts/check_memory_leaks.sh
```

### Method 5: Heaptrack (Linux)

Heaptrack provides detailed heap profiling with a nice GUI.

**Setup:**

```bash
# Install heaptrack
sudo apt install heaptrack heaptrack-gui

# Run test with heaptrack
cargo test --no-run --test leak_detection_integration
TEST_BINARY=$(find target/debug/deps -name 'leak_detection_integration-*' -executable)
heaptrack $TEST_BINARY --test-threads=1

# View results
heaptrack_gui heaptrack.*.gz
```

## Test File Structure

### tests/test_memory_safety.rs

Contains tests that measure memory usage and verify proper cleanup:

- `test_memory_usage_stability` - Measures RSS growth over 1000 operations
- `test_clone_independence_no_double_free` - Verifies clone creates independent memory
- `test_repeated_operations_amplify_leaks` - Amplifies small leaks through iteration
- `test_large_cover_allocations` - Tests with substantial allocations
- `test_multithreaded_memory_isolation` - Multi-threaded leak detection

**Run:** `cargo test --test test_memory_safety -- --nocapture`

### tests/leak_detection_integration.rs

Simple, focused tests designed for external leak detectors:

- `leak_test_01_basic_allocation` - Basic alloc/free
- `leak_test_02_clone_allocation` - Clone creates independent memory
- `leak_test_03_minimize` - Minimize allocates and frees correctly
- `leak_test_05_repeated_operations` - 100 iterations to amplify leaks
- `leak_test_10_stress` - 500 iterations for stress testing

**Run with ASan:**
```bash
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test --test leak_detection_integration
```

**Run with valgrind:**
```bash
./scripts/check_memory_leaks.sh
```

## Common Leak Patterns to Test

### 1. Basic Allocation Leak

**Problem:** `sf_new()` called but `sf_free()` never called

```rust
#[test]
fn test_basic() {
    let cover = EspressoCover::from_cubes(cubes, 2, 1).unwrap();
    // If Drop doesn't call sf_free(), this leaks
}
```

**Detection:** Any tool will catch this

### 2. Clone Double-Free

**Problem:** Clone doesn't allocate new memory, both instances free same pointer

```rust
#[test]
fn test_clone() {
    let cover1 = EspressoCover::from_cubes(cubes, 2, 1).unwrap();
    let cover2 = cover1.clone(); // Must call sf_save()!
    // If clone doesn't allocate, double-free occurs
}
```

**Detection:** ASan, valgrind will report double-free

### 3. into_raw() Leak

**Problem:** `into_raw()` doesn't set ptr to null, double-free in Drop

```rust
pub fn into_raw(self) -> sys::pset_family {
    let ptr = self.ptr;
    // BUG: forgot to set self.ptr = null
    drop(self); // This calls sf_free()
    ptr // Caller also frees - DOUBLE FREE!
}
```

**Detection:** ASan, valgrind

### 4. Minimize Leak

**Problem:** Minimize doesn't properly wrap returned pointers

```rust
pub fn minimize(f, d, r) -> (EspressoCover, ...) {
    let f_ptr = /* ... */;
    let result = unsafe { sys::espresso(f_ptr, d_ptr, r_ptr) };
    // BUG: return raw pointer instead of EspressoCover
    // result is never freed!
}
```

**Detection:** All tools

### 5. Repeated Operation Leak

**Problem:** Small leak that's only noticeable after many iterations

```rust
#[test]
fn test_repeated() {
    for _ in 0..1000 {
        // If each iteration leaks 1KB, total leak is 1MB
    }
}
```

**Detection:** Memory measurement, valgrind summary

## Interpreting Results

### ASan Output

**Clean run:**
```
test result: ok. 12 passed; 0 failed
```

**With leak:**
```
=================================================================
==12345==ERROR: LeakSanitizer: detected memory leaks

Direct leak of 1024 byte(s) in 1 object(s) allocated from:
    #0 0x... malloc
    #1 0x... sf_new (espresso-src/set.c:123)
    #2 0x... EspressoCover::from_cubes (src/espresso.rs:214)
```

### Valgrind Output

**Clean run:**
```
==12345== HEAP SUMMARY:
==12345==     in use at exit: 0 bytes in 0 blocks
==12345==   total heap usage: 1,234 allocs, 1,234 frees
```

**With leak:**
```
==12345== 1,024 bytes in 1 blocks are definitely lost in loss record 1 of 1
==12345==    at 0x...: malloc (vg_replace_malloc.c:299)
==12345==    by 0x...: sf_new (set.c:123)
==12345==    by 0x...: EspressoCover::from_cubes (espresso.rs:214)
```

### Memory Measurement Output

**Clean run:**
```
Memory baseline: 2048 KB
Memory after 1000 ops: 2156 KB
Memory growth: 108 KB
✓ 1000 iterations completed without memory leak
```

**With leak:**
```
Memory baseline: 2048 KB
Memory after 1000 ops: 12048 KB
Memory growth: 10000 KB
thread 'test_memory_usage_stability' panicked at:
Memory grew by 10000 KB - possible leak!
```

## Best Practices

1. **Use multiple methods:** Memory measurement for CI, ASan for thorough testing
2. **Test in isolation:** Run leak tests with `--test-threads=1` to avoid interference
3. **Amplify leaks:** Use many iterations (100-1000) to make small leaks obvious
4. **Test edge cases:** Clone, explicit drops, dimension changes, etc.
5. **Automate:** Include leak tests in CI with memory measurement
6. **Document:** Keep this guide updated with new leak patterns

## CI Integration

For CI/CD pipelines, use memory measurement tests:

```yaml
# .github/workflows/test.yml
- name: Run memory leak tests
  run: |
    cargo test --test test_memory_safety -- --nocapture --test-threads=1
```

For thorough testing (manual or scheduled):

```yaml
- name: Run with AddressSanitizer
  run: |
    rustup toolchain install nightly
    RUSTFLAGS="-Z sanitizer=address" cargo +nightly test --test leak_detection_integration
```

## Quick Reference

| Tool | Platform | Setup | Features |
|------|----------|-------|----------|
| Memory measurement | All | None | RSS-based leak detection |
| AddressSanitizer | All | Nightly Rust | Comprehensive instrumentation |
| Valgrind | Linux | `apt install` | Leaks, use-after-free, invalid reads |
| macOS leaks | macOS | None | C malloc/free leak detection |
| Heaptrack | Linux | `apt install` | Heap profiling with GUI |

## Summary

**For development:** Use `cargo test --test test_memory_safety`

**For verification:** Use `RUSTFLAGS="-Z sanitizer=address" cargo +nightly test --test leak_detection_integration`

**For CI:** Use memory measurement tests

**For deep debugging:** Use valgrind or Instruments

Remember: Tests that just run operations are **not** leak tests. You must either:
1. Measure memory usage
2. Use external leak detection tools

