# Testing Guide

This document provides comprehensive testing documentation for the espresso-logic library.

## Overview

The test suite includes:
- **51 unit and integration tests** - Testing all API levels and functionality
- **322 documentation tests** - Ensuring all code examples in docs are correct
- **~276 regression tests** - Validating Rust CLI against C implementation
- Memory safety and leak detection tests
- Thread safety and concurrency tests
- Performance benchmarks

**Total: 373 automated tests, all passing ✅**

## Quick Start

Run all tests:

```bash
cargo test
```

Run with verbose output:

```bash
cargo test -- --nocapture
```

Run tests for a specific module:

```bash
cargo test --test test_integration
cargo test --test test_memory_safety
cargo test --test test_thread_safety
```

## Test Organization

### Unit and Integration Tests

Tests for the high-level Rust API across multiple test files:

**`tests/test_integration.rs`** - Core API functionality:
- Cover construction and manipulation
- PLA file reading and writing
- Type-safe Cover API
- Unified Cover type functionality

**`tests/test_boolean_expressions.rs`** - Boolean expression functionality:
- Boolean expression parsing and minimization
- Expression composition and evaluation
- Cover trait integration with expressions
- Complex boolean logic (XOR, majority, etc.)

**Run:**
```bash
cargo test --test test_integration
cargo test --test test_boolean_expressions
```

### Thread Safety Tests

**Location:** `tests/test_thread_safety.rs`

Verifies that thread-local storage works correctly:
- Multiple threads executing concurrently
- Independent state per thread
- No interference between threads
- Correct resource cleanup per thread

**Run:**
```bash
cargo test --test test_thread_safety
```

### Memory Safety Tests

**Location:** `tests/test_memory_safety.rs`

Comprehensive memory management verification including:
- Memory usage stability (RSS-based leak detection)
- Clone independence (no double-free)
- Ownership transfer correctness
- Repeated operations (leak amplification)
- Large allocations
- Multi-threaded memory isolation
- Espresso lifetime management

**Run:**
```bash
cargo test --test test_memory_safety -- --nocapture --test-threads=1
```

**Key tests:**
- `test_memory_usage_stability` - Measures RSS growth over 1000 operations
- `test_clone_independence_no_double_free` - Verifies independent clones
- `test_repeated_operations_amplify_leaks` - Amplifies small leaks through iteration
- `test_multithreaded_memory_isolation` - Tests 8 threads × 100 operations each

See [docs/MEMORY_SAFETY.md](docs/MEMORY_SAFETY.md) for detailed memory management analysis.

## Regression Testing

Regression tests validate that the Rust CLI produces identical output to the original C implementation.

### Quick Regression Test

Fast validation with 4 key test cases (~1 second):

```bash
./tests/quick_regression.sh
```

### Comprehensive Regression Test

Complete regression suite with ~276 test cases (~45 seconds):

```bash
./tests/regression_test.sh
```

Tests include:
- 27 basic examples from `pla/` directory with default output
- 11 files tested with all output format variations (-o f, fd, fr, fdr): 44 tests
- 41 PLA files from `tlex/` directory with default output
- 41 PLA files from `tlex/` with all output format variations: 164 tests

**Status:** ✅ All tests passing - Rust CLI produces identical output to C CLI

### Test Methodology

Each regression test:
1. Runs the C binary (`bin/espresso`) on a test file
2. Runs the Rust binary (`target/release/espresso`) on the same file
3. Compares outputs byte-for-byte using `diff`
4. C output is considered the reference (correct)

### Prerequisites

Both binaries must be built:

```bash
# Build C binary
cd espresso-src && make

# Build Rust binary
cargo build --release --bin espresso
```

## Memory Leak Detection

We use a two-tier testing strategy for comprehensive memory safety verification.

### Tier 1: Quick Smoke Tests

RSS-based tests that measure memory growth during development.

**Run:**
```bash
cargo test --test test_memory_safety -- --nocapture --test-threads=1
```

**What it tests:**
- ✅ Memory usage stability (RSS growth over 1000 operations)
- ✅ Clone independence (no double-free)
- ✅ Ownership transfer (into_raw() correctness)
- ✅ Repeated operations (leak amplification)
- ✅ Large allocations
- ✅ Multi-threaded memory isolation

**Purpose:** Quick feedback during development. Catches obvious leaks.

### Tier 2: OS-Level Leak Detection

Use native OS tools for thorough verification with external leak detectors.

#### Using the Automated Script

```bash
# Cross-platform (auto-detects OS)
./scripts/check_memory_leaks.sh

# macOS only: leaks command
./scripts/check_leaks_macos.sh

# Validate that leak detection works
./scripts/check_leaks_macos.sh --validate  # macOS
```

#### Platform-Specific Tools

**macOS:** Uses built-in `leaks` command
```bash
export MallocStackLogging=1
cargo run --example leak_check
leaks espresso
```

- ✅ Detects C malloc/free leaks
- ✅ No installation required
- ✅ Validated with intentional_leak example

**Linux:** Uses valgrind
```bash
cargo test --no-run --test test_memory_safety
TEST_BINARY=$(find target/debug/deps -name 'test_memory_safety-*' -executable)
valgrind --leak-check=full --show-leak-kinds=all $TEST_BINARY --test-threads=1
```

