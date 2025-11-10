# Boolean Expression API

This document provides comprehensive documentation for the boolean expression API in espresso-logic.

## Overview

The boolean expression API provides a high-level, intuitive interface for working with boolean functions. Instead of manually constructing truth tables or working with low-level cubes, you can:

- **Build expressions programmatically** using a fluent monadic interface
- **Parse expressions from strings** using standard boolean notation
- **Use operator overloading** with `*`, `+`, and `!`
- **Use the `expr!` macro** for clean, readable syntax
- **Compose expressions** - elegantly combine parsed or existing expressions
- **Minimize directly** with `.minimize()` method

## Quick Start

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() -> std::io::Result<()> {
    // Create variables
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Build expression with clean syntax
    let xor = expr!(a * !b + !a * b);

    // Minimize
    let minimized = xor.minimize()?;
    println!("{}", minimized);
    
    Ok(())
}
```

## Creating Boolean Expressions

### Method 1: Variable Creation

```rust
use espresso_logic::BoolExpr;

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
}
```

Variable names can be:
- Any valid Rust identifier
- Multi-character (e.g., `"input_a"`, `"clk_enable"`)
- Case-sensitive (`"A"` and `"a"` are different)

### Method 2: Constants

```rust
use espresso_logic::BoolExpr;

fn main() {
    let t = BoolExpr::constant(true);
    let f = BoolExpr::constant(false);
}
```

### Method 3: Parsing Strings

```rust
use espresso_logic::BoolExpr;

fn main() -> std::io::Result<()> {
    let expr = BoolExpr::parse("(a + b) * (c + d)")?;
    Ok(())
}
```

## Building Expressions

### When to Use Which API

**Use `expr!` macro** - Recommended for compile-time expression construction and composition:
- Clean, readable syntax matching mathematical notation
- Perfect for simple and complex expressions alike
- No reference syntax needed (`&`)
- Works with string literals and any `BoolExpr` values (parsed, created, minimized, etc.)
- **Ideal for composing expressions** - combine user-defined functions elegantly

**Use `BoolExpr::parse()`** - For runtime user input:
- Parse expressions from strings at runtime
- User input, config files, CLI arguments, etc.
- Standard boolean algebra notation

**Use operator overloading or monadic interface** - For special cases:
- Building expressions in loops or conditional logic
- When structure depends on runtime conditions
- Advanced programmatic construction

### `expr!` Macro (Recommended)

The `expr!` macro is a procedural macro that provides the cleanest syntax. At compile time, the macro expands to use the monadic interface (`.and()`, `.or()`, `.not()` methods), so there is zero runtime overhead.

#### Using String Literals

No variable declarations needed - variables are created automatically:

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() {
    // Simple expressions
    let and_expr = expr!("a" * "b");
    let or_expr = expr!("a" + "b");
    let not_expr = expr!(!"a");

    // XOR - no variable declarations!
    let xor = expr!("a" * "b" + !"a" * !"b");

    // Complex nested
    let complex = expr!(("a" + "b") * ("c" + "d"));

    // Majority function
    let majority = expr!("a" * "b" + "b" * "c" + "a" * "c");
}
```

#### Combining Expressions

You can create expressions using any method (`BoolExpr::variable()`, `BoolExpr::parse()`, etc.) and combine them with `expr!`:

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() -> std::io::Result<()> {
    // Create expressions using different methods
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let func_a = BoolExpr::parse("input1 * input2")?;
    let func_b = BoolExpr::parse("input3 + input4")?;
    
    // Combine them all with expr! - clean and readable
    let xor = expr!(a * !b + !a * b);
    let combined = expr!(func_a + func_b);
    
    // Mix created and parsed expressions
    let selector = BoolExpr::variable("mode");
    let output = expr!(selector * func_a + !selector * func_b);
    
    println!("XOR: {}", xor);
    println!("Combined: {}", combined);
    println!("Output: {}", output);
    
    Ok(())
}
```

#### Complete Example: Mixing Expressions with String Literals

Combine expressions (from parsing, minimization, etc.) with string literals seamlessly:

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() -> std::io::Result<()> {
    // Parse complex expressions from user input or config files
    let f1 = BoolExpr::parse("(a + b) * (c + d)")?;
    let f2 = BoolExpr::parse("x * y + z")?;
    
    // Mix parsed expressions with string literals using expr!
    let out = expr!(!"rst" * ("en" * f1 + !"en" * f2) + "rst" * "def");
    
    println!("Output: {}", out);
    
    // Another example: compose minimized sub-expressions (efficient!)
    let expr = BoolExpr::parse("p * q + p * r")?;
    let min = expr.minimize()?;  // Already in minimal DNF form
    let final_expr = expr!(min * "s" + !"t");
    
    println!("Final: {}", final_expr);
    
    Ok(())
}
```

