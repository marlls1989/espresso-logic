# Test Suite

This directory contains the test suite for the espresso-logic library.

## Test Files

| File | Description |
|------|-------------|
| `test_integration.rs` | Integration tests for high-level APIs (BoolExpr, Cover) |
| `test_thread_safety.rs` | Thread-local storage and concurrency tests |
| `test_memory_safety.rs` | Memory management and leak detection tests |
| `test_boolean_expressions.rs` | Tests for boolean expression parsing and evaluation |

## Test Scripts

| Script | Description | Tests | Duration |
|--------|-------------|-------|----------|
| `regression_test.sh` | Comprehensive regression suite | ~283 tests | ~45 seconds |
| `quick_regression.sh` | Fast sanity check for development | 4 tests | <5 seconds |

### Regression Tests

**`regression_test.sh`** - Comprehensive test suite that compares Rust CLI output against C CLI:
- All basic examples (ex*, b*, in*, m*, t* files) with default output
- All output format variations (f, fd, fr, fdr) on representative basic examples
- All .pla files from tlex/ with default output
- All .pla files from tlex/ with all output formats (f, fd, fr, fdr)
- Timeout protection (30s per test)
- Detailed diff output on failures

**`quick_regression.sh`** - Fast iteration during development:
- Tests 4 representative examples
- Incremental builds (no clean)
- 10s timeout per test
- Minimal output for speed

## Quick Commands

```bash
# Run all Rust tests
cargo test

# Run specific test file
cargo test --test test_integration
cargo test --test test_thread_safety
cargo test --test test_memory_safety

# Run regression tests
./tests/regression_test.sh          # Full test suite
./tests/quick_regression.sh         # Quick sanity check
```

## Full Documentation

For comprehensive testing documentation including:
- Memory leak detection methodology
- Detailed testing workflow
- CI integration guidelines
- Troubleshooting guide

See **[TESTING.md](../TESTING.md)** in the project root.
