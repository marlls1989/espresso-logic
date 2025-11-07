# Espresso Logic Benchmarks

Comprehensive benchmark suite for the Espresso logic minimizer using [Criterion](https://github.com/bheisler/criterion.rs).

## Overview

These benchmarks test performance across available PLA test files (100+ files from `examples/`, `tlex/`, and `hard_examples/`), automatically categorizing them by size and complexity:

- **Small**: < 10 cubes
- **Medium**: 10-100 cubes  
- **Large**: 100-1000 cubes
- **Very Large**: > 1000 cubes

**Sampling Strategy**: To provide comprehensive coverage while keeping execution time reasonable, the suite randomly selects **10 files from each size category** (40 files total). This ensures balanced testing across problem sizes with full Criterion statistics (100 samples per file).

## Running Benchmarks

### Run all benchmarks

```bash
cargo bench --bench pla_benchmarks
```

### Run a specific benchmark group

```bash
# Parse performance only
cargo bench --bench pla_benchmarks parse_pla

# Minimization performance only
cargo bench --bench pla_benchmarks minimize

# Full pipeline (parse + minimize)
cargo bench --bench pla_benchmarks full_pipeline

# Performance by category
cargo bench --bench pla_benchmarks by_category

# Cube iteration performance
cargo bench --bench pla_benchmarks cube_iteration
```

### Quick benchmarks (faster, less accurate)

```bash
cargo bench --bench pla_benchmarks -- --quick
```

### Save and compare results

```bash
# Save baseline
cargo bench --bench pla_benchmarks -- --save-baseline before

# Make changes to code...

# Compare against baseline
cargo bench --bench pla_benchmarks -- --baseline before
```

## Benchmark Groups

### `parse_pla`

Measures parsing performance for PLA files. Tests the overhead of converting PLA text format into internal representation.

- **Metric**: Time to parse
- **Throughput**: Elements (cubes) per second
- **Files tested**: 10 files from each size category (~40 files total)

### `minimize`

Measures the core Espresso minimization algorithm performance.

- **Metric**: Time to minimize
- **Throughput**: Cubes processed per second  
- **Files tested**: 10 files from each size category (~40 files total)
- **Note**: Uses Criterion default settings (100 samples) for comprehensive statistics

### `full_pipeline`

Measures end-to-end performance including both parsing and minimization.

- **Metric**: Total time (parse + minimize)
- **Throughput**: Cubes processed per second
- **Files tested**: 10 files from each size category (~40 files total)
- **Use case**: Real-world usage pattern

### `by_category`

Compares performance across problem sizes, showing how the algorithm scales.

- **Metric**: Time per category
- **Purpose**: Understand scalability characteristics
- **Files tested**: One representative file from each category

### `cube_iteration`

Measures the performance of iterating over cubes in a cover.

- **Metric**: Time to iterate all cubes
- **Purpose**: API overhead measurement
- **File tested**: One medium-sized file

## Output

Criterion generates detailed reports in `target/criterion/`:

- **HTML reports**: `target/criterion/report/index.html`
- **Individual plots**: `target/criterion/<benchmark_name>/`
- **Raw data**: `target/criterion/<benchmark_name>/base/`

Open the HTML report in a browser for:
- Performance graphs
- Statistical analysis
- Comparison charts
- Outlier detection

## Test Files

Benchmarks automatically discover PLA files from:

- `examples/` - Basic test cases
- `tlex/` - Extended test suite (41 files)
- `hard_examples/` - Challenging minimization problems

Files are automatically categorized by initial cube count before minimization.

## Performance Tips

1. **Build in release mode**: Benchmarks automatically use `--release`
2. **Close other applications**: Reduce system load for accurate measurements
3. **Warm-up runs**: Criterion does automatic warm-up before measuring
4. **Multiple samples**: Each benchmark is run multiple times for statistical significance
5. **Consistent environment**: Run benchmarks on the same machine for comparisons

## Example Output

```
minimize/espresso/small/examples/tms
                        time:   [154.23 µs 154.89 µs 155.74 µs]
                        thrpt:  [0.0000  elem/s 0.0000  elem/s 0.0000  elem/s]

minimize/espresso/medium/examples/ex5
                        time:   [3.2145 ms 3.2289 ms 3.2451 ms]
                        thrpt:  [15.382 Kelem/s 15.459 Kelem/s 15.530 Kelem/s]

minimize/espresso/large/examples/mainpla
                        time:   [125.67 ms 126.34 ms 127.12 ms]
                        thrpt:  [3.5421 Kelem/s 3.5637 Kelem/s 3.5827 Kelem/s]
```

## CI/CD Integration

Add to your CI pipeline to track performance over time:

```yaml
- name: Run benchmarks
  run: cargo bench --bench pla_benchmarks -- --save-baseline ci-${{ github.sha }}
```

## Troubleshooting

### No PLA files found

Ensure you have the test files:
```bash
ls examples/*.pla tlex/*.pla hard_examples/*
```

### Benchmarks take too long

The full suite tests ~40 files with 100 samples each (balanced across size categories). Use `--quick` flag (10 samples) or filter specific benchmark groups:
```bash
# Quick mode (10 samples instead of 100)
cargo bench --bench pla_benchmarks -- --quick

# Or run specific groups only
cargo bench --bench pla_benchmarks -- parse_pla
```

### Memory issues with large files

The suite uses balanced sampling (10 files per category). If you encounter memory issues, use `--quick` mode or run specific benchmark groups.

## Contributing

When adding new benchmark scenarios:

1. Add new benchmark function
2. Register in `criterion_group!` macro
3. Update this README with description
4. Consider performance impact on CI