**Key insight:** Everything is just a `BoolExpr` - whether created via parsing, string literals in `expr!()`, `BoolExpr::variable()`, or any other method. All `BoolExpr` values can be freely mixed and composed.

**Key use cases for composition:**
- Combining user-defined functions from configuration files
- Building complex logic from simpler parsed components
- Creating conditional expressions based on runtime parameters
- Composing minimized sub-expressions into larger systems (efficient - already in minimal DNF form)

**Advantages of the `expr!` macro:**
- No explicit `&` references needed
- Clean, readable syntax matching mathematical notation
- Flexible - works with string literals and any `BoolExpr` values
- Automatic operator precedence
- Perfect for expressing common patterns
- Ideal for expression composition

### Parser (For Runtime User Input)

Use `BoolExpr::parse()` to parse expressions from strings at runtime. The parser supports standard boolean algebra notation:

#### Operators

| Operator | Meaning | Precedence | Example |
|----------|---------|------------|---------|
| `( )` | Parentheses | Highest (0) | `(a + b)` |
| `~` or `!` | NOT | High (1) | `~a`, `!b` |
| `*` or `&` | AND | Medium (2) | `a * b`, `a & b` |
| `+` or `\|` | OR | Lowest (3) | `a + b`, `a \| b` |

#### Precedence Rules

Operators follow standard boolean algebra precedence:

1. **Parentheses** (highest) - force evaluation order
2. **NOT** - evaluated first (after parentheses)
3. **AND** - evaluated second
4. **OR** (lowest) - evaluated last

```rust
use espresso_logic::BoolExpr;

fn main() -> std::io::Result<()> {
    // These are equivalent:
    let expr1 = BoolExpr::parse("~a * b + c")?;
    let expr2 = BoolExpr::parse("((~a) * b) + c")?;

    // NOT binds tighter than AND
    let expr3 = BoolExpr::parse("a * ~b")?;  // a AND (NOT b)

    // AND binds tighter than OR
    let expr4 = BoolExpr::parse("a * b + c")?;  // (a AND b) OR c
    
    Ok(())
}
```

#### Parentheses

Use parentheses to override precedence:

```rust
use espresso_logic::BoolExpr;

fn main() -> std::io::Result<()> {
    let expr = BoolExpr::parse("(a + b) * c")?;  // (a OR b) AND c
    let expr2 = BoolExpr::parse("~(a * b)")?;    // NOT (a AND b)
    Ok(())
}
```

#### Constants

The parser recognizes boolean constants:

```rust
use espresso_logic::BoolExpr;

fn main() -> std::io::Result<()> {
    let expr1 = BoolExpr::parse("a * 1")?;      // a AND true = a
    let expr2 = BoolExpr::parse("b + 0")?;      // b OR false = b
    let expr3 = BoolExpr::parse("true * a")?;   // true AND a = a
    let expr4 = BoolExpr::parse("false + b")?;  // false OR b = b
    Ok(())
}
```

#### Variable Names

Variable names must:
- Start with a letter or underscore
- Contain only alphanumeric characters and underscores
- Be case-sensitive

```rust
use espresso_logic::BoolExpr;

fn main() -> std::io::Result<()> {
    // Valid variable names:
    let expr1 = BoolExpr::parse("x * y")?;
    let expr2 = BoolExpr::parse("input_a * input_b")?;
    let expr3 = BoolExpr::parse("clk_enable + reset_n")?;
    let expr4 = BoolExpr::parse("A * B")?;  // Different from a * b

    // Whitespace is ignored
    let expr5 = BoolExpr::parse("a*b+c")?;
    let expr6 = BoolExpr::parse("a * b + c")?;  // Same as above
    
    Ok(())
}
```

