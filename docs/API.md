# Espresso Logic Minimizer - API Documentation

## High-Level API: Boolean Expressions

The boolean expression API provides a high-level, user-friendly interface for working with boolean functions. It's the recommended way to use this library for most use cases.

### `BoolExpr`

A boolean expression that can be constructed programmatically, parsed from strings, or built using operator overloading.

```rust
pub struct BoolExpr
```

#### Construction Methods

- `pub fn variable(name: &str) -> Self`
  
  Creates a variable expression with the given name.
  
  ```rust
  let a = BoolExpr::variable("a");
  ```

- `pub fn constant(value: bool) -> Self`
  
  Creates a constant expression (true or false).
  
  ```rust
  let t = BoolExpr::constant(true);
  let f = BoolExpr::constant(false);
  ```

- `pub fn parse(input: &str) -> Result<Self, String>`
  
  Parses a boolean expression from a string.
  
  **Supported operators:**
  - `+` for OR
  - `*` for AND
  - `~` or `!` for NOT
  - Parentheses for grouping
  - Constants: `0`, `1`, `true`, `false`
  - Multi-character variable names (alphanumeric + underscore)
  
  ```rust
  let expr = BoolExpr::parse("(a + b) * (c + d)")?;
  let expr2 = BoolExpr::parse("~a * b + a * ~b")?; // XOR
  ```

#### Expression Methods

- `pub fn and(&self, other: &BoolExpr) -> BoolExpr`
  
  Logical AND: creates a new expression that is the conjunction of this and another.
  
  ```rust
  let result = a.and(&b);
  ```

- `pub fn or(&self, other: &BoolExpr) -> BoolExpr`
  
  Logical OR: creates a new expression that is the disjunction of this and another.
  
  ```rust
  let result = a.or(&b);
  ```

- `pub fn not(&self) -> BoolExpr`
  
  Logical NOT: creates a new expression that is the negation of this one.
  
  ```rust
  let result = a.not();
  ```

- `pub fn minimize(self) -> std::io::Result<BoolExpr>`
  
  Minimizes this boolean expression using Espresso.
  
  This is a convenience method that creates an `ExprCover`, minimizes it,
  and returns the minimized expression.
  
  ```rust
  let a = BoolExpr::variable("a");
  let b = BoolExpr::variable("b");
  let c = BoolExpr::variable("c");
  let expr = a.and(&b).or(&a.and(&b).and(&c)); // a*b + a*b*c (redundant)
  let minimized = expr.minimize()?; // Result: a*b
  ```

- `pub fn collect_variables(&self) -> BTreeSet<Arc<str>>`
  
  Collects all variables used in this expression in alphabetical order.

#### Operator Overloading

`BoolExpr` supports Rust's standard operators:

```rust
let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");

// Using operators (requires references)
let and_expr = &a * &b;        // AND
let or_expr = &a + &b;         // OR
let not_expr = !&a;            // NOT

// Chaining
let complex = &a * &b + &(!&a) * &(!&b);  // XNOR
```

**Note:** Operators require `&` references. For cleaner syntax, use the `expr!` macro.

### `expr!` Macro

Provides clean syntax for building boolean expressions without explicit `&` references.

```rust
use espresso_logic::expr;

let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");
let c = BoolExpr::variable("c");

// Clean syntax!
let xor = expr!(a * !b + !a * b);
let majority = expr!(a * b + b * c + a * c);
let complex = expr!((a + b) * (!c + d));
```

**Supported syntax:**
- `*` for AND
- `+` for OR
- `!` for NOT
- Parentheses for grouping

### `ExprCover`

A cover representation of a boolean expression that implements the `Cover` trait.

```rust
pub struct ExprCover
```

#### Methods

- `pub fn from_expr(expr: BoolExpr) -> Self`
  
  Creates a cover from a boolean expression by converting it to Disjunctive Normal Form (DNF).
  
  ```rust
  let expr = BoolExpr::parse("a * b + c")?;
  let cover = ExprCover::from_expr(expr);
  ```

- `pub fn to_expr(&self) -> BoolExpr`
  
  Converts the cover back to a boolean expression.
  
  ```rust
  let mut cover = ExprCover::from_expr(original_expr);
  cover.minimize()?;
  let minimized_expr = cover.to_expr();
  ```

- `pub fn variables(&self) -> &[Arc<str>]`
  
  Gets the variables in this cover (in alphabetical order).

- Implements `Minimizable` trait:
  - `pub fn minimize(&mut self) -> std::io::Result<()>`
  - `pub fn num_inputs(&self) -> usize`
  - `pub fn num_outputs(&self) -> usize`
  - `pub fn num_cubes(&self) -> usize`
  - And more...

