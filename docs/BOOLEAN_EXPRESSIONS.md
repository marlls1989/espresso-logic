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

// Create variables
let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");

// Build expression with clean syntax
let xor = expr!(a * !b + !a * b);

// Minimize
let minimized = xor.minimize()?;
println!("{}", minimized);
```

## Creating Boolean Expressions

### Method 1: Variable Creation

```rust
let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");
let c = BoolExpr::variable("c");
```

Variable names can be:
- Any valid Rust identifier
- Multi-character (e.g., `"input_a"`, `"clk_enable"`)
- Case-sensitive (`"A"` and `"a"` are different)

### Method 2: Constants

```rust
let t = BoolExpr::constant(true);
let f = BoolExpr::constant(false);
```

### Method 3: Parsing Strings

```rust
let expr = BoolExpr::parse("(a + b) * (c + d)")?;
```

## Building Expressions

### Method API (Recommended for Complex Logic)

The method API uses explicit method calls:

```rust
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
```

**Advantages:**
- Explicit and clear
- No reference syntax needed
- Easy to chain
- Good for complex nested expressions

### Operator Overloading

Boolean expressions support Rust's standard operators:

```rust
let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");

let and_expr = &a * &b;        // AND
let or_expr = &a + &b;         // OR
let not_expr = !&a;            // NOT

// Complex: XNOR
let xnor = &a * &b + &(!&a) * &(!&b);
```

**Note:** Requires `&` references due to Rust's ownership rules.

### `expr!` Macro (Recommended for Readability)

The `expr!` macro provides the cleanest syntax:

```rust
use espresso_logic::expr;

let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");
let c = BoolExpr::variable("c");

// Simple AND
let and_expr = expr!(a * b);

// Simple OR
let or_expr = expr!(a + b);

// NOT
let not_expr = expr!(!a);

// XOR
let xor = expr!(a * !b + !a * b);

// XNOR
let xnor = expr!(a * b + !a * !b);

// Majority function
let majority = expr!(a * b + b * c + a * c);

// With parentheses
let complex = expr!((a + b) * (c + d));
```

**Advantages:**
- No explicit `&` references
- Clean, readable syntax
- Matches mathematical notation
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
// These are equivalent:
let expr1 = BoolExpr::parse("~a * b + c")?;
let expr2 = BoolExpr::parse("((~a) * b) + c")?;

// NOT binds tighter than AND
let expr3 = BoolExpr::parse("a * ~b")?;  // a AND (NOT b)

// AND binds tighter than OR
let expr4 = BoolExpr::parse("a * b + c")?;  // (a AND b) OR c
```

### Parentheses

Use parentheses to override precedence:

```rust
let expr = BoolExpr::parse("(a + b) * c")?;  // (a OR b) AND c
let expr2 = BoolExpr::parse("~(a * b)")?;    // NOT (a AND b)
```

### Constants

The parser recognizes boolean constants:

```rust
let expr1 = BoolExpr::parse("a * 1")?;      // a AND true = a
let expr2 = BoolExpr::parse("b + 0")?;      // b OR false = b
let expr3 = BoolExpr::parse("true * a")?;   // true AND a = a
let expr4 = BoolExpr::parse("false + b")?;  // false OR b = b
```

### Variable Names

Variable names must:
- Start with a letter or underscore
- Contain only alphanumeric characters and underscores
- Be case-sensitive

```rust
// Valid variable names:
let expr = BoolExpr::parse("x * y")?;
let expr = BoolExpr::parse("input_a * input_b")?;
let expr = BoolExpr::parse("clk_enable + reset_n")?;
let expr = BoolExpr::parse("A * B")?;  // Different from a * b

// Whitespace is ignored
let expr = BoolExpr::parse("a*b+c")?;
let expr = BoolExpr::parse("a * b + c")?;  // Same as above
```

## Minimization

### Direct Minimization

The simplest way to minimize an expression:

```rust
let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");
let c = BoolExpr::variable("c");

// Redundant expression: a*b + a*b*c
let expr = a.and(&b).or(&a.and(&b).and(&c));

// Minimize directly
let minimized = expr.minimize()?;

println!("{}", minimized);  // Output: (a * b)
```

### Using ExprCover for More Control

For more control over the minimization process:

```rust
use espresso_logic::{BoolExpr, ExprCover, Cover};

let expr = BoolExpr::parse("a * b + a * b * c")?;

// Convert to cover
let mut cover = ExprCover::from_expr(expr);

// Inspect before minimization
println!("Variables: {:?}", cover.variables());
println!("Inputs: {}", cover.num_inputs());
println!("Outputs: {}", cover.num_outputs());
println!("Cubes before: {}", cover.num_cubes());

// Minimize
cover.minimize()?;

println!("Cubes after: {}", cover.num_cubes());

// Convert back to expression
let minimized = cover.to_expr();
println!("Result: {}", minimized);
```

## Common Patterns

### XOR (Exclusive OR)

```rust
// Method API
let xor = a.and(&b.not()).or(&a.not().and(&b));

// expr! macro
let xor = expr!(a * !b + !a * b);

// Parser
let xor = BoolExpr::parse("a * ~b + ~a * b")?;
```

### XNOR (Equivalence)

