# Usage Examples

This guide provides comprehensive examples for using espresso-logic.

## Table of Contents

- [Boolean Expressions](#boolean-expressions)
- [Truth Tables](#truth-tables)
- [Multiple Outputs](#multiple-outputs)
- [PLA Files](#pla-files)
- [Expression Operations](#expression-operations)
- [Low-Level API](#low-level-api)
- [Concurrent Execution](#concurrent-execution)

## Boolean Expressions

### Basic Usage

```rust
use espresso_logic::{BoolExpr, expr};

fn main() -> std::io::Result<()> {
    // Three styles of building expressions
    
    // Style 1: String literals (most concise)
    let xor = expr!("a" * "b" + !"a" * !"b");
    println!("XOR: {}", xor);
    
    // Style 2: Variables
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    
    let expr = expr!(a * b + a * b * c);
    let minimized = expr.minimize()?;
    println!("Minimized: {}", minimized);  // Output: a * b
    
    // Style 3: Mixed
    let complex = expr!(a * "b" + !"c" * b);
    
    Ok(())
}
```

### Parsing Expressions

```rust
use espresso_logic::BoolExpr;

fn main() -> Result<(), espresso_logic::EspressoError> {
    // Parse from string
    let expr = BoolExpr::parse("(a + b) * (c + d)")?;
    let minimized = expr.minimize()?;
    println!("Result: {}", minimized);
    
    // Parse complex expressions
    let complex = BoolExpr::parse("a*b + b*c + a*c")?;
    let simple = complex.minimize()?;
    
    Ok(())
}
```

### Operator Overloading

```rust
use espresso_logic::BoolExpr;

let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");
let c = BoolExpr::variable("c");

// AND: * operator
let and = &a * &b;

// OR: + operator
let or = &a + &b;

// NOT: ! operator
let not = !&a;

// Complex expressions
let complex = (&a * &b) + (!&a * &c);

// Minimize
let minimized = complex.minimize()?;
```

## Truth Tables

### Building from Truth Tables

```rust
use espresso_logic::{Cover, CoverType};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::new(CoverType::F);
    
    // XOR function: a XOR b
    // Inputs: [a, b], Output: [f]
    cover.add_cube(&[Some(false), Some(false)], &[Some(false)]); // 00 -> 0
    cover.add_cube(&[Some(false), Some(true)],  &[Some(true)]);  // 01 -> 1
    cover.add_cube(&[Some(true),  Some(false)], &[Some(true)]);  // 10 -> 1
    cover.add_cube(&[Some(true),  Some(true)],  &[Some(false)]); // 11 -> 0
    
    cover.minimize()?;
    println!("Minimized to {} cubes", cover.num_cubes());
    
    Ok(())
}
```

### Using Don't Cares

```rust
use espresso_logic::{Cover, CoverType};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::new(CoverType::F);
    
    // Use None for don't care values
    cover.add_cube(&[Some(true), None], &[Some(true)]);  // 1- -> 1
    cover.add_cube(&[None, Some(true)], &[Some(true)]);  // -1 -> 1
    
    cover.minimize()?;
    
    Ok(())
}
```

## Multiple Outputs

### Named Outputs

```rust
use espresso_logic::{Cover, CoverType, BoolExpr, expr};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::new(CoverType::F);
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    
    // Add multiple outputs with names
    cover.add_expr(expr!(a * b), "and_output")?;
    cover.add_expr(expr!(a + c), "or_output")?;
    cover.add_expr(expr!(a * b + b * c), "complex_output")?;
    
    // Minimize all together
    cover.minimize()?;
    
    // Retrieve minimized expressions
    for (name, expr) in cover.to_exprs() {
        println!("{}: {}", name, expr);
    }
    
    Ok(())
}
```

## PLA Files

### Reading and Writing

```rust
use espresso_logic::{Cover, CoverType};

fn main() -> std::io::Result<()> {
    // Read from file
    let mut cover = Cover::from_pla_file("input.pla")?;
    
    println!("Inputs: {}", cover.num_inputs());
    println!("Outputs: {}", cover.num_outputs());
    println!("Cubes before: {}", cover.num_cubes());
    
    // Minimize
    cover.minimize()?;
    
    println!("Cubes after: {}", cover.num_cubes());
    
    // Write result
    cover.to_pla_file("output.pla", CoverType::F)?;
    
    Ok(())
}
```

### PLA File Format

Example PLA file:

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

See [PLA_FORMAT.md](PLA_FORMAT.md) for complete PLA file format specification.

## Expression Operations

### Semantic Equality

```rust
use espresso_logic::{BoolExpr, expr};

let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");

// Check logical equivalence
let expr1 = expr!(a * b);
let expr2 = expr!(b * a);
assert!(expr1.equivalent_to(&expr2));

// Different structure, same logic
let expr3 = expr!(a * b + a * b * "c");
let expr4 = expr!(a * b);
assert!(expr3.equivalent_to(&expr4));
```

### Evaluation

```rust
use espresso_logic::{BoolExpr, expr};
use std::collections::HashMap;
use std::sync::Arc;

let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");

let expr = expr!(a * b + !a);

// Create assignment
let mut assignment = HashMap::new();
assignment.insert(Arc::from("a"), true);
assignment.insert(Arc::from("b"), false);

// Evaluate
let result = expr.evaluate(&assignment);
println!("Result: {}", result);  // true
```

### Collecting Variables

```rust
use espresso_logic::{BoolExpr, expr};

let expr = expr!("a" * "b" + "c" * "d");

// Get all variables (sorted)
let vars = expr.collect_variables();
println!("Variables: {:?}", vars);  // {"a", "b", "c", "d"}
```

## Low-Level API

### Direct Espresso Usage

```rust
use espresso_logic::espresso::{EspressoCover, CubeType};

fn main() -> Result<(), String> {
    // Build cover from cubes
    let cubes = vec![
        (vec![0, 1], vec![1]),  // 01 -> 1
        (vec![1, 0], vec![1]),  // 10 -> 1
    ];
    let f = EspressoCover::from_cubes(cubes, 2, 1)?;
    
    // Minimize
    let (minimized, _d, _r) = f.minimize(None, None);
    
    // Extract results
    let result_cubes = minimized.to_cubes(2, 1, CubeType::F);
    println!("Result: {} cubes", result_cubes.len());
    
    Ok(())
}
```

### Custom Configuration

```rust
use espresso_logic::espresso::Espresso;
use espresso_logic::EspressoConfig;

fn main() -> Result<(), String> {
    // Custom configuration
    let mut config = EspressoConfig::default();
    config.single_expand = true;
    config.use_super_gasp = false;
    
    // Create instance with config
    let _esp = Espresso::new(2, 1, &config);
    
    // Now all operations use this configuration
    // ...
    
    Ok(())
}
```

## Concurrent Execution

### Thread-Safe High-Level API

```rust
use espresso_logic::{BoolExpr, expr};
use std::thread;

fn main() -> std::io::Result<()> {
    let handles: Vec<_> = (0..4).map(|i| {
        thread::spawn(move || -> std::io::Result<String> {
            let expr = expr!("a" * "b" + "a" * "c" + "b" * "c");
            let minimized = expr.minimize()?;
            Ok(format!("Thread {}: {}", i, minimized))
        })
    }).collect();
    
    for handle in handles {
        println!("{}", handle.join().unwrap()?);
    }
    
    Ok(())
}
```

### Parallel Cover Processing

```rust
use espresso_logic::{Cover, CoverType};
use std::thread;

fn main() -> std::io::Result<()> {
    let files = vec!["a.pla", "b.pla", "c.pla", "d.pla"];
    
    let handles: Vec<_> = files.into_iter().map(|file| {
        thread::spawn(move || -> std::io::Result<usize> {
            let mut cover = Cover::from_pla_file(file)?;
            cover.minimize()?;
            Ok(cover.num_cubes())
        })
    }).collect();
    
    for handle in handles {
        println!("Result: {} cubes", handle.join().unwrap()?);
    }
    
    Ok(())
}
```

## Advanced Examples

### Inspecting Cubes

```rust
use espresso_logic::Cover;

fn main() -> std::io::Result<()> {
    let mut cover = Cover::from_pla_file("input.pla")?;
    
    // Get cubes before minimization
    let before = cover.get_cubes();
    println!("Before: {} cubes", before.len());
    
    cover.minimize()?;
    
    // Get cubes after minimization
    let after = cover.get_cubes();
    println!("After: {} cubes", after.len());
    
    // Inspect individual cubes
    for (inputs, outputs) in after {
        println!("Inputs: {:?}, Outputs: {:?}", inputs, outputs);
    }
    
    Ok(())
}
```

## Running Examples

The repository includes runnable examples:

```bash
# Boolean expressions
cargo run --example boolean_expressions

# XOR function
cargo run --example xor_function

# PLA processing
cargo run --example pla_file

# Low-level API
cargo run --example espresso_direct_api

# Concurrent execution
cargo run --example concurrent_transparent

# Expression macro
cargo run --example expr_macro_demo

# Inspect cubes
cargo run --example inspect_cubes
```