#### Example: Full Workflow

```rust
use espresso_logic::{BoolExpr, ExprCover, expr};

// Create expression
let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");
let c = BoolExpr::variable("c");

// Build with macro
let expr = expr!(a * b + a * b * c);  // Redundant term

// Convert to cover
let mut cover = ExprCover::from_expr(expr);

println!("Before: {} cubes", cover.num_cubes());  // 2 cubes
println!("Variables: {:?}", cover.variables());   // ["a", "b", "c"]

// Minimize
cover.minimize()?;

println!("After: {} cubes", cover.num_cubes());   // 1 cube

// Convert back to expression
let minimized = cover.to_expr();
println!("Result: {}", minimized);  // (a * b)
```

## Low-Level API: Cubes and Covers

### `Cover<I, O>` (Const Generic Cover)

Represents a cover (set of cubes) with compile-time known dimensions.

```rust
pub struct Cover<const I: usize, const O: usize>
```

**Type Parameters:**
- `I`: Number of inputs (compile-time constant)
- `O`: Number of outputs (compile-time constant)

#### Methods

- `pub fn new() -> Self`
  
  Creates a new empty cover.
  
  ```rust
  let cover = Cover::<2, 1>::new();  // 2 inputs, 1 output
  ```

- `pub fn add_cube(&mut self, inputs: &[Option<bool>; I], outputs: &[bool; O])`
  
  Adds a cube to the cover.
  
  **Parameters:**
  - `inputs`: Array of input values where `Some(true)` = 1, `Some(false)` = 0, `None` = don't care
  - `outputs`: Array of output values
  
  ```rust
  let mut cover = Cover::<2, 1>::new();
  cover.add_cube(&[Some(true), Some(false)], &[true]);  // 10 -> 1
  cover.add_cube(&[None, Some(true)], &[true]);         // -1 -> 1
  ```

- `pub fn minimize(&mut self) -> std::io::Result<()>`
  
  Minimizes this cover using the Espresso heuristic algorithm.
  Runs in an isolated process for thread safety.

- `pub fn num_cubes(&self) -> usize`
  
  Returns the number of cubes in this cover.

- `pub fn cubes_iter(&self) -> impl Iterator<Item = (Vec<Option<bool>>, Vec<Option<bool>>)>`
  
  Iterates over the cubes in this cover, returning (inputs, outputs) tuples.

### `CoverBuilder<I, O>`

Helper for building covers programmatically with const generic dimensions.

```rust
pub struct CoverBuilder<const I: usize, const O: usize>
```

#### Methods

- `pub fn new() -> Self`
  
  Creates a new cover builder.
  
  ```rust
  let mut builder = CoverBuilder::<3, 2>::new();  // 3 inputs, 2 outputs
  ```

- `pub fn add_cube(&mut self, inputs: &[Option<bool>; I], outputs: &[Option<bool>; O]) -> &mut Self`
  
  Adds a cube to the cover.
  
  ```rust
  let mut builder = CoverBuilder::<2, 1>::new();
  builder.add_cube(&[Some(false), Some(true)], &[Some(true)]);
  ```

- `pub fn minimize(&mut self) -> std::io::Result<()>`
  
  Minimizes the cover in place.

- `pub fn num_cubes(&self) -> usize`
  
  Returns the number of cubes currently in the builder.

### `PLACover`

A dynamic cover that can load PLA files with runtime-determined dimensions.

```rust
pub struct PLACover
```

#### Methods

- `pub fn from_pla_file<P: AsRef<Path>>(path: P) -> io::Result<Self>`
  
  Reads a PLA cover from a file in Berkeley PLA format.
  
  ```rust
  let cover = PLACover::from_pla_file("input.pla")?;
  ```

- `pub fn from_pla_string(s: &str) -> io::Result<Self>`
  
  Reads a PLA cover from a string.

- `pub fn minimize(&mut self) -> io::Result<()>`
  
  Minimizes this cover using Espresso.
  
  ```rust
  let mut cover = PLACover::from_pla_file("input.pla")?;
  cover.minimize()?;
  ```

- `pub fn to_pla_file<P: AsRef<Path>>(&self, path: P, pla_type: PLAType) -> io::Result<()>`
  
  Writes this cover to a PLA file.
  
  ```rust
  cover.to_pla_file("output.pla", PLAType::F)?;
  ```

- `pub fn to_pla_string(&self, pla_type: PLAType) -> io::Result<String>`
  
  Converts this cover to a PLA format string.

- `pub fn num_inputs(&self) -> usize`
  
  Returns the number of inputs.

- `pub fn num_outputs(&self) -> usize`
  
  Returns the number of outputs.

