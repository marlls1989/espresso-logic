# Regression Test Suite

This directory contains regression tests that validate the Rust CLI against the original C implementation.

## Test Scripts

### `quick_regression.sh`

Fast regression test with 4 key test cases:
```bash
./tests/quick_regression.sh
```

Runs in ~1 second. Good for quick validation during development.

### `comprehensive_regression.sh`

Complete regression suite with 38 test cases:
```bash
./tests/comprehensive_regression.sh
```

Tests:
- 27 example files from `examples/` directory
- Multiple output formats (-o f, fd, fr, fdr)
- 5 PLA files from `tlex/` directory

Runs in ~5 seconds.

### `regression_test.sh`

Original regression script with detailed diff output on failures.

## Test Methodology

Each test:
1. Runs the C binary (`bin/espresso`) on a test file
2. Runs the Rust binary (`target/release/espresso`) on the same file
3. Compares outputs byte-for-byte using `diff`
4. C output is considered the reference (correct)

## Current Status

✅ **38/38 tests passing**

The Rust CLI produces **identical output** to the C CLI for all tested cases.

## Running Tests

```bash
# Quick test (4 cases, ~1 second)
./tests/quick_regression.sh

# Comprehensive test (38 cases, ~5 seconds)
./tests/comprehensive_regression.sh

# Full test with detailed diff output
./tests/regression_test.sh
```

## Prerequisites

Both binaries must be built:

```bash
# Build C binary
cd espresso-src && make

# Build Rust binary
cargo build --release --bin espresso
```

## Test Files

Tests use files from:
- `examples/` - Various Boolean functions
- `tlex/` - PLA format test files

## Adding New Tests

Edit the test scripts to add more files to the `test_files` array.

## Troubleshooting

If tests fail:

1. Check that both binaries are built
2. Verify global variable initialization in `src/bin/espresso.rs`
3. Check that all Espresso options are properly exposed in `build.rs`
4. Review diff output to identify differences

## Implementation Notes

The Rust CLI must match C behavior exactly, including:
- Global variable initialization
- Algorithm parameters
- Output formatting
- Error handling

Key insight: The Espresso algorithm depends on many global variables that must be initialized to specific defaults for deterministic behavior.