### Operator Overloading

Boolean expressions support Rust's standard operators as an alternative to the `expr!` macro:

```rust
use espresso_logic::BoolExpr;

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let and_expr = &a * &b;        // AND
    let or_expr = &a + &b;         // OR
    let not_expr = !&a;            // NOT

    // Complex: XNOR
    let xnor = &a * &b + &(!&a) * &(!&b);
}
```

**Note:** Requires `&` references due to Rust's ownership rules. The `expr!` macro is preferred as it avoids this requirement.

### Monadic Interface

The monadic interface provides explicit method calls for building expressions. The `expr!` macro expands to this interface:

```rust
use espresso_logic::BoolExpr;

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // AND
    let and_expr = a.and(&b);

    // OR
    let or_expr = a.or(&b);

    // NOT
    let not_expr = a.not();

    // Complex expression: (a * b) + (~a * c)
    let complex = a.and(&b).or(&a.not().and(&c));
}
```

**Actual macro expansions** (verified with `cargo expand`):

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // let _expr1 = expr!(a * b);
    // Expands to:
    let _expr1 = (&(a)).and(&(b));

    // let _expr2 = expr!(a + b);
    // Expands to:
    let _expr2 = (&(a)).or(&(b));

    // let _expr3 = expr!(!a);
    // Expands to:
    let _expr3 = (&(a)).not();

    // let _expr4 = expr!(a * b + !c);
    // Expands to:
    let _expr4 = (&((&(a)).and(&(b)))).or(&((&(c)).not()));

    // let _expr5 = expr!("x" * "y" + !"z");
    // Expands to:
    let _expr5 = (&((&(BoolExpr::variable("x"))).and(&(BoolExpr::variable("y")))))
        .or(&((&(BoolExpr::variable("z"))).not()));
}
```

The macro generates clean calls to the monadic interface, using references for all arguments. The monadic methods (`.and()`, `.or()`, `.not()`) all take `&self` and handle any necessary cloning internally - the macro itself does not clone. String literals are automatically converted to `BoolExpr::variable()` calls.

**When to use:**
- Building expressions in loops or conditional logic
- When structure depends on runtime conditions
- Advanced programmatic construction

**Example - Dynamic construction:**

```rust
use espresso_logic::BoolExpr;

fn main() {
    let mut expr = BoolExpr::variable("a");
    
    // Build expression dynamically
    for var_name in ["b", "c", "d"] {
        expr = expr.and(&BoolExpr::variable(var_name));
    }
    
    // Results in: a * b * c * d
    println!("{}", expr);
}
```

## Minimization

### Direct Minimization (Heuristic)

The simplest way to minimize an expression using the fast heuristic algorithm:

```rust
use espresso_logic::{BoolExpr, Minimizable};

fn main() -> std::io::Result<()> {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Redundant expression: a*b + a*b*c
    let expr = a.and(&b).or(&a.and(&b).and(&c));

    // Minimize directly using heuristic algorithm (fast)
    let minimized = expr.minimize()?;

    println!("{}", minimized);  // Output: (a * b)
    
    Ok(())
}
```

### Exact Minimization (Guaranteed Minimal)

For provably minimal results, use `minimize_exact()`:

```rust
use espresso_logic::{BoolExpr, Minimizable};

fn main() -> std::io::Result<()> {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Redundant expression
    let expr = a.and(&b).or(&a.and(&b).and(&c));

    // Exact minimization - guaranteed minimal result
    let minimized = expr.minimize_exact()?;

    println!("{}", minimized);  // Guaranteed to be minimal: (a * b)
    
    Ok(())
}
```

**When to use each:**

- **`minimize()`**: Fast heuristic, near-optimal results (~99% optimal in practice)
  - Best for: Large expressions, production use, when speed matters
  - Time complexity: Near-linear in practice

- **`minimize_exact()`**: Slower but guaranteed minimal results
  - Best for: Equivalency checking, small expressions, when optimality is critical
  - Time complexity: Exponential worst case, but polynomial for many practical cases

### Using Cover for More Control

For more control over the minimization process:

```rust
use espresso_logic::{BoolExpr, Cover, CoverType, Minimizable};