- `pub fn num_cubes(&self) -> usize`
  
  Returns the number of cubes.

### `PLAType`

Output format for PLA files.

```rust
pub enum PLAType {
    F = 1,      // On-set only
    FD = 3,     // On-set and don't-care set
    FR = 5,     // On-set and off-set
    FDR = 7,    // All three sets
}
```

## Low-Level API

The `sys` module contains raw FFI bindings to the C library. Most users should use the safe wrappers instead.

```rust
pub mod sys
```

## Examples

### Boolean Expression Minimization (Recommended)

```rust
use espresso_logic::{BoolExpr, expr};

// Using the expr! macro
let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");
let c = BoolExpr::variable("c");

// Create expression with redundant term
let expr = expr!(a * b + a * b * c);

// Minimize directly
let minimized = expr.minimize()?;
println!("{}", minimized);  // (a * b)
```

### Parsing Boolean Expressions

```rust
use espresso_logic::BoolExpr;

// Parse from string
let expr = BoolExpr::parse("(a + b) * (c + d)")?;
let minimized = expr.minimize()?;

// XOR example
let xor = BoolExpr::parse("a * ~b + ~a * b")?;
let result = xor.minimize()?;
```

### Working with ExprCover

```rust
use espresso_logic::{BoolExpr, ExprCover, expr};

let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");

// Create expression
let expr = expr!(a * !b + !a * b);

// Convert to cover for more control
let mut cover = ExprCover::from_expr(expr);

println!("Variables: {:?}", cover.variables());
println!("Inputs: {}", cover.num_inputs());
println!("Before: {} cubes", cover.num_cubes());

// Minimize
cover.minimize()?;

println!("After: {} cubes", cover.num_cubes());

// Convert back to expression
let minimized = cover.to_expr();
println!("Result: {}", minimized);
```

### Low-Level Cover API

```rust
use espresso_logic::CoverBuilder;

// Create a cover for XOR function
let mut cover = CoverBuilder::<2, 1>::new();
cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);   // 01 -> 1
cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);   // 10 -> 1

// Minimize
cover.minimize()?;

println!("Result: {} cubes", cover.num_cubes());
```

### Reading and Minimizing a PLA File

```rust
use espresso_logic::{PLACover, PLAType};

// Read PLA file
let mut cover = PLACover::from_pla_file("input.pla")?;

// Minimize
cover.minimize()?;

// Write result
cover.to_pla_file("output.pla", PLAType::F)?;
```

### Converting Between Formats

```rust
use espresso_logic::{BoolExpr, ExprCover, PLAType};

// Expression to PLA
let expr = BoolExpr::parse("a * b + c")?;
let cover = ExprCover::from_expr(expr);
let pla_string = cover.to_pla_string(PLAType::F)?;

println!("{}", pla_string);
```

## Memory Management

All types implement proper RAII patterns:

- `Drop` implementations ensure C resources are freed
- `Clone` is implemented where appropriate (creates deep copies)
- Raw pointer conversions are `unsafe` and require manual memory management

The library handles the complexity of managing C memory while providing a safe Rust API.

## Thread Safety

**This library IS thread-safe!** All public APIs use **transparent process isolation**:

- `BoolExpr`, `ExprCover`, `Cover`, `CoverBuilder`, and `PLACover` are all safe to use concurrently
- The underlying C library (with global state) runs in isolated forked processes
- Parent process never touches global state
- No manual synchronization needed

```rust
use espresso_logic::CoverBuilder;
use std::thread;

// Safe concurrent execution
let handles: Vec<_> = (0..4).map(|_| {
    thread::spawn(|| {
        let mut cover = CoverBuilder::<2, 1>::new();
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        cover.minimize()
    })
}).collect();

for handle in handles {
    handle.join().unwrap()?;  // All operations succeed
}
```

See [PROCESS_ISOLATION.md](PROCESS_ISOLATION.md) for details.

## Performance Notes

- **Rust wrapper overhead**: Negligible compared to C
- **Boolean expression parsing**: Very fast (microseconds for typical expressions)
- **DNF conversion**: Linear in expression size
- **Minimization**: 
  - Heuristic (`minimize`) is fast but may not be optimal
  - Exact minimization guarantees optimality but is slower
  - Large Boolean functions (>1000 cubes) may take significant time
- **Process isolation overhead**: ~10-20ms per operation (fork + IPC)
  - Worth it for safety and simplicity
  - Amortized over typical minimization time

## Error Handling

Most operations return `Result` types for error handling:

- File I/O operations return `io::Result`
- Parsing errors return descriptive `String` messages
- Minimization operations return `io::Result`
- Invalid cube dimensions are caught at compile time (const generics)

