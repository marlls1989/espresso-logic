# Espresso Logic Minimizer

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A powerful Rust library for minimizing Boolean functions using the classic [Espresso heuristic logic minimizer](https://en.wikipedia.org/wiki/Espresso_heuristic_logic_minimizer) from UC Berkeley.

## Why Espresso Logic?

Espresso takes a Boolean function represented as a sum-of-products and produces a minimal or near-minimal equivalent representation. This Rust crate provides safe, idiomatic bindings with modern features.

**Key Contributions:**
- **Thread-Safe Execution** - The original Espresso C code used global state, making concurrent execution unsafe. This crate implements C11 thread-local storage for all globals, enabling safe parallel minimization across threads.
- **Safe Rust API** - Memory-safe wrappers with proper resource management
- **Modern Features** - Boolean expression parsing, operator overloading, and procedural macros

**Perfect for:**
- Digital logic synthesis and optimization
- PLA (Programmable Logic Array) minimization
- Boolean function simplification in CAD tools
- Concurrent logic minimization in multi-threaded applications
- Teaching Boolean algebra and logic design
- Research in logic optimization

## ✨ Features

- **Thread-Safe Concurrent Execution** - Unlike original Espresso, safely run minimization in parallel threads via C11 thread-local storage
- **Safe Rust API** - Memory-safe wrappers with zero-cost abstractions
- **Boolean Expressions** - High-level API with parsing, operator overloading, and `expr!` macro
- **PLA File Support** - Read and write Berkeley PLA format files
- **Minimal Dependencies** - Core library uses only `libc` and `lalrpop-util`
- **Command Line Tool** - Compatible with original Espresso CLI (optional)
- **Well Documented** - Comprehensive API documentation and examples

## 🚀 Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
espresso-logic = "3.0"
```

### Simple Example

```rust
use espresso_logic::{BoolExpr, expr};

fn main() -> std::io::Result<()> {
    // Build expression using string literals - no variable declarations needed!
    let xor = expr!("a" * "b" + !"a" * !"b");
    println!("Original: {}", xor);
    
    // Minimize it
    let minimized = xor.minimize()?;
    println!("Minimized: {}", minimized);
    
    Ok(())
}
```

### Working with Variables

```rust
use espresso_logic::{BoolExpr, expr};

fn main() -> std::io::Result<()> {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    
    // Build complex expression with redundant terms
    let expr = expr!(a * b + a * b * c + a * c);
    
    // Minimize automatically removes redundancy
    let minimized = expr.minimize()?;
    println!("Simplified: {}", minimized);
    
    // Check logical equivalence
    assert!(expr.equivalent_to(&minimized));
    
    Ok(())
}
```

## 📚 Common Use Cases

### Truth Table Minimization

```rust
use espresso_logic::{Cover, CoverType};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::new(CoverType::F);
    
    // XOR function: output is 1 when inputs differ
    cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);  // 01 -> 1
    cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);  // 10 -> 1
    
    cover.minimize()?;
    println!("Minimized to {} cubes", cover.num_cubes());
    
    Ok(())
}
```

### Multiple Outputs

```rust
use espresso_logic::{Cover, CoverType, BoolExpr, expr};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::new(CoverType::F);
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    
    // Define multiple outputs
    cover.add_expr(expr!(a * b), "and_gate")?;
    cover.add_expr(expr!(a + b), "or_gate")?;
    
    cover.minimize()?;
    
    // Get minimized results
    for (name, expr) in cover.to_exprs() {
        println!("{}: {}", name, expr);
    }
    
    Ok(())
}
```

### PLA Files

```rust
use espresso_logic::{Cover, CoverType};

fn main() -> std::io::Result<()> {
    // Read, minimize, and write back
    let mut cover = Cover::from_pla_file("input.pla")?;
    cover.minimize()?;
    cover.to_pla_file("output.pla", CoverType::F)?;
    
    println!("Minimized {} inputs, {} outputs", 
             cover.num_inputs(), cover.num_outputs());
    
    Ok(())
}
```

## 🎯 API Overview

This crate provides **two API levels** to suit different needs:

### High-Level API (Recommended)

Use `BoolExpr` and `Cover` for most applications:

- ✅ **Thread-safe by design** - No manual synchronization needed
- ✅ **Automatic memory management** - RAII handles cleanup
- ✅ **Clean syntax** - `expr!` macro and operator overloading
- ✅ **Dynamic dimensions** - Automatic dimension management
- ✅ **Easy to use** - Idiomatic Rust API

**Perfect for:**
- Application development
- Logic synthesis tools
- Educational projects
- Rapid prototyping

### Low-Level API (Advanced)

Direct `espresso::Espresso` and `espresso::EspressoCover` access for specialized needs:

- 📊 **Access to intermediate covers** - Get ON-set (F), don't-care (D), OFF-set (R) separately
- 🎯 **Custom don't-care/off-sets** - Provide your own D and R covers for optimization
- ⚡ **Maximum performance** - ~5-10% faster than high-level API, minimal overhead
- 🔧 **Explicit instance control** - Manually manage Espresso instance lifecycle

**Use when you need:**
- Access to all three covers (F, D, R) from minimization
- To provide custom don't-care or off-set covers
- Absolute maximum performance (5-10% speedup)
- Explicit control over instance creation/destruction

**Note:** Algorithm configuration via `EspressoConfig` works with **both** APIs - 
if you only need to tune algorithm parameters, use the high-level 
`Cover::minimize_with_config()` instead.

**⚠️ Important Constraints:**
- All covers on a thread must use the **same dimensions** until dropped
- Requires manual dimension management
- More complex error handling

```rust
use espresso_logic::espresso::{Espresso, EspressoCover, CubeType};
use espresso_logic::EspressoConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Custom configuration
    let mut config = EspressoConfig::default();
    config.single_expand = true;
    let _esp = Espresso::new(2, 1, &config);
    
    // Create and minimize
    let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
    let cover = EspressoCover::from_cubes(cubes, 2, 1)?;
    let (f, d, r) = cover.minimize(None, None);
    
    println!("Minimized to {} cubes", f.to_cubes(2, 1, CubeType::F).len());
    Ok(())
}
```

See [docs/API.md](docs/API.md) for complete API documentation and the `espresso` module documentation for detailed safety guidelines.

## 🛠️ Command Line Usage

Install the CLI tool (optional):

```bash
cargo install espresso-logic --features cli
```

Basic usage:

```bash
# Minimize a PLA file
espresso input.pla > output.pla

