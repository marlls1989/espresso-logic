# Espresso Logic Minimizer

[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Rust bindings to the Espresso heuristic logic minimiser from UC Berkeley, with a modern high-level API for Boolean function minimisation.

## Overview

Espresso takes Boolean functions and produces minimal or near-minimal equivalent representations. This Rust crate provides safe, thread-safe bindings with:

- **High-Level API** - Boolean expressions and truth tables (covers) with automatic dimension tracking
- **Automatic Minimisation** - Heuristic and exact algorithms
- **Multi-Output Support** - Minimise multiple outputs simultaneously
- **Thread-Safe** - Concurrent execution
- **Flexible Input** - Parse expressions, build truth tables, or load PLA files

## Features

- **Boolean Expressions** - Parse and compose expressions with the `expr!` macro — AND/OR/XOR/NOT in mathematical (`*`, `+`, `^`, `~`) or logical (`&`, `|`) notation — plus the `BoolExpr::build` closure builder (which `expr!` lowers to) and a `BoolExpr::ite` if-then-else
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
espresso-logic = "5.0"
```

### Boolean Expression Minimisation

```rust
use espresso_logic::{expr, Cover, Minimizable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build an expression with redundant terms (math or logical spellings both work).
    let redundant = expr!("a" * "b" + "a" * "b" * "c");
    println!("Original: {}", redundant);

    // Minimise it: route the expression through a cover, then read the result back.
    let minimised = Cover::from(&redundant).minimize()?.to_expr_by_index(0)?;
    println!("Minimised: {}", minimised);  // a & b  (Display uses canonical spellings)

    Ok(())
}
```

### Truth Table Minimisation (Cover API)

```rust
use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a cover for XOR function (positional, no labels)
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);

    // Push cubes: output is 1 when inputs differ
    cover.push(Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F)); // 01 -> 1
    cover.push(Cube::anonymous(&[Some(true), Some(false)], &[true], CubeType::F)); // 10 -> 1

    let minimised = cover.minimize()?;
    println!("Minimised to {} cubes", minimised.num_cubes());

    Ok(())
}
```

**Note:** Covers support multi-output functions, don't-care optimisation, and PLA file I/O.

### Reading and Writing PLA Files

```rust
use espresso_logic::{CoverType, Minimizable, PlaCover, PLAWriter, Symbol};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pla = "\
.i 2
.o 1
.ilb a b
.ob f
01 1
10 1
.e
";

    // Parse a PLA from a string (or use PlaCover::<Symbol>::from_pla_file(path))
    let cover = PlaCover::<Symbol>::from_pla_string(pla)?;
    println!("{} inputs, {} outputs, {} cubes",
        cover.num_inputs(), cover.num_outputs(), cover.num_cubes());

    // Minimise and write back out as PLA text
    let minimised = cover.minimize()?;
    let out = minimised.to_pla_string(CoverType::F)?;
    println!("{out}");

    Ok(())
}
```

## API Overview

Choose the right API for your use case:

### BoolExpr - For Expression-Based Logic

Use when you need to:
- Parse or compose Boolean expressions
- Work with single-output functions
- Use high-level operators and the `expr!` macro

```rust
use espresso_logic::{expr, Cover, Minimizable};

let xor = expr!("a" * !"b" + !"a" * "b");
let minimised = Cover::from(&xor).minimize()?;
```

`BoolExpr` is an owned, syntactic value — it carries no manager and is `Send`/`Sync`. Compose it with
the `expr!` macro (which lowers to the `BoolExpr::build` closure builder) or parse it from text;
equality is *syntactic*, so `a & b` and `b & a` are different values.

### Bdd - Canonical, semantic functions

When you need the Boolean *function* rather than its syntax — logical equivalence, cofactors,
quantification, tautology checks — build expressions into a BDD. A builder owns a private node table
and hands out `Bdd` handles branded to it, so handles from two different builders cannot be mixed (a
compile error, not a runtime check). Mint one with `bdd_builder!` (single-threaded) or
`sync_bdd_builder!` (`Send + Sync`); a `Bdd` handle is `Clone` and itself `Send`/`Sync` under the
sync builder. It also keeps its manager alive, so a handle can outlive the builder — recover a builder
onto the same manager with `handle.builder()`.

```rust
use espresso_logic::{bdd_builder, Minimizable};

let builder = bdd_builder!();

// Compose without `.clone()` in a scope: `ScopedBdd` handles are `Copy`, so operands compose in place
// (and may be reused). The BDD layer canonicalises, so logical laws hold.
let f = builder.scope(|s| s.var("a") & s.var("b"));
assert!(f.equivalent_to(&builder.parse("a & b").unwrap()));
assert!(builder.scope(|s| { let a = s.var("a"); a | !a }).is_tautology());

let minimised = f.minimize().unwrap();        // minimise the function to a cover

// A stored handle can recover a builder onto its manager and seed further construction.
let g = f.builder().parse("a | b").unwrap();
assert!(g.equivalent_to(&builder.parse("a | b").unwrap()));
```

### Cover - For Truth Tables and Multi-Output Functions

Use when you need to:
- Build truth tables directly with cubes
- Handle multi-output functions
- Control don't-care and off-sets (FD, FR, FDR types)
- Read/write PLA files

```rust
use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType};

let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::FD);  // with don't-cares
cover.push(Cube::anonymous(&[Some(true), None], &[true], CubeType::F));  // 1- -> 1
```

### Low-Level API - For Maximum Control

Use the `espresso` module directly when you need:
- Access to intermediate covers (ON-set, don't-care, OFF-set)
- Lower per-call overhead — the high-level API additionally validates the cover and rebuilds an output
  `Cover`, so the low-level edge is a fixed per-call cost: measured ~10–14% faster on small covers but
  only ~1–5% (within measurement noise) on large ones, and machine-/input-dependent (see the
  `api_overhead` group in `benches/pla_benchmarks.rs`)
- Fine-grained control over the minimisation process

See the [`espresso` module documentation](https://docs.rs/espresso-logic/latest/espresso_logic/espresso/) for details.

## Installation

**Prerequisites:** Rust 1.82+, C compiler, libclang

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
espresso -s input.pla        # Minimise with an execution summary on stderr
espresso -D stats input.pla  # Print PLA statistics only
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
