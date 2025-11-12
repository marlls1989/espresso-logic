# Espresso Logic Minimizer

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Rust bindings to the Espresso heuristic logic minimiser from UC Berkeley, with a modern high-level API for Boolean function minimisation.

## Overview

Espresso takes Boolean functions and produces minimal or near-minimal equivalent representations. This Rust crate provides safe, thread-safe bindings with modern features:

- **High-Level API** - Boolean expressions and truth tables (covers) with automatic dimension tracking
- **Automatic Minimisation** - Heuristic and exact algorithms
- **Multi-Output Support** - Minimise multiple outputs simultaneously
- **Thread-Safe** - Safe concurrent execution
- **Flexible Input** - Parse expressions, build truth tables, or load PLA files

## Features

- **Boolean Expressions** - Parse and compose expressions with the `expr!` macro, supporting both mathematical (`*`, `+`) and logical (`&`, `|`) notation
- **Cover API** - Direct truth table manipulation with automatic dimension management for precise control
- **Multi-Output Functions** - Minimise multiple outputs simultaneously in a single cover
- **Advanced Minimisation** - Both heuristic (fast, ~99% optimal) and exact (guaranteed minimal) algorithms
- **Don't-Care Optimisation** - FD, FR, and FDR cover types for flexible optimisation with don't-care and off-sets
- **Thread-Safe** - Safe concurrent execution with C11 thread-local storage
- **PLA File Support** - Read and write Berkeley PLA format for interoperability

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
espresso-logic = "3.1"
```

### Boolean Expression Minimisation

```rust
use espresso_logic::{BoolExpr, expr};

fn main() -> std::io::Result<()> {
    // Build expression with redundant terms
    let redundant = expr!("a" * "b" + "a" * "b" * "c");
    println!("Original: {}", redundant);
    
    // Minimise it
    let minimised = redundant.minimize()?;
    println!("Minimised: {}", minimised);  // Output: a * b
    
    Ok(())
}
```

### Truth Table Minimisation (Cover API)

```rust
use espresso_logic::{Cover, CoverType};

fn main() -> std::io::Result<()> {
    // Create a cover for XOR function
    let mut cover = Cover::new(CoverType::F);
    
    // Add cubes: output is 1 when inputs differ
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)])?;  // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)])?;  // 10 -> 1
    
    let minimised = cover.minimize()?;
    println!("Minimised to {} cubes", minimised.num_cubes());
    
    Ok(())
}
```

**Note:** Covers support multi-output functions, don't-care optimisation, and PLA file I/O.

## API Overview

Choose the right API for your use case:

### BoolExpr - For Expression-Based Logic

Use when you need to:
- Parse or compose Boolean expressions
- Work with single-output functions
- Use high-level operators and the `expr!` macro

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

let xor = expr!("a" * !"b" + !"a" * "b");
let minimised = xor.minimize()?;
```

### Cover - For Truth Tables and Multi-Output Functions

Use when you need to:
- Build truth tables directly with cubes
- Handle multi-output functions
- Control don't-care and off-sets (FD, FR, FDR types)
- Read/write PLA files

```rust
use espresso_logic::{Cover, CoverType};

let mut cover = Cover::new(CoverType::FD);  // with don't-cares
cover.add_cube(&[Some(true), None], &[Some(true)])?;  // 1- -> 1
```

### Low-Level API - For Maximum Control

Use the `espresso` module directly when you need:
- Access to intermediate covers (ON-set, don't-care, OFF-set)
- Maximum performance (~5-10% faster)
- Fine-grained control over the minimisation process

See the [`espresso` module documentation](https://docs.rs/espresso-logic/latest/espresso_logic/espresso/) for details.

## Installation

**Prerequisites:** Rust 1.70+, C compiler, libclang

```bash
# macOS
xcode-select --install

# Ubuntu/Debian
sudo apt-get install build-essential libclang-dev
```

See [docs/INSTALLATION.md](docs/INSTALLATION.md) for detailed platform-specific instructions.

## Command Line Tool

Install the optional CLI:

```bash
cargo install espresso-logic --features cli
```

Usage:

```bash
espresso input.pla > output.pla
espresso -s input.pla  # Show statistics
```

See [docs/CLI.md](docs/CLI.md) for more options.

## Examples

Run included examples:

```bash
cargo run --example boolean_expressions
cargo run --example xor_function
cargo run --example pla_file
cargo run --example concurrent_transparent
```

See [docs/EXAMPLES.md](docs/EXAMPLES.md) for comprehensive code examples.

## Documentation

- [API Reference](https://docs.rs/espresso-logic) - Complete API documentation
- [Examples Guide](docs/EXAMPLES.md) - Comprehensive usage examples
- [Boolean Expressions](docs/BOOLEAN_EXPRESSIONS.md) - Expression API details
- [PLA Format](docs/PLA_FORMAT.md) - PLA file format specification
- [CLI Guide](docs/CLI.md) - Command-line usage
- [Installation](docs/INSTALLATION.md) - Platform-specific setup

## Testing

```bash
cargo test
```

See [TESTING.md](TESTING.md) for comprehensive testing documentation.

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

This project contains three layers of licensed work:

- **Original Espresso**: UC Berkeley (permissive license)
- **Modernized C Code**: Sébastien Cottinet (MIT)
- **Rust Wrapper**: Marcos Sartori (MIT)

See [LICENSE](LICENSE) and [ACKNOWLEDGMENTS.md](ACKNOWLEDGMENTS.md) for details.

## Acknowledgments

Espresso was developed by Robert K. Brayton and his team at UC Berkeley. Special thanks to the original developers and Sébastien Cottinet for the modernized C version.

## Citation

If you use this library in academic work, please cite:

```bibtex
@article{brayton1984logic,
  title={Logic minimization algorithms for VLSI synthesis},
  author={Brayton, Robert K and Hachtel, Gary D and McMullen, Curtis T and Sangiovanni-Vincentelli, Alberto L},
  journal={Kluwer Academic Publishers},
  year={1984}
}
```

---

Made with ❤️ for the Rust and digital logic communities.
