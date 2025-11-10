# Usage Examples

This guide provides comprehensive examples for using espresso-logic.

## Table of Contents

- [Boolean Expressions](#boolean-expressions)
- [Truth Tables](#truth-tables)
- [Multiple Outputs](#multiple-outputs)
- [PLA Files](#pla-files)
- [Expression Operations](#expression-operations)
- [Binary Decision Diagrams (BDDs)](#binary-decision-diagrams-bdds)
- [Low-Level API](#low-level-api)
- [Concurrent Execution](#concurrent-execution)

## Boolean Expressions

### Basic Usage

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

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
use espresso_logic::{BoolExpr, Minimizable};

fn main() -> std::io::Result<()> {
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
use espresso_logic::{BoolExpr, Minimizable};

fn main() -> std::io::Result<()> {
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
    let complex = (&a * &b) + ((!&a) * c);

    // Minimize
    let minimized = complex.minimize()?;
    
    Ok(())
}
```

## Truth Tables

### Building from Truth Tables

```rust
use espresso_logic::{Cover, CoverType, Minimizable};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::new(CoverType::F);
    
    // XOR function: a XOR b
    // Inputs: [a, b], Output: [f]
    cover.add_cube(&[Some(false), Some(false)], &[Some(false)]); // 00 -> 0
    cover.add_cube(&[Some(false), Some(true)],  &[Some(true)]);  // 01 -> 1
    cover.add_cube(&[Some(true),  Some(false)], &[Some(true)]);  // 10 -> 1
    cover.add_cube(&[Some(true),  Some(true)],  &[Some(false)]); // 11 -> 0
    
    cover = cover.minimize()?;
    println!("Minimized to {} cubes", cover.num_cubes());
    
    Ok(())
}
```

### Using Don't Cares

```rust
use espresso_logic::{Cover, CoverType, Minimizable};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::new(CoverType::F);
    
    // Use None for don't care values
    cover.add_cube(&[Some(true), None], &[Some(true)]);  // 1- -> 1
    cover.add_cube(&[None, Some(true)], &[Some(true)]);  // -1 -> 1
    
    cover = cover.minimize()?;
    
    Ok(())
}
```

## Multiple Outputs

### Named Outputs

```rust
use espresso_logic::{Cover, CoverType, BoolExpr, expr, Minimizable};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::new(CoverType::F);
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    
    // Add multiple outputs with names
    cover.add_expr(&expr!(a * b), "and_output")?;
    cover.add_expr(&expr!(a + c), "or_output")?;
    cover.add_expr(&expr!(a * b + b * c), "complex_output")?;
    
    // Minimize all together
    cover = cover.minimize()?;
    
    // Retrieve minimized expressions
    for (name, expr) in cover.to_exprs() {
        println!("{}: {}", name, expr);
    }
    
    Ok(())
}
```

## PLA Files

### Reading and Writing

```rust,no_run
use espresso_logic::{Cover, CoverType, Minimizable, PLAReader, PLAWriter};

fn main() -> std::io::Result<()> {
    // Read from file
    let mut cover = Cover::from_pla_file("input.pla")?;
    
    println!("Inputs: {}", cover.num_inputs());
    println!("Outputs: {}", cover.num_outputs());
    println!("Cubes before: {}", cover.num_cubes());
    
    // Minimize
    cover = cover.minimize()?;
    
    println!("Cubes after: {}", cover.num_cubes());
    
    // Write result
    cover.to_pla_file("output.pla", CoverType::F)?;
    
    Ok(())
}
```

### PLA File Format

Example PLA file:

```text
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
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() {
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
}
```

### Evaluation

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};
use std::collections::HashMap;
use std::sync::Arc;

fn main() {
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
}
```

### Collecting Variables

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() {
    let expr = expr!("a" * "b" + "c" * "d");

    // Get all variables (sorted)
    let vars = expr.collect_variables();
    println!("Variables: {:?}", vars);  // {"a", "b", "c", "d"}
}
```

## Binary Decision Diagrams (BDDs)

Binary Decision Diagrams provide canonical representation with efficient operations. 
Introduced in v3.1, BDDs are used internally for cover generation and are also available 
as a public API.

### Basic BDD Construction

```rust
use espresso_logic::{BoolExpr, Bdd};
use std::sync::Arc;

fn main() {
    // Create BDDs from constants
    let true_bdd = Bdd::constant(true);
    let false_bdd = Bdd::constant(false);
    
    // Create BDD from variable
    let a = Bdd::variable("a");
    
    // Convert expression to BDD
    let expr = BoolExpr::variable("a").and(&BoolExpr::variable("b"));
    let bdd = expr.to_bdd();
    
    // Or use the from_expr method
    let bdd2 = Bdd::from_expr(&expr);
    
    println!("BDD has {} nodes", bdd.node_count());
}
```

### BDD Operations

```rust
use espresso_logic::{BoolExpr, Bdd};
use std::sync::Arc;

fn main() {
    let a = Bdd::variable("a");
    let b = Bdd::variable("b");
    let c = Bdd::variable("c");
    
    // Logical operations
    let a_and_b = a.and(&b);
    let a_or_b = a.or(&b);
    let not_a = a.not();
    
    // Complex expression: (a AND b) OR (NOT a AND c)
    let complex = a.and(&b).or(&a.not().and(&c));
    
    println!("Complex BDD has {} nodes", complex.node_count());
    println!("Uses {} variables", complex.var_count());
}
```

### Converting Between BDD and BoolExpr

```rust
use espresso_logic::{BoolExpr, Bdd, Minimizable};

fn main() -> std::io::Result<()> {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    
    // Create expression
    let expr = a.and(&b).or(&b.and(&c));
    
    // Convert to BDD (efficient canonical representation)
    let bdd = expr.to_bdd();
    println!("Original expression as BDD: {} nodes", bdd.node_count());
    
    // Convert back to expression (DNF form)
    let expr2 = bdd.to_expr();
    println!("Converted back: {}", expr2);
    
    // Verify equivalence
    assert!(expr.equivalent_to(&expr2));
    
    Ok(())
}
```

### Equivalence Checking with BDDs

```rust
use espresso_logic::BoolExpr;

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    
    // Two equivalent expressions (commutative)
    let expr1 = a.and(&b);
    let expr2 = b.and(&a);
    
    // Convert to BDDs
    let bdd1 = expr1.to_bdd();
    let bdd2 = expr2.to_bdd();
    
    // BDDs are identical for equivalent expressions (canonical representation)
    assert_eq!(bdd1, bdd2);
    assert_eq!(bdd1.node_count(), bdd2.node_count());
    
    println!("Expressions are equivalent!");
}
```

### BDD Properties and Inspection

```rust
use espresso_logic::{BoolExpr, Bdd, Dnf};

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let expr = a.and(&b).or(&a.not());
    
    let bdd = expr.to_bdd();
    
    // Check BDD properties
    println!("Is terminal: {}", bdd.is_terminal());
    println!("Is true: {}", bdd.is_true());
    println!("Is false: {}", bdd.is_false());
    println!("Node count: {}", bdd.node_count());
    println!("Variable count: {}", bdd.var_count());
    
    // Extract cubes (paths to TRUE)
    let dnf = Dnf::from(&bdd);
    println!("Number of cubes: {}", dnf.len());
    for cube in dnf.cubes() {
        println!("  Cube: {:?}", cube);
    }
}
```

### BDD Automatic Optimization

```rust
use espresso_logic::{BoolExpr, Dnf};

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    
    // Consensus theorem: a*b + ~a*c + b*c
    // The b*c term is redundant
    let expr = a.and(&b).or(&a.not().and(&c)).or(&b.and(&c));
    
    println!("Original expression: {}", expr);
    
    // BDD automatically recognizes redundancy
    let bdd = expr.to_bdd();
    let dnf = Dnf::from(&bdd);
    
    println!("BDD has {} cubes (redundancy eliminated)", dnf.len());
    // Outputs: 2 cubes (b*c was redundant and eliminated)
    
    // Convert back to see simplified form
    let simplified = bdd.to_expr();
    println!("Simplified: {}", simplified);
}
```

### Using BDDs for Efficient Minimization

```rust
use espresso_logic::{BoolExpr, Minimizable};

fn main() -> std::io::Result<()> {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    
    // Redundant expression
    let expr = a.and(&b).or(&a.and(&b).and(&c));
    
    // Minimization workflow (v3.1):
    // 1. Expression → BDD (efficient canonical form)
    // 2. BDD → Cover cubes (optimized extraction)
    // 3. Cover → Minimized cover (Espresso algorithm)
    
    let minimized = expr.minimize()?;
    println!("Minimized: {}", minimized);  // Output: a * b
    
    Ok(())
}
```

## Low-Level API

### Direct Espresso Usage

```rust
use espresso_logic::espresso::{EspressoCover, CubeType};

fn main() -> std::io::Result<()> {
    // Build cover from cubes
    let cubes = vec![
        (vec![0, 1], vec![1]),  // 01 -> 1
        (vec![1, 0], vec![1]),  // 10 -> 1
    ];
    let f = EspressoCover::from_cubes(cubes, 2, 1)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    
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

fn main() -> std::io::Result<()> {
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
use espresso_logic::{BoolExpr, expr, Minimizable};
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

```rust,no_run
use espresso_logic::{Cover, Minimizable, PLAReader};
use std::thread;

fn main() -> std::io::Result<()> {
    let files = vec!["a.pla", "b.pla", "c.pla", "d.pla"];
    
    let handles: Vec<_> = files.into_iter().map(|file| {
        thread::spawn(move || -> std::io::Result<usize> {
            let mut cover = Cover::from_pla_file(file)?;
            cover = cover.minimize()?;
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

```rust,no_run
use espresso_logic::{Cover, Minimizable, PLAReader};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::from_pla_file("input.pla")?;
    
    // Get cubes before minimization
    let before_count = cover.num_cubes();
    println!("Before: {} cubes", before_count);
    
    cover = cover.minimize()?;
    
    // Get cubes after minimization
    let after_count = cover.num_cubes();
    println!("After: {} cubes", after_count);
    
    // Inspect individual cubes
    for cube in cover.cubes() {
        println!("Cube: {:?}", cube);
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

