# Boolean Expression API

This document provides comprehensive documentation for the boolean expression API in espresso-logic.

## Overview

The boolean expression API provides a high-level, intuitive interface for working with boolean functions. Instead of manually constructing truth tables or working with low-level cubes, you can:

- **Build expressions programmatically** using a fluent method API
- **Parse expressions from strings** using standard boolean notation
- **Use operator overloading** with `*`, `+`, and `!`
- **Use the `expr!` macro** for clean, readable syntax
- **Minimize directly** with `.minimize()` method

## Quick Start

```rust
use espresso_logic::{BoolExpr, expr};

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

### Method API (Recommended for Complex Logic)

The method API uses explicit method calls:

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

**Advantages:**
- Explicit and clear
- No reference syntax needed
- Easy to chain
- Good for complex nested expressions

### Operator Overloading

Boolean expressions support Rust's standard operators:

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

**Note:** Requires `&` references due to Rust's ownership rules.

### `expr!` Macro (Recommended for Readability)

The `expr!` macro is a procedural macro that provides the cleanest syntax with three usage styles:

#### Style 1: String Literals (Most Concise)

No variable declarations needed - variables are created automatically:

```rust
use espresso_logic::{BoolExpr, expr};

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

#### Style 2: Existing BoolExpr Variables

Use pre-defined variables for more control:

```rust
use espresso_logic::{BoolExpr, expr};

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Simple operations
    let and_expr = expr!(a * b);
    let or_expr = expr!(a + b);
    let not_expr = expr!(!a);

    // XOR
    let xor = expr!(a * !b + !a * b);

    // XNOR
    let xnor = expr!(a * b + !a * !b);

    // With parentheses
    let complex = expr!((a + b) * c);
}
```

#### Style 3: Mixed (Best of Both)

Combine existing variables with string literals:

```rust
use espresso_logic::{BoolExpr, expr};

fn main() {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    // Mix both styles
    let expr_mixed = expr!(a * "temp" + b * "enable");

    // Compose sub-expressions
    let sub1 = expr!(a * b);
    let sub2 = expr!("c" + "d");
    let combined = expr!(sub1 + sub2);
}
```

**Advantages:**
- No explicit `&` references
- Clean, readable syntax
- Matches mathematical notation
- Three flexible usage styles
- Automatic operator precedence
- Perfect for expressing common patterns

## Parsing Syntax

The parser supports standard boolean algebra notation:

### Operators

| Operator | Meaning | Precedence | Example |
|----------|---------|------------|---------|
| `~` or `!` | NOT | Highest (1) | `~a`, `!b` |
| `*` | AND | Medium (2) | `a * b` |
| `+` | OR | Lowest (3) | `a + b` |

### Precedence Rules

Operators follow standard boolean algebra precedence:

1. **NOT** (highest) - evaluated first
2. **AND** - evaluated second
3. **OR** (lowest) - evaluated last

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

### Parentheses

Use parentheses to override precedence:

```rust
use espresso_logic::BoolExpr;

fn main() -> std::io::Result<()> {
    let expr = BoolExpr::parse("(a + b) * c")?;  // (a OR b) AND c
    let expr2 = BoolExpr::parse("~(a * b)")?;    // NOT (a AND b)
    Ok(())
}
```

### Constants

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

### Variable Names

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

## Minimization

### Direct Minimization

The simplest way to minimize an expression:

```rust
use espresso_logic::BoolExpr;

fn main() -> std::io::Result<()> {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Redundant expression: a*b + a*b*c
    let expr = a.and(&b).or(&a.and(&b).and(&c));

    // Minimize directly
    let minimized = expr.minimize()?;

    println!("{}", minimized);  // Output: (a * b)
    
    Ok(())
}
```

### Using Cover for More Control

For more control over the minimization process:

```rust
use espresso_logic::{BoolExpr, Cover, CoverType};

fn main() -> std::io::Result<()> {
    let expr = BoolExpr::parse("a * b + a * b * c")?;
    
    // Create cover and add expression
    let mut cover = Cover::new(CoverType::F);
    cover.add_expr(expr, "output")?;
    
    // Inspect before minimization
    println!("Input variables: {:?}", cover.input_labels());
    println!("Inputs: {}", cover.num_inputs());
    println!("Outputs: {}", cover.num_outputs());
    println!("Cubes before: {}", cover.num_cubes());
    
    // Minimize
    cover.minimize()?;
    
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
use espresso_logic::{BoolExpr, expr};

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
use espresso_logic::{BoolExpr, expr};

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
use espresso_logic::{BoolExpr, expr};

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
use espresso_logic::{BoolExpr, expr};

fn main() {
    // ~(a * b) = ~a + ~b (using string notation)
    let expr1 = expr!(!("a" * "b"));
    let expr2 = expr!(!"a" + !"b");

    // ~(a + b) = ~a * ~b
    let expr3 = expr!(!("a" + "b"));
    let expr4 = expr!(!"a" * !"b");
}
```

## Working with Cubes

### Iterating Over Cubes

```rust
use espresso_logic::{BoolExpr, Cover, CoverType};

fn main() -> std::io::Result<()> {
    let expr = BoolExpr::parse("a * b + ~a * c")?;
    let mut cover = Cover::new(CoverType::F);
    cover.add_expr(expr, "out")?;
    
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
    cover.add_expr(expr, "output")?;
    
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
    cover.add_expr(expr, "out")?;

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
use espresso_logic::BoolExpr;

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

### DNF Conversion

- Conversion to DNF: Worst case exponential (DNF can be exponentially larger)
- For most practical expressions: linear to quadratic
- Uses De Morgan's laws to push NOTs to literals

### Minimization

- Dominated by Espresso algorithm time
- Boolean expression overhead is negligible
- Thread-local storage overhead is minimal

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

    // Logical equivalence (truth table comparison)
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
use espresso_logic::{BoolExpr, expr};

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
    // For simple expressions: use parser
    let expr1 = BoolExpr::parse("a * b + c")?;

    // For programmatic construction: use expr! macro
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let expr2 = expr!(a * b + c);

    // For complex nested logic: use method API
    let complex_subexpr = expr!("x" * "y");
    let expr3 = a.and(&b).or(&complex_subexpr.not());
    
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

```rust
use espresso_logic::*;

fn main() -> std::io::Result<()> {
    // Good: minimize intermediate results
    let large_expr = expr!("a" * "b" + "c" * "d" + "e" * "f");
    let other_term = expr!("x" * "y");
    
    let intermediate = large_expr.minimize()?;
    let final_expr = expr!(intermediate + other_term).minimize()?;

    // Less efficient: combine then minimize
    let large_expr2 = expr!("a" * "b" + "c" * "d" + "e" * "f");
    let other_term2 = expr!("x" * "y");
    let final_expr2 = large_expr2.or(&other_term2).minimize()?;
    
    Ok(())
}
```

### 4. Use Type System

```rust
use espresso_logic::*;

fn main() -> std::io::Result<()> {
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    
    // The type system prevents mistakes
    let expr: BoolExpr = expr!(a * b);  // Type-safe
    let mut cover: Cover = Cover::new(CoverType::F);  // Clear types
    cover.add_expr(expr, "output")?;  // Explicit conversion
    
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

### "Parse error" when using operators

❌ Wrong: Using bitwise operators
```rust
use espresso_logic::*;

// These will fail to parse at runtime
match BoolExpr::parse("a & b") {
    Err(_) => println!("Parse error: & not supported"),
    Ok(_) => unreachable!(),
}

match BoolExpr::parse("a | b") {
    Err(_) => println!("Parse error: | not supported"),
    Ok(_) => unreachable!(),
}
```

✅ Correct: Use boolean operators
```rust
use espresso_logic::*;

let _expr1 = BoolExpr::parse("a * b").unwrap();  // AND
let _expr2 = BoolExpr::parse("a + b").unwrap();  // OR
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
    cover.add_expr(expr, "out")?;

    println!("Cubes before: {}", cover.num_cubes());  // Check size
    cover.minimize()?;
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