fn main() -> std::io::Result<()> {
    let expr = BoolExpr::parse("a * b + a * b * c")?;
    
    // Create cover and add expression
    let mut cover = Cover::new(CoverType::F);
    cover.add_expr(&expr, "output")?;
    
    // Inspect before minimization
    println!("Input variables: {:?}", cover.input_labels());
    println!("Inputs: {}", cover.num_inputs());
    println!("Outputs: {}", cover.num_outputs());
    println!("Cubes before: {}", cover.num_cubes());
    
    // Minimize
    cover = cover.minimize()?;
    
    println!("Cubes after: {}", cover.num_cubes());
    
    // Convert back to expression
    let minimized = cover.to_expr("output")?;
    println!("Result: {}", minimized);
    
    Ok(())
}
```

## Common Patterns

### XOR (Exclusive OR)

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() -> std::io::Result<()> {
    // Method API
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let xor1 = a.and(&b.not()).or(&a.not().and(&b));

    // expr! macro (using strings - cleanest)
    let xor2 = expr!("a" * !"b" + !"a" * "b");

    // Parser
    let xor3 = BoolExpr::parse("a * ~b + ~a * b")?;
    
    Ok(())
}
```

### XNOR (Equivalence)

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() -> std::io::Result<()> {
    // Method API
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let xnor1 = a.and(&b).or(&a.not().and(&b.not()));

    // expr! macro (using strings - cleanest)
    let xnor2 = expr!("a" * "b" + !"a" * !"b");

    // Parser
    let xnor3 = BoolExpr::parse("a * b + ~a * ~b")?;
    
    Ok(())
}
```

### Majority Function (3 inputs)

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() -> std::io::Result<()> {
    // expr! macro (clearest - using strings)
    let majority1 = expr!("a" * "b" + "b" * "c" + "a" * "c");

    // Parser
    let majority2 = BoolExpr::parse("a * b + b * c + a * c")?;

    // Method API
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let majority3 = a.and(&b)
        .or(&b.and(&c))
        .or(&a.and(&c));
    
    Ok(())
}
```

### De Morgan's Laws

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() {
    // ~(a * b) = ~a + ~b (using string notation)
    let expr1 = expr!(!("a" * "b"));
    let expr2 = expr!(!"a" + !"b");

    // ~(a + b) = ~a * ~b
    let expr3 = expr!(!("a" + "b"));
    let expr4 = expr!(!"a" * !"b");
}
```

## Working with BDDs

Binary Decision Diagrams (BDDs) provide a canonical representation of boolean functions with
efficient operations. In version 3.1, BDDs are used internally for efficient cover generation
from boolean expressions before minimization by Espresso.

### BDD Role in Minimization

When you minimize a `BoolExpr`, the library:
1. Converts the expression to a `Bdd` (canonical representation, automatic optimizations)
2. Extracts cubes from the BDD to create a `Cover`
3. Minimizes the cover using Espresso's algorithm (heuristic or exact)

The BDD step enables efficient cover generation with automatic redundancy elimination.

### Direct BDD Usage

BDDs are also available as a public API for advanced use cases:

```rust
use espresso_logic::{BoolExpr, Bdd};

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    
    // Build expression
    let expr = a.and(&b).or(&b.and(&c));
    
    // Convert to BDD
    let bdd = expr.to_bdd();
    // Or use: let bdd = Bdd::from_expr(&expr);
    
    // Inspect BDD properties
    println!("BDD nodes: {}", bdd.node_count());
    println!("Variables: {}", bdd.var_count());
    
    // Perform operations directly on BDDs
    let d = BoolExpr::variable("d");
    let bdd_d = d.to_bdd();
    let combined = bdd.and(&bdd_d);
    
    // Convert back to expression
    let result_expr = combined.to_expr();
    println!("Result: {}", result_expr);
}
```

### BDD Advantages

BDDs automatically optimize expressions during construction:

```rust
use espresso_logic::{BoolExpr, Dnf};

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    
    // Consensus theorem: a*b + ~a*c + b*c
    // The b*c term is redundant
    let expr = a.and(&b).or(&a.not().and(&c)).or(&b.and(&c));
    
    // BDD automatically recognizes redundancy
    let bdd = expr.to_bdd();
    let dnf = Dnf::from(&bdd);
    
    println!("Cubes: {}", dnf.len());  // Outputs: 2 (b*c eliminated)
}
```

### When to Use BDDs

Use BDDs directly when you need:

- **Canonical representation**: Compare expressions for equivalence
- **Efficient operations**: Build complex expressions incrementally
- **Size inspection**: Check representation size before further operations
- **Optimization analysis**: Understand how expressions simplify

```rust
use espresso_logic::{BoolExpr, Bdd};

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    
    // Build two equivalent expressions
    let expr1 = a.and(&b);
    let expr2 = b.and(&a);  // Commutative
    
    // Convert to BDDs
    let bdd1 = expr1.to_bdd();
    let bdd2 = expr2.to_bdd();
    
    // BDDs are identical for equivalent expressions
    assert_eq!(bdd1.node_count(), bdd2.node_count());
    
    // Can perform operations efficiently
    let result = bdd1.or(&bdd2);
    println!("Result nodes: {}", result.node_count());
}
```

## Working with Cubes

### Iterating Over Cubes

```rust
use espresso_logic::{BoolExpr, Cover, CoverType, Minimizable};

