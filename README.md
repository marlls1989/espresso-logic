# Espresso Logic Minimizer

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A Rust library for minimizing Boolean functions using the classic Espresso heuristic logic minimizer from UC Berkeley. Perfect for digital logic synthesis, PLA optimization, and Boolean function simplification.

## Why Espresso?

Espresso takes Boolean functions in sum-of-products form and produces minimal or near-minimal equivalents. This Rust crate provides safe, thread-safe bindings with modern features like expression parsing and operator overloading.

## Features

- **Thread-Safe** - Safe concurrent execution via C11 thread-local storage
- **Boolean Expressions** - High-level API with parsing, operators, and `expr!` macro
- **Unified BDD Representation (v3.1.1+)** - All boolean expressions use BDD as their canonical internal representation, providing efficient operations, structural sharing, and automatic simplification
- **Flexible Parser** - Supports both mathematical (`*`, `+`) and logical (`&`, `|`) operator notations
- **Expression Composition** - Seamlessly compose parsed, minimised, and constructed expressions
- **Algebraic Factorisation (v3.1.1+)** - Beautiful multi-level logic display with common factor extraction
- **Both Algorithms** - Exposes both heuristic and exact minimisation algorithms from Espresso
- **PLA File Support** - Read and write Berkeley PLA format
- **Two API Levels** - High-level for ease of use, low-level for maximum control
- **Well Documented** - Comprehensive API docs and examples

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
espresso-logic = "3.1"
```

### Boolean Expression Minimization

```rust
use espresso_logic::{BoolExpr, expr};

fn main() -> std::io::Result<()> {
    // Build expression with redundant terms
    let redundant = expr!("a" * "b" + "a" * "b" * "c");
    println!("Original: {}", redundant);
    
    // Minimize it
    let minimized = redundant.minimize()?;
    println!("Minimized: {}", minimized);  // Output: a * b
    
    Ok(())
}
```

### Truth Table Minimization

```rust
use espresso_logic::{Cover, CoverType};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::new(CoverType::F);
    
    // XOR function: output is 1 when inputs differ
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)])?;  // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)])?;  // 10 -> 1
    
    cover.minimize()?;
    println!("Minimized to {} cubes", cover.num_cubes());
    
    Ok(())
}
```

### PLA File Processing

```rust
use espresso_logic::{Cover, CoverType, PLAReader, PLAWriter};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::from_pla_file("input.pla")?;
    cover.minimize()?;
    cover.to_pla_file("output.pla", CoverType::F)?;
    Ok(())
}
```

### Binary Decision Diagrams

**Note (v3.1.1+):** `BoolExpr` and `Bdd` are now unified—`Bdd` is a type alias for `BoolExpr`. All expressions use BDD as their internal representation, providing canonical form, efficient operations, and automatic simplification.

```rust
use espresso_logic::{BoolExpr, Bdd};

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr = a.and(&b).or(&b.and(&c));
    
    // BoolExpr IS a BDD - canonical representation
    println!("BDD has {} nodes", expr.node_count());
    
    // All BoolExpr instances support efficient BDD operations
    let combined = expr.and(&c);
    
    // Display uses algebraic factorisation (v3.1.1+)
    println!("Result: {}", combined);
}
```

## API Overview

### High-Level API (Recommended)

Use `BoolExpr` (alias `Bdd`) and `Cover` for most applications:

- **Unified BDD representation (v3.1.1+)** - All expressions are BDDs internally
- Automatic memory management and dimension tracking
- Thread-safe by design
- Clean, idiomatic Rust API
- Canonical representation with structural sharing
- Efficient operations via hash consing and memoisation

```rust
use espresso_logic::{BoolExpr, expr};

// Parse with flexible notation: * or & for AND, + or | for OR
let xor = expr!("a" * !"b" + !"a" * "b");
let xor_alt = BoolExpr::parse("a & !b | !a & b")?;

// All expressions ARE BDDs - canonical representation
// Minimise to get optimal logic
let minimised = xor.minimize()?;
println!("{}", minimised);  // Uses algebraic factorisation (v3.1.1+)
```

### Low-Level API (Advanced)

Direct `espresso::Espresso` access for specialized needs:

- Access to intermediate covers (ON-set, don't-care, OFF-set)
- Custom don't-care/off-set optimization
- Maximum performance (~5-10% faster)

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