# Show statistics
espresso -s input.pla

# Exact minimization (slower but optimal)
espresso --do exact input.pla
```

See [docs/CLI.md](docs/CLI.md) for more options.

## 📦 Installation

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
Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) with C++ support

See [docs/INSTALLATION.md](docs/INSTALLATION.md) for detailed platform-specific instructions.

## 🎓 Examples

Run included examples to see different features:

```bash
# Boolean expressions
cargo run --example boolean_expressions

# XOR minimization
cargo run --example xor_function

# PLA file processing
cargo run --example pla_file

# Concurrent execution
cargo run --example concurrent_transparent
```

See [docs/EXAMPLES.md](docs/EXAMPLES.md) for comprehensive code examples.

## 📖 Documentation

- **[API Reference](https://docs.rs/espresso-logic)** - Complete API documentation
- **[Examples Guide](docs/EXAMPLES.md)** - Comprehensive usage examples
- **[Boolean Expressions](docs/BOOLEAN_EXPRESSIONS.md)** - Expression API details
- **[CLI Guide](docs/CLI.md)** - Command-line usage
- **[Installation](docs/INSTALLATION.md)** - Platform-specific setup
- **[Testing Guide](TESTING.md)** - How to run tests
- **[Contributing](CONTRIBUTING.md)** - How to contribute

## 🧪 Testing

```bash
cargo test
```

For comprehensive testing including memory safety and regression tests, see [TESTING.md](TESTING.md).

## 🌐 Compatibility

- **Rust:** 1.70+
- **Platforms:** Linux, macOS, Windows
- **Espresso Version:** 2.3 (01/31/88)

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📚 References

- [Original Espresso Paper](https://www2.eecs.berkeley.edu/Pubs/TechRpts/1984/CSD-84-175.pdf)
- [UC Berkeley Espresso Page](https://embedded.eecs.berkeley.edu/pubs/downloads/espresso/index.htm)
- [Wikipedia Article](https://en.wikipedia.org/wiki/Espresso_heuristic_logic_minimizer)

## 📄 License

This project contains three layers of licensed work:

- **Original Espresso**: UC Berkeley (permissive license)
- **Modernized C Code**: Sébastien Cottinet (MIT)
- **Rust Wrapper**: Marcos Sartori (MIT)

See [LICENSE](LICENSE) and [ACKNOWLEDGMENTS.md](ACKNOWLEDGMENTS.md) for details.

## 🙏 Acknowledgments

Espresso was developed by Robert K. Brayton and his team at UC Berkeley. Special thanks to:

- The original Espresso developers (Brayton, Hachtel, McMullen, Sangiovanni-Vincentelli)
- **Sébastien Cottinet** for the MIT-licensed modernized C version
- The Rust community for excellent FFI tools

For complete acknowledgments, see [ACKNOWLEDGMENTS.md](ACKNOWLEDGMENTS.md).

## 📊 Citation

If you use this library in academic work, please cite:

```bibtex
@article{brayton1984logic,
  title={Logic minimization algorithms for VLSI synthesis},
  author={Brayton, Robert K and Hachtel, Gary D and McMullen, Curtis T and Sangiovanni-Vincentelli, Alberto L},
  journal={Kluwer Academic Publishers},
  year={1984}
}
```

## 🔗 Related Projects

- [ABC](https://people.eecs.berkeley.edu/~alanmi/abc/) - Modern logic synthesis tool
- [Yosys](http://www.clifford.at/yosys/) - Open-source synthesis suite

## 💬 Support

- 🐛 [Report a bug](https://github.com/marlls1989/espresso-logic/issues)
- 💡 [Request a feature](https://github.com/marlls1989/espresso-logic/issues)
- 📖 [View documentation](https://docs.rs/espresso-logic)
- 💬 [Ask a question](https://github.com/marlls1989/espresso-logic/discussions)

---

Made with ❤️ for the Rust and digital logic communities.