fn main() -> std::io::Result<()> {
    let expr = BoolExpr::parse("a * b + ~a * c")?;
    let mut cover = Cover::new(CoverType::F);
    cover.add_expr(&expr, "out")?;
    
    for (i, (inputs, outputs)) in cover.cubes_iter().enumerate() {
        println!("Cube {}: inputs={:?}, outputs={:?}", i, inputs, outputs);
    }
    
    Ok(())
}
```

### Converting to PLA Format

```rust
use espresso_logic::{BoolExpr, Cover, CoverType, PLAWriter};

fn main() -> std::io::Result<()> {
    let expr = BoolExpr::parse("a * b + c")?;
    let mut cover = Cover::new(CoverType::F);
    cover.add_expr(&expr, "output")?;
    
    // Export to PLA string
    let pla_string = cover.to_pla_string(CoverType::F)?;
    println!("{}", pla_string);
    
    // Or write to file
    cover.to_pla_file("output.pla", CoverType::F)?;
    
    Ok(())
}
```

## Variable Ordering

Variables are automatically sorted alphabetically:

```rust
use espresso_logic::*;

fn main() -> std::io::Result<()> {
    let c = BoolExpr::variable("c");
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let expr = expr!(c * a * b);
    let mut cover = Cover::new(CoverType::F);
    cover.add_expr(&expr, "out")?;

    println!("{:?}", cover.input_labels());  // ["a", "b", "c"] (sorted)
    
    Ok(())
}
```

This ensures consistent ordering in truth tables and PLA files.

## Error Handling

### Parsing Errors

```rust
use espresso_logic::BoolExpr;

fn main() {
    match BoolExpr::parse("a * * b") {
        Ok(expr) => println!("Parsed: {}", expr),
        Err(e) => println!("Parse error: {}", e),
    }
}
```

Common parse errors:
- Syntax errors: `"a * * b"`, `"a +"`, `"(a * b"`
- Invalid tokens: `"a & b"`, `"a | b"`, `"a ^ b"`
- Empty input: `""`

### Minimization Errors

```rust
use espresso_logic::{BoolExpr, Minimizable};

