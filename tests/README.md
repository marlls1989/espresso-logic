# Test Suite

This directory contains the test suite for the espresso-logic library.

## Test Files

| File | Description |
|------|-------------|
| `test_integration.rs` | Integration tests for high-level APIs (BoolExpr, CoverBuilder, PLACover) |
| `test_thread_safety.rs` | Thread-local storage and concurrency tests |
| `test_memory_safety.rs` | Memory management and leak detection tests |

## Test Scripts

| Script | Description |
|--------|-------------|
| `quick_regression.sh` | Fast regression test (4 cases, ~1 second) |
| `comprehensive_regression.sh` | Full regression suite (38 cases, ~5 seconds) |
| `regression_test.sh` | Detailed regression with diff output |

## Quick Commands

```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test test_integration
cargo test --test test_thread_safety
cargo test --test test_memory_safety

# Run regression tests
./tests/quick_regression.sh
./tests/comprehensive_regression.sh
```

## Full Documentation

For comprehensive testing documentation including:
- Memory leak detection methodology
- Detailed testing workflow
- CI integration guidelines
- Troubleshooting guide

See **[TESTING.md](../TESTING.md)** in the project root.