- ✅ Detects both Rust and C leaks
- ✅ Detects use-after-free, invalid reads
- Installation: `sudo apt install valgrind`

#### Leak Detection Examples

Located in `examples/`:
- `leak_check.rs` - Leak check (10,000 iterations)
- `intentional_leak.rs` - ⚠️ INTENTIONALLY LEAKS (validates methodology)

### Tool Comparison

| Tool | Platform | Detects C Leaks | Notes |
|------|----------|-----------------|-------|
| RSS tests | All | ✅ | Quick smoke test |
| macOS leaks | macOS | ✅ | Validated ✅ |
| valgrind | Linux | ✅ | Detects leaks, use-after-free, invalid reads |

### Testing Workflow

1. **During development**: Run `cargo test` (Tier 1 - seconds)
2. **Before commit**: Run `./scripts/check_memory_leaks.sh` (Tier 2 - minutes)
3. **Validate methodology**: Run leak detection validation script

Both tiers are complementary:
- **Tier 1**: Fast feedback during development
- **Tier 2**: Definitive verification with OS tools

## Key Testing Principles

The test suite verifies memory safety through:
1. **Memory measurement** - RSS-based leak detection over many iterations
2. **External tools** - OS-level leak detection (valgrind, leaks command)
3. **Stress testing** - Large allocations and repeated operations to amplify leaks

## Test File Reference

### `tests/test_integration.rs`

Integration tests for the high-level Rust API (Cover, PLA reading/writing).

### `tests/test_boolean_expressions.rs`

Comprehensive tests for boolean expression parsing, evaluation, minimization, and composition (BoolExpr, Cover trait integration).

### `tests/test_thread_safety.rs`

Tests that verify thread-local storage works correctly and threads don't interfere.

### `tests/test_memory_safety.rs`

Contains tests that measure memory usage and verify proper cleanup:

- `test_memory_usage_stability` - Measures RSS growth over 1000 operations
- `test_clone_independence_no_double_free` - Verifies clone creates independent memory
- `test_repeated_operations_amplify_leaks` - Amplifies small leaks through iteration
- `test_large_cover_allocations` - Tests with substantial allocations
- `test_multithreaded_memory_isolation` - Multi-threaded leak detection

### Regression Test Scripts

- `tests/quick_regression.sh` - Quick regression test (4 cases, ~1 second)
- `tests/regression_test.sh` - Comprehensive regression test (~276 cases, ~45 seconds) with detailed diff output

## Benchmarks

Performance benchmarks using Criterion:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench pla_benchmarks
```

**Location:** `benches/`

## Continuous Integration

For CI/CD pipelines, use memory measurement tests:

```yaml
# .github/workflows/test.yml
- name: Run tests
  run: cargo test

- name: Run memory leak tests
  run: |
    cargo test --test test_memory_safety -- --nocapture --test-threads=1

- name: Run regression tests
  run: |
    cd espresso-src && make
    cargo build --release --bin espresso
    ./tests/regression_test.sh
```

## Troubleshooting

### Regression Tests Failing

If regression tests fail:

1. Check that both binaries are built
2. Verify global variable initialization in `src/bin/espresso.rs`
3. Check that all Espresso options are properly exposed in `build.rs`
4. Review diff output to identify differences

### Memory Tests Failing

If memory tests report leaks:

1. Run with `--nocapture` to see memory measurements
2. Use external tools (valgrind, leaks) for verification
3. Check for missing `Drop` implementations
4. Verify `into_raw()` properly nulls pointers

### Thread Safety Tests Failing

If thread safety tests fail:

1. Verify thread-local storage is properly initialized
2. Check that C code uses `_Thread_local` for globals
3. Ensure `setdown_cube()` is called on thread exit
4. Review [docs/THREAD_LOCAL_IMPLEMENTATION.md](docs/THREAD_LOCAL_IMPLEMENTATION.md)

## Additional Resources

- [docs/MEMORY_SAFETY.md](docs/MEMORY_SAFETY.md) - Detailed memory management analysis
- [docs/LEAK_TESTING.md](docs/LEAK_TESTING.md) - Complete guide to leak testing techniques
- [docs/THREAD_LOCAL_IMPLEMENTATION.md](docs/THREAD_LOCAL_IMPLEMENTATION.md) - Thread-local storage implementation details

## Best Practices

1. **Run tests frequently** - Use `cargo test` during development
2. **Test in isolation** - Run leak tests with `--test-threads=1` to avoid interference
3. **Amplify leaks** - Use many iterations (100-1000) to make small leaks obvious
4. **Test edge cases** - Clone, explicit drops, dimension changes, etc.
5. **Automate** - Include tests in CI with memory measurement
6. **Validate** - Periodically run OS-level leak detection tools

## Summary

**For quick validation:**
```bash
cargo test
```

**For comprehensive testing:**
```bash
cargo test --test test_memory_safety -- --nocapture --test-threads=1
./tests/regression_test.sh
./scripts/check_memory_leaks.sh
```

**For development:**
- Unit tests run automatically on every build
- Memory tests provide quick feedback
- Regression tests validate CLI compatibility

**For release:**
- All tests must pass
- Memory leak detection must show no leaks
- Regression tests must match C implementation exactly