fn main() -> std::io::Result<()> {
    let expr = BoolExpr::parse("a * b")?;
    
    match expr.minimize() {
        Ok(minimized) => println!("Success: {}", minimized),
        Err(e) => eprintln!("Minimization failed: {}", e),
    }
    
    Ok(())
}
```

## Performance Considerations

### Expression Construction

- Variable creation: O(1)
- Expression building: O(1) per operation (uses Arc for sharing)
- Parsing: O(n) where n is the input length

### BDD-Based Cover Generation (v3.1+)

**Binary Decision Diagrams (BDDs) for Cover Generation:**
- As of version 3.1, expressions are converted to BDDs before cube extraction
- BDDs provide canonical representation with automatic optimization
- Hash consing ensures identical subexpressions are shared
- Operations (AND, OR, NOT) are memoized for efficiency
- **Note:** The minimization itself is still performed by Espresso, not the BDD

**Performance characteristics:**
- BDD construction: Polynomial for most practical expressions (vs exponential DNF)
- Canonical representation: Equivalent expressions produce identical BDDs
- Automatic simplification: Redundant terms eliminated during BDD construction
- Memory efficient: Structural sharing via hash consing
- Global singleton manager: All BDDs share one manager (thread-safe via Mutex)

**Minimization workflow:**
1. Expression → BDD (fast, polynomial)
2. BDD → Cover cubes (extraction, linear in BDD size)
3. Cover → Minimized cover (Espresso algorithm, dominant cost)

**Performance improvements (v3.0 → v3.1):**
- Faster equivalence checking via BDD canonical representation
- More efficient cover generation from complex expressions
- Reduced redundancy in generated covers (better Espresso input)

**Cost Amortization Strategy:**
- **Minimize partial expressions early** - this amortizes the DNF conversion cost
- Minimized expressions are already in DNF form with the minimal number of clauses
- **Composing DNF expressions is cheap:**
  - OR operations: simply concatenate the clauses (union of terms)
  - AND operations: distribute, but with fewer clauses the cross-product is smaller
  - **NOT operations: expensive** - require De Morgan's law propagation to convert back to DNF
- When expressions have fewer clauses (due to minimization), combining them is much more efficient
- The DNF extraction for the minimization of a composed expression is significantly cheaper when sub-expressions are already in minimal DNF form, compared to converting a large complex expression from scratch
- **For negation:** Prefer to negate first, then minimize, then compose - this avoids expensive De Morgan propagation later

This approach is especially beneficial when working with large or complex sub-expressions that will be composed together.

### Memory

- Expressions use Arc for structural sharing
- Very memory efficient for large expressions
- Variables are deduplicated automatically

## BoolExpr API Methods

### Display and Formatting

Boolean expressions are displayed with minimal parentheses based on operator precedence:

```rust
use espresso_logic::*;

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Simple operations - no unnecessary parentheses
    println!("{}", expr!(a * b));        // Output: a * b
    println!("{}", expr!(a + b));        // Output: a + b
    println!("{}", expr!(a * b + c));    // Output: a * b + c

    // Parentheses only when needed for precedence
    println!("{}", expr!((a + b) * c));  // Output: (a + b) * c
    println!("{}", expr!(!(a * b)));     // Output: ~(a * b)

    // Clean formatting for complex expressions
    let xor = expr!(a * b + !a * !b);
    println!("{}", xor);  // Output: a * b + ~a * ~b (not ((a * b) + (~a * ~b)))
}
```

**Formatting rules:**
- Variables and constants: no parentheses
- NOT chains: no parentheses (e.g., `~~a`)
- AND chains: no parentheses (e.g., `a * b * c`)
- OR chains: no parentheses (e.g., `a + b + c`)
- OR inside AND: parentheses required (e.g., `(a + b) * c`)
- Compound expressions in NOT: parentheses required (e.g., `~(a * b)`)

### Semantic Equality

Check if two expressions are logically equivalent (produce same outputs):

```rust
use espresso_logic::*;

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Different structures, same logic
    let expr1 = expr!(a * b);
    let expr2 = expr!(b * a);  // Commutative

    // Structural equality (tree comparison)
    assert_ne!(expr1, expr2);  // Different tree structure

    // Logical equivalence (efficient exact minimization check)
    assert!(expr1.equivalent_to(&expr2));  // Same logic!

    // Test double negation
    let expr3 = a.clone();
    let expr4 = expr!(!!a);
    assert!(expr3.equivalent_to(&expr4));

    // Non-equivalent expressions
    let and_expr = expr!(a * b);
    let or_expr = expr!(a + b);
    assert!(!and_expr.equivalent_to(&or_expr));
}
```

**Performance Note:**

The `equivalent_to()` method uses a two-phase BDD-based approach (v3.1+):

1. **Fast BDD equality check**: Convert both expressions to BDDs and compare. BDDs use canonical 
   representation, so equal BDDs guarantee equivalence. This is very fast (O(e) where e is expression size).
2. **Exact minimization fallback**: If BDDs differ, use exact minimization for thorough verification.

Previous approach:
- **v3.0**: Exhaustive truth table evaluation - O(2^n) where n = number of variables
  - Generated all 2^n possible variable assignments
  - Evaluated both expressions for each assignment
  - Completely impractical for expressions with many variables

This makes equivalency checking **dramatically faster** for expressions with many variables:
- 10 variables: 1,024x faster
- 20 variables: 1,048,576x faster  
- 30 variables: Previously impossible, now feasible

The method combines both expressions into a single cover with two outputs, minimizes exactly once, and checks if all cubes have identical output patterns.

### Evaluation

Evaluate expressions with specific variable assignments:

```rust
use espresso_logic::*;
use std::collections::HashMap;
use std::sync::Arc;