```rust
// Method API
let xnor = a.and(&b).or(&a.not().and(&b.not()));

// expr! macro
let xnor = expr!(a * b + !a * !b);

// Parser
let xnor = BoolExpr::parse("a * b + ~a * ~b")?;
```

### Majority Function (3 inputs)

```rust
// expr! macro (clearest)
let majority = expr!(a * b + b * c + a * c);

// Parser
let majority = BoolExpr::parse("a * b + b * c + a * c")?;

// Method API
let majority = a.and(&b)
    .or(&b.and(&c))
    .or(&a.and(&c));
```

### De Morgan's Laws

```rust
// ~(a * b) = ~a + ~b
let expr1 = expr!(!(a * b));
let expr2 = expr!(!a + !b);

// ~(a + b) = ~a * ~b
let expr3 = expr!(!(a + b));
let expr4 = expr!(!a * !b);
```

## Working with Cubes

### Iterating Over Cubes

```rust
let expr = BoolExpr::parse("a * b + ~a * c")?;
let cover = ExprCover::from_expr(expr);

for (i, (inputs, outputs)) in cover.cubes_iter().enumerate() {
    println!("Cube {}: inputs={:?}, outputs={:?}", i, inputs, outputs);
}
```

### Converting to PLA Format

```rust
use espresso_logic::{BoolExpr, ExprCover, PLAType};

let expr = BoolExpr::parse("a * b + c")?;
let cover = ExprCover::from_expr(expr);

// Export to PLA string
let pla_string = cover.to_pla_string(PLAType::F)?;
println!("{}", pla_string);

// Or write to file
cover.to_pla_file("output.pla", PLAType::F)?;
```

## Variable Ordering

Variables are automatically sorted alphabetically:

```rust
let c = BoolExpr::variable("c");
let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");

let expr = expr!(c * a * b);
let cover = ExprCover::from_expr(expr);

println!("{:?}", cover.variables());  // ["a", "b", "c"] (sorted)
```

This ensures consistent ordering in truth tables and PLA files.

## Error Handling

### Parsing Errors

```rust
match BoolExpr::parse("a * * b") {
    Ok(expr) => println!("Parsed: {}", expr),
    Err(e) => println!("Parse error: {}", e),
}
```

Common parse errors:
- Syntax errors: `"a * * b"`, `"a +"`, `"(a * b"`
- Invalid tokens: `"a & b"`, `"a | b"`, `"a ^ b"`
- Empty input: `""`

### Minimization Errors

```rust
use std::io;

let expr = BoolExpr::parse("a * b")?;

match expr.minimize() {
    Ok(minimized) => println!("Success: {}", minimized),
    Err(e) => eprintln!("Minimization failed: {}", e),
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
- Process isolation adds ~10-20ms overhead

### Memory

- Expressions use Arc for structural sharing
- Very memory efficient for large expressions
- Variables are deduplicated automatically

## Best Practices

### 1. Choose the Right API

```rust
// For simple expressions: use parser
let expr = BoolExpr::parse("a * b + c")?;

// For programmatic construction: use expr! macro
let expr = expr!(a * b + c);

// For complex nested logic: use method API
let expr = a.and(&b).or(&complex_subexpr.not());
```

### 2. Reuse Variables

```rust
// Good: reuse variable objects
let a = BoolExpr::variable("a");
let expr1 = expr!(a * b);
let expr2 = expr!(a + c);

// Works but less efficient: create variables multiple times
let expr1 = BoolExpr::parse("a * b")?;
let expr2 = BoolExpr::parse("a + c")?;
```

### 3. Minimize Early

```rust
// Good: minimize intermediate results
let intermediate = large_expr.minimize()?;
let final_expr = expr!(intermediate + other_term).minimize()?;

// Less efficient: combine then minimize
let final_expr = large_expr.or(&other_term).minimize()?;
```

### 4. Use Type System

```rust
// The type system prevents mistakes
let expr: BoolExpr = expr!(a * b);  // Type-safe
let cover: ExprCover = ExprCover::from_expr(expr);  // Clear conversion
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
let expr = BoolExpr::parse("a & b")?;  // Error: & not supported
let expr = BoolExpr::parse("a | b")?;  // Error: | not supported
```

✅ Correct: Use boolean operators
```rust
let expr = BoolExpr::parse("a * b")?;  // AND
let expr = BoolExpr::parse("a + b")?;  // OR
```

### "Expected &" in operator overloading

❌ Wrong: Missing references
```rust
let expr = a * b;  // Error: can't move a and b
```

✅ Correct: Use references or expr! macro
```rust
let expr = &a * &b;           // Operator overloading
let expr = expr!(a * b);      // expr! macro (cleaner)
```

### Expression doesn't minimize as expected

```rust
// Check the DNF conversion
let expr = BoolExpr::parse("(a + b) * (c + d)")?;
let cover = ExprCover::from_expr(expr);

println!("Cubes before: {}", cover.num_cubes());  // Check size
cover.minimize()?;
println!("Cubes after: {}", cover.num_cubes());

// View the result
let result = cover.to_expr();
println!("Result: {}", result);
```

## See Also

- [API Documentation](API.md) - Complete API reference
- [Process Isolation](PROCESS_ISOLATION.md) - Thread safety details
- [PLA Format](../espresso-src/README) - PLA file format specification
- [Examples](../examples/) - Working code examples

