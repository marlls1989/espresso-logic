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
- ✅ **Boolean Expressions** - High-level API with parsing, operator overloading, and `expr!` macro
- ✅ **Thread-Safe** - Concurrent execution via C11 thread-local storage
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

## Which API Should I Use?

This library provides three API levels to suit different needs:

### High-Level: Boolean Expressions (`BoolExpr`)
**✅ Recommended for most users**
- ✅ Thread-safe by design - use freely in concurrent applications
- ✅ Automatic lifetime management - thread-local Espresso instances handled transparently
- ✅ Easy to use - parse from strings or build programmatically
- ✅ Clean syntax with `expr!` macro

**Use when:** You want the simplest, safest API for boolean minimization

### Mid-Level: Typed Covers (`Cover`, `CoverBuilder`, `PLACover`)
- ✅ Thread-safe by design - use freely in concurrent applications
- ✅ Automatic lifetime management - thread-local Espresso instances handled transparently
- ✅ Type-safe with compile-time dimensions (`Cover<I, O>`)
- ✅ Good for programmatic cover construction
- ✅ Direct PLA file support

**Use when:** You need typed covers or work with PLA files

### Low-Level: Direct Espresso API (`Espresso`, `EspressoCover`)
- ⚡ Maximum performance - minimal overhead
- 🎛️ Fine-grained control over minimization process
- 🔧 Access to intermediate results (F, D, R covers)
- ⚠️ **Thread-local state** - one Espresso instance per thread
- ⚠️ **Not thread-safe** - covers are `!Send + !Sync`
- ⚠️ **Manual management** - requires understanding of constraints

**Use when:** You need maximum control and performance, and understand thread-local limitations

## Usage Examples

### Boolean Expressions (High-Level API) - Recommended

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

### Typed Cover API (Mid-Level)

```rust
use espresso_logic::Cover;

fn main() -> std::io::Result<()> {
    // Create a cover for a 2-input, 1-output function
    let mut cover = Cover::<2, 1>::new();
    
    // Build a truth table (XOR function)
    cover.add_cube(&[Some(false), Some(true)], &[true]); // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[true]); // 10 -> 1
    
    // Minimize - thread-safe via thread-local storage
    cover.minimize()?;
    
    println!("Minimized! Cubes: {}", cover.num_cubes());
    Ok(())
}
```

### Direct Espresso API (Low-Level)

The low-level API provides direct access to the Espresso C library with maximum performance and control.

**⚠️ Thread-Local State Constraints:**
- Each thread has its own independent `Espresso` instance
- `EspressoCover` objects are tied to their `Espresso` instance
- Covers cannot be shared between threads (`!Send + !Sync`)
- High-level APIs abstract these limitations away

**Simple usage** (Espresso instance managed automatically):

```rust
use espresso_logic::espresso::{EspressoCover, CubeType};

fn main() -> Result<(), String> {
    // Build a cover (XOR function) - Espresso instance created automatically
    let cubes = vec![
        (vec![0, 1], vec![1]),  // 01 -> 1
        (vec![1, 0], vec![1]),  // 10 -> 1
    ];
    let f = EspressoCover::from_cubes(cubes, 2, 1)?;
    
    // Minimize directly on the cover
    let (minimized, _d, _r) = f.minimize(None, None);
    
    // Extract results
    let result_cubes = minimized.to_cubes(2, 1, CubeType::F);
    println!("Minimized to {} cubes", result_cubes.len());
    Ok(())
}
```

**Advanced usage** with explicit configuration:

```rust
use espresso_logic::espresso::{Espresso, EspressoCover};
use espresso_logic::EspressoConfig;

fn main() -> Result<(), String> {
    // Explicitly create an Espresso instance with custom config
    let mut config = EspressoConfig::default();
    config.single_expand = true;
    let _esp = Espresso::new(2, 1, &config);
    
    // Now all covers on this thread will use this instance
    let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
    let f = EspressoCover::from_cubes(cubes, 2, 1)?;
    let (minimized, _, _) = f.minimize(None, None);
    Ok(())
}
```

**Multi-threaded usage** - each thread gets independent state:

```rust
use espresso_logic::espresso::{EspressoCover, CubeType};
use std::thread;

fn main() -> Result<(), String> {
    let handles: Vec<_> = (0..4).map(|_| {
        thread::spawn(|| -> Result<usize, String> {
            // Each thread automatically gets its own Espresso instance
            let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
            let f = EspressoCover::from_cubes(cubes, 2, 1)?;
            
            // Thread-safe: independent global state per thread
            let (result, _, _) = f.minimize(None, None);
            Ok(result.to_cubes(2, 1, CubeType::F).len())
        })
    }).collect();
    
    for handle in handles {
        let num_cubes = handle.join().unwrap()?;
        println!("Result: {} cubes", num_cubes);
    }
    Ok(())
}
```

See [docs/API.md](docs/API.md) for complete low-level API documentation and [docs/THREAD_LOCAL_IMPLEMENTATION.md](docs/THREAD_LOCAL_IMPLEMENTATION.md) for technical details.

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

## Command Line Usage

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
# Boolean expressions (high-level API)
cargo run --example boolean_expressions

# Basic minimization
cargo run --example minimize

# XOR function (classic example)
cargo run --example xor_function

# PLA file processing
cargo run --example pla_file

# Direct Espresso API
cargo run --example espresso_direct_api

# Concurrent execution
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

## Testing

Run the test suite:

```bash
cargo test
```

For comprehensive testing including regression tests and memory safety validation, see [TESTING.md](TESTING.md).

## Documentation

Generate and view the API documentation:

```bash
cargo doc --open
```

Additional documentation:
- [API Reference](docs/API.md) - Complete API documentation including low-level Espresso API
- [Boolean Expressions Guide](docs/BOOLEAN_EXPRESSIONS.md) - Comprehensive guide to the expression API
- [Command Line Interface](docs/CLI.md) - CLI usage guide
- [Testing Guide](TESTING.md) - Comprehensive testing documentation
- [Memory Safety Analysis](docs/MEMORY_SAFETY.md) - C FFI memory management verification
- [Thread-Local Implementation](docs/THREAD_LOCAL_IMPLEMENTATION.md) - Thread-safe concurrent execution
- [Process Isolation (Historical)](docs/PROCESS_ISOLATION.md) - Previous implementation (pre-2.6.2)
- [Contributing Guidelines](CONTRIBUTING.md)
- [Original Espresso README](espresso-src/README)

## Compatibility

- **Rust:** 1.70 or later
- **Platforms:** Linux, macOS, Windows
- **Espresso Version:** 2.3 (Release date 01/31/88)
- **Wrapper Version:** 2.3.0 (matches Espresso version)

## Limitations

- Very large Boolean functions may exhaust memory
- PLA file format has some limitations compared to modern formats
- Low-level API requires understanding of thread-local state constraints

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