let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");
let expr = expr!(a * b + !a);

// Create variable assignments
let mut assignment = HashMap::new();
assignment.insert(Arc::from("a"), true);
assignment.insert(Arc::from("b"), false);

// Evaluate: a * b + !a = true * false + !true = false + false = false
let result = expr.evaluate(&assignment);
println!("Result: {}", result);  // false

// Try different assignments: a * b + !a = false * true + !false = false + true = true
assignment.insert(Arc::from("a"), false);
assignment.insert(Arc::from("b"), true);
let result2 = expr.evaluate(&assignment);
println!("Result: {}", result2);  // true
```

### Variable Collection

Get all variables used in an expression:

```rust
use espresso_logic::{BoolExpr, expr, Minimizable};

fn main() {
    let expr = expr!("x" * "y" + "z");

    let vars = expr.collect_variables();
    // Returns BTreeSet<Arc<str>> in alphabetical order
    for var in vars {
        println!("Variable: {}", var);
    }
    // Output:
    // Variable: x
    // Variable: y
    // Variable: z
}
```

## Best Practices

### 1. Choose the Right API

```rust
use espresso_logic::*;

fn main() -> std::io::Result<()> {
    // For runtime expressions from user input: use parser
    let user_input = "a * b + c";  // From user, config file, etc.
    let expr1 = BoolExpr::parse(user_input)?;

    // For compile-time expressions: use expr! macro (preferred)
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr2 = expr!(a * b + c);

    // For complex nested logic: still use expr! macro
    let complex_subexpr = expr!("x" * "y");
    let expr3 = expr!(a * b + complex_subexpr);

    // For composing parsed expressions: use expr! macro
    let func1 = BoolExpr::parse("x * y")?;
    let func2 = BoolExpr::parse("z + w")?;
    let composed = expr!(func1 + !func2);  // Clean and idiomatic!

    // Monadic interface is available for special cases (dynamic construction, loops, etc.)
    let mut dynamic_expr = a.clone();
    for var in ["b", "c", "d"] {
        dynamic_expr = dynamic_expr.and(&BoolExpr::variable(var));
    }
    
    Ok(())
}
```

### 2. Reuse Variables

```rust
use espresso_logic::*;

fn main() -> std::io::Result<()> {
    // Good: reuse variable objects
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr1 = expr!(a * b);
    let expr2 = expr!(a + c);

    // Works but less efficient: create variables multiple times
    let expr3 = BoolExpr::parse("a * b")?;
    let expr4 = BoolExpr::parse("a + c")?;
    
    Ok(())
}
```

### 3. Minimize Early

Minimizing partial expressions early amortizes the DNF conversion cost:

```rust
use espresso_logic::*;

