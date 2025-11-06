# Espresso Logic Minimizer

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A Rust wrapper for the [Espresso heuristic logic minimizer](https://en.wikipedia.org/wiki/Espresso_heuristic_logic_minimizer), a classic tool from UC Berkeley for minimizing Boolean functions.

## Overview

Espresso takes a Boolean function represented as a sum-of-products and produces a minimal or near-minimal equivalent representation. This Rust crate provides safe, idiomatic bindings to the original C implementation.

**Use cases:**
- Digital logic synthesis
- PLA (Programmable Logic Array) minimization
- Boolean function simplification
- Logic optimization in CAD tools
- Teaching Boolean algebra and logic design

## Features

- ✅ **Safe Rust API** - Memory-safe wrappers around the C library
- ✅ **Boolean Expressions** - NEW! High-level API with parsing, operator overloading, and `expr!` macro
- ✅ **Process Isolation** - Thread-safe concurrent execution via isolated processes
- ✅ **Command Line Interface** - Compatible with original Espresso CLI
- ✅ **PLA File Support** - Read and write Berkeley PLA format files
- ✅ **Flexible Input** - Boolean expressions, programmatic covers, or PLA files
- ✅ **Multiple Algorithms** - Both heuristic (fast) and exact (optimal) minimization
- ✅ **Zero-cost Abstractions** - Minimal overhead compared to direct C usage
- ✅ **Well Documented** - Comprehensive API documentation and examples

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
espresso-logic = "2.6"
```

### Command Line Usage

```bash
# Build the CLI
cargo build --release --bin espresso

# Minimize a PLA file
./target/release/espresso input.pla > output.pla

# With summary
./target/release/espresso -s input.pla

# Exact minimization
./target/release/espresso --do exact input.pla
```

### Boolean Expressions (High-Level API)

```rust
use espresso_logic::{BoolExpr, expr};

fn main() -> std::io::Result<()> {
    // Create variables
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    
    // Build expression with expr! macro for clean syntax
    let expr = expr!(a * b + a * b * c);  // Redundant term
    
    // Minimize directly
    let minimized = expr.minimize()?;
    println!("Minimized: {}", minimized);  // Output: (a * b)
    
    // Or parse from string
    let parsed = BoolExpr::parse("(a + b) * (c + d)")?;
    let result = parsed.minimize()?;
    
    Ok(())
}
```

### Library API Example (Low-Level)

```rust
use espresso_logic::Cover;

fn main() -> std::io::Result<()> {
    // Create a cover for a 2-input, 1-output function
    let mut cover = Cover::<2, 1>::new();
    
    // Build a truth table (XOR function)
    cover.add_cube(&[Some(false), Some(true)], &[true]); // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[true]); // 10 -> 1
    
    // Minimize - runs in isolated process
    cover.minimize()?;
    
    println!("Minimized! Cubes: {}", cover.num_cubes());
    Ok(())
}
```

### Working with PLA Files

```rust
use espresso_logic::{PLA, PLAType};

fn main() -> std::io::Result<()> {
    // Read a PLA file
    let pla = PLA::from_file("input.pla")?;
    pla.print_summary();
    
    // Minimize it
    let minimized = pla.minimize();
    
    // Write the result
    minimized.to_file("output.pla", PLAType::F)?;
    
    Ok(())
}
```

### Concurrent Execution (Process Isolation)

**NEW!** Thread-safe by default - the API uses **transparent process isolation** where all C code runs in isolated forked processes:

```rust
use espresso_logic::Cover;
use std::thread;

fn main() -> std::io::Result<()> {
    // Spawn multiple threads - each creates its own cover
    let handles: Vec<_> = (0..4).map(|_| {
        thread::spawn(move || {
            // Create cover (pure Rust, no C code)
            let mut cover = Cover::<2, 1>::new();
            cover.add_cube(&[Some(false), Some(true)], &[true]);
            cover.add_cube(&[Some(true), Some(false)], &[true]);
            
            // Minimize - C code runs in isolated worker process
            cover.minimize()?;
            Ok(cover.num_cubes())
        })
    }).collect();
    
    for handle in handles {
        let num_cubes = handle.join().unwrap()?;
        println!("Result: {} cubes", num_cubes);
    }
    Ok(())
}
```

**Key benefits:**
- ✅ Thread-safe by default - no synchronization needed
- ✅ Zero global state in parent process
- ✅ True parallelism - operations run concurrently
- ✅ Simple API with const generics for compile-time safety
- ✅ Efficient - uses shared memory IPC

See [docs/PROCESS_ISOLATION.md](docs/PROCESS_ISOLATION.md) for details.

## Installation

### Prerequisites

- Rust 1.70 or later
- C compiler (gcc, clang, or msvc)
- libclang (for bindgen during build)

**macOS:**
```bash
xcode-select --install
```

**Ubuntu/Debian:**
```bash
sudo apt-get install build-essential libclang-dev
```

**Windows:**
- Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) with C++ support
- Or use [MSYS2](https://www.msys2.org/) with mingw-w64

### Building

```bash
cargo build --release
```

The build script automatically compiles the C source code and generates FFI bindings.

## Examples

The crate includes several examples demonstrating different use cases:

```bash
# Boolean expressions (high-level API) - NEW!
cargo run --example boolean_expressions

# Basic minimization
cargo run --example minimize

# XOR function (classic example)
cargo run --example xor_function

# PLA file processing
cargo run --example pla_file

# PLA file with output
cargo run --example pla_file output.pla

# Process-isolated transparent API
cargo run --example transparent_api

# Concurrent execution with full isolation
cargo run --example concurrent_transparent
```

## PLA File Format

Espresso uses the Berkeley PLA format. Here's a simple example:

```
.i 2        # 2 inputs
.o 1        # 1 output
.ilb a b    # input labels
.ob f       # output label
.p 2        # 2 product terms
01 1        # a=0, b=1 => f=1
10 1        # a=1, b=0 => f=1
.e          # end
```

**Notation:**
- `0` - Variable must be 0
- `1` - Variable must be 1
- `-` - Don't care

## API Overview

### Core Types

#### High-Level API (Boolean Expressions)
- **`BoolExpr`** - Boolean expression with operator overloading
- **`ExprCover`** - Cover representation for boolean expressions
- **`expr!`** - Macro for clean expression syntax

#### Low-Level API (Cubes and Covers)
- **`Cover<I, O>`** - Generic cover with compile-time dimensions
- **`CoverBuilder<I, O>`** - Builder for constructing covers programmatically
- **`PLACover`** - Dynamic cover for PLA files
- **`PLAType`** - Output format specifier

### Key Methods

```rust
// Boolean expressions (high-level)
let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");
let expr = expr!(a * b + !a * !b);
let minimized = expr.minimize()?;

// Or parse from string
let expr = BoolExpr::parse("(a + b) * c")?;

// Low-level cover API
let mut cover = CoverBuilder::<2, 1>::new();
cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
cover.minimize()?;

// PLA file operations
let mut pla = PLACover::from_pla_file("file.pla")?;
pla.minimize()?;
pla.to_pla_file("out.pla", PLAType::F)?;
```

See [docs/API.md](docs/API.md) for complete API documentation.

## Performance

The Rust wrapper has negligible overhead compared to the C library:

- Heuristic minimization is fast and suitable for most use cases
- Exact minimization guarantees optimality but is slower
- Large functions (>1000 cubes) may require significant time

## Architecture

The crate is organized into three layers:

1. **C Library** (`espresso-src/`) - Original Espresso implementation
2. **FFI Bindings** (`src/sys.rs`) - Auto-generated by bindgen
3. **Safe API** (`src/lib.rs`) - Idiomatic Rust wrappers

Memory management is handled automatically through RAII patterns.

## Testing

### Unit and Integration Tests

Run the Rust test suite:

```bash
cargo test
```

Run with verbose output:

```bash
cargo test -- --nocapture
```

### Regression Tests (CLI)

The project includes regression tests that validate the Rust CLI against the original C implementation:

```bash
# Quick regression test (4 cases, ~1 second)
./tests/quick_regression.sh

# Comprehensive regression test (38 cases, ~5 seconds)
./tests/comprehensive_regression.sh
```

**Status**: ✅ 38/38 tests passing - Rust CLI produces identical output to C CLI

## Documentation

Generate and view the API documentation:

```bash
cargo doc --open
```

Additional documentation:
- [Boolean Expressions Guide](docs/BOOLEAN_EXPRESSIONS.md) - Comprehensive guide to the expression API
- [API Reference](docs/API.md) - Complete API documentation
- [Command Line Interface](docs/CLI.md) - CLI usage guide
- [Process Isolation](docs/PROCESS_ISOLATION.md) - Thread-safe concurrent execution
- [Contributing Guidelines](CONTRIBUTING.md)
- [Original Espresso README](espresso-src/README)

## Compatibility

- **Rust:** 1.70 or later
- **Platforms:** Linux, macOS, Windows
- **Espresso Version:** 2.3 (Release date 01/31/88)
- **Wrapper Version:** 2.3.0 (matches Espresso version)

## Limitations

- **Global State (Solved!)**: The C library uses global state, but our **process isolation** feature provides thread-safe concurrent execution
- Very large Boolean functions may exhaust memory
- PLA file format has some limitations compared to modern formats
- Process isolation currently requires Unix-like systems (Linux, macOS, BSD) - Windows support planned

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## References

- [Espresso Paper](https://www2.eecs.berkeley.edu/Pubs/TechRpts/1984/CSD-84-175.pdf) - Original research paper
- [UC Berkeley Espresso Page](https://embedded.eecs.berkeley.edu/pubs/downloads/espresso/index.htm)
- [Wikipedia Article](https://en.wikipedia.org/wiki/Espresso_heuristic_logic_minimizer)
- Brayton, R. K., et al. "Logic minimization algorithms for VLSI synthesis." (1984)

## License

This project contains three layers of licensed work:

- **Original Espresso**: Copyright (c) 1988, 1989, Regents of the University of California (UC Berkeley License)
- **Modernized C Code**: Copyright (c) 2016 Sébastien Cottinet (MIT License)
- **Rust Wrapper**: Copyright (c) 2024 Marcos Sartori (MIT License)

All are permissive licenses. See [LICENSE](LICENSE) for complete terms and [ACKNOWLEDGMENTS.md](ACKNOWLEDGMENTS.md) for attribution details.

## Acknowledgments

The Espresso logic minimizer was developed by Robert K. Brayton and his team at UC Berkeley. This Rust wrapper builds upon their excellent work to make it accessible to the Rust ecosystem.

**Copyright (c) 1988, 1989, Regents of the University of California. All rights reserved.**

Special thanks to:
- The original Espresso developers at UC Berkeley (Brayton, Hachtel, McMullen, Sangiovanni-Vincentelli)
- The Electronics Research Laboratory at UC Berkeley
- **Sébastien Cottinet** for the 2016 MIT-licensed modernized C version
- classabbyamp for maintaining the modernized C codebase
- The Rust community for excellent FFI tools (bindgen, cc crate)
- Contributors to this wrapper

For complete acknowledgments and license compliance information, see [ACKNOWLEDGMENTS.md](ACKNOWLEDGMENTS.md)

## Citation

If you use this library in academic work, please cite the original Espresso paper:

```bibtex
@article{brayton1984logic,
  title={Logic minimization algorithms for VLSI synthesis},
  author={Brayton, Robert K and Hachtel, Gary D and McMullen, Curtis T and Sangiovanni-Vincentelli, Alberto L},
  journal={Kluwer Academic Publishers},
  year={1984}
}
```

## Related Projects

- [ABC](https://people.eecs.berkeley.edu/~alanmi/abc/) - A modern logic synthesis tool from UC Berkeley
- [Yosys](http://www.clifford.at/yosys/) - Open-source synthesis suite
- [PLA Tools](https://github.com/topics/pla) - Other PLA manipulation tools

## Support

- 🐛 [Report a bug](https://github.com/marlls1989/espresso-logic/issues)
- 💡 [Request a feature](https://github.com/marlls1989/espresso-logic/issues)
- 📖 [View documentation](https://docs.rs/espresso-logic)
- 💬 [Ask a question](https://github.com/marlls1989/espresso-logic/discussions)

---

Made with ❤️ for the Rust and digital logic communities.