fn main() -> std::io::Result<()> {
    // Good: minimize intermediate results
    let large_expr = expr!("a" * "b" + "c" * "d" + "e" * "f");
    let other_term = expr!("x" * "y");
    
    let intermediate = large_expr.minimize()?;  // Already in minimal DNF form
    let final_expr = expr!(intermediate + other_term).minimize()?;

    // Less efficient: combine then minimize
    // Must convert the entire combined expression to DNF
    let large_expr2 = expr!("a" * "b" + "c" * "d" + "e" * "f");
    let other_term2 = expr!("x" * "y");
    let final_expr2 = large_expr2.or(&other_term2).minimize()?;
    
    // Good: negate, minimize, then compose (avoids De Morgan propagation)
    let expr = BoolExpr::parse("a * b + c * d")?;
    let negated_min = expr.not().minimize()?;  // De Morgan applied once
    let composed = expr!(negated_min * "e").minimize()?;
    
    // Less efficient: compose then negate (De Morgan on larger expression)
    let expr2 = BoolExpr::parse("a * b + c * d")?;
    let composed2 = expr!(expr2 * "e").not().minimize()?;
    
    Ok(())
}
```

**Why this works:** 
- For OR/AND: The minimized `intermediate` is already in DNF form with minimal clauses. When composing with OR, the DNF forms are simply concatenated. When composing with AND, fewer clauses means smaller cross-product. The final DNF extraction is cheap because `intermediate` has few clauses.
- For NOT: Negation requires De Morgan's law propagation to convert back to DNF. By negating and minimizing first, you apply De Morgan to a smaller expression, then compose the minimized result (which is in DNF). This is much cheaper than composing first and then negating a larger expression.

### 4. Use Type System

```rust
use espresso_logic::*;

fn main() -> std::io::Result<()> {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    
    // The type system prevents mistakes
    let expr: BoolExpr = expr!(a * b);  // Type-safe
    let mut cover: Cover = Cover::new(CoverType::F);  // Clear types
    cover.add_expr(&expr, "output")?;  // Explicit conversion
    
    Ok(())
}
```

## Examples

See `examples/boolean_expressions.rs` for comprehensive examples including:

1. Programmatic construction with expr! macro
2. Parsing from strings
3. Complex expressions with negation
4. Minimization examples
5. XNOR function
6. Three-variable majority function
7. PLA format export
8. Expressions with constants
9. De Morgan's laws
10. Equivalent expressions
11. Cube iteration

## Troubleshooting

### Multiple operator notations supported

✅ Both algebraic and bitwise-style operators are supported:
```rust
use espresso_logic::*;

// Both notations work for AND
let _expr1 = BoolExpr::parse("a * b").unwrap();  // Algebraic notation
let _expr2 = BoolExpr::parse("a & b").unwrap();  // Bitwise-style notation

// Both notations work for OR
let _expr3 = BoolExpr::parse("a + b").unwrap();  // Algebraic notation
let _expr4 = BoolExpr::parse("a | b").unwrap();  // Bitwise-style notation

// You can even mix them
let _expr5 = BoolExpr::parse("a & b | c * d").unwrap();
```

### Moving variables multiple times

❌ Wrong: Reusing moved variables
```rust,compile_fail
use espresso_logic::*;

let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");
// XOR: trying to reuse 'a' and 'b' after moving them
let xor = a * b + !a * !b;  // Error: a and b moved in first term
```

✅ Correct: Use references or expr! macro
```rust
use espresso_logic::*;

let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");

// Option 1: With references - can reuse variables
let xor1 = &a * &b + &(!&a) * &(!&b);

// Option 2: Use expr! macro (cleanest, no references needed)
let xor2 = expr!(a * b + !a * !b);

// Option 3: Clone variables if you need to move them
let a2 = BoolExpr::variable("a");
let b2 = BoolExpr::variable("b");
let xor3 = a2.clone() * b2.clone() + !a2 * !b2;
```

### Expression doesn't minimize as expected

```rust
use espresso_logic::*;

fn main() -> std::io::Result<()> {
    // Check the DNF conversion
    let expr = BoolExpr::parse("(a + b) * (c + d)")?;
    let mut cover = Cover::new(CoverType::F);
    cover.add_expr(&expr, "out")?;

    println!("Cubes before: {}", cover.num_cubes());  // Check size
    cover = cover.minimize()?;
    println!("Cubes after: {}", cover.num_cubes());

    // View the result
    let result = cover.to_expr("out")?;
    println!("Result: {}", result);
    
    Ok(())
}
```

## See Also

- [API Documentation](API.md) - Complete API reference
- [Thread-Local Implementation](THREAD_LOCAL_IMPLEMENTATION.md) - Thread safety details
- [PLA Format](PLA_FORMAT.md) - PLA file format specification
- [Examples](../examples/) - Working code examples

