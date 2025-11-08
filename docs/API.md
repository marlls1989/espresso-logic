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
  
  This is a convenience method that creates a `Cover`, adds the expression to it,
  minimizes it, and returns the minimized expression.
  
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

### `Cover` - Unified Dynamic Cover

The unified cover type that supports dynamic sizing, boolean expressions, and PLA files.
Dimensions grow automatically as cubes are added, and it provides the primary interface
for working with Boolean functions in this library.

```rust
pub struct Cover
```

#### Construction Methods

- `pub fn new(cover_type: CoverType) -> Self`
  
  Creates a new empty cover with the specified type.
  
  ```rust
  let cover = Cover::new(CoverType::F);  // ON-set only
  let cover = Cover::new(CoverType::FD); // ON-set + Don't-cares
  ```

- `pub fn with_labels<S: AsRef<str>>(cover_type: CoverType, input_labels: &[S], output_labels: &[S]) -> Self`
  
  Creates a cover with pre-defined variable labels.
  
  ```rust
  let cover = Cover::with_labels(CoverType::F, &["a", "b", "c"], &["out"]);
  ```

- `pub fn from_pla_file<P: AsRef<Path>>(path: P) -> io::Result<Self>`
  
  Loads a cover from a PLA file.
  
  ```rust
  let cover = Cover::from_pla_file("input.pla")?;
  ```

- `pub fn from_pla_reader<R: BufRead>(reader: R) -> io::Result<Self>`
  
  Loads a cover from any `BufRead` implementation.
  
  ```rust
  use std::io::Cursor;
  let reader = Cursor::new(pla_content.as_bytes());
  let cover = Cover::from_pla_reader(reader)?;
  ```

- `pub fn from_pla_string(s: &str) -> io::Result<Self>`
  
  Loads a cover from a PLA format string.
  
  ```rust
  let pla = ".i 2\n.o 1\n.p 1\n01 1\n.e\n";
  let cover = Cover::from_pla_string(pla)?;
  ```

#### Adding Data

- `pub fn add_cube(&mut self, inputs: &[Option<bool>], outputs: &[Option<bool>])`
  
  Adds a cube to the cover. Dimensions grow automatically if needed.
  
  ```rust
  let mut cover = Cover::new(CoverType::F);
  cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);  // 01 -> 1
  // Dimensions automatically set to 2 inputs, 1 output
  ```

- `pub fn add_expr(&mut self, expr: BoolExpr, output_name: &str) -> Result<(), EspressoError>`
  
  Adds a boolean expression to a named output. Variables are matched by name,
  new variables are appended. Returns error if output name already exists.
  
  ```rust
  let mut cover = Cover::new(CoverType::F);
  let a = BoolExpr::variable("a");
  let b = BoolExpr::variable("b");
  
  cover.add_expr(a.and(&b), "result")?;
  // Input variables: a, b
  // Output variables: result
  ```

#### Query Methods

- `pub fn num_inputs(&self) -> usize` - Number of input variables
- `pub fn num_outputs(&self) -> usize` - Number of output variables
- `pub fn num_cubes(&self) -> usize` - Number of cubes
- `pub fn cover_type(&self) -> CoverType` - Cover type (F, FD, FR, or FDR)
- `pub fn input_labels(&self) -> &[Arc<str>]` - Input variable names
- `pub fn output_labels(&self) -> &[Arc<str>]` - Output variable names

#### Iteration

- `pub fn cubes(&self) -> impl Iterator<Item = &Cube>`
  
  Iterates over cubes as `Cube` references.

- `pub fn cubes_iter(&self) -> impl Iterator<Item = (Vec<Option<bool>>, Vec<Option<bool>>)>`
  
  Iterates over cubes as (inputs, outputs) tuples.

#### Minimization

- `pub fn minimize(&mut self) -> io::Result<()>`
  
  Minimizes this cover using Espresso with default configuration.

- `pub fn minimize_with_config(&mut self, config: &EspressoConfig) -> io::Result<()>`
  
  Minimizes with custom configuration.

#### Expression Conversion

- `pub fn to_exprs(&self) -> impl Iterator<Item = (Arc<str>, BoolExpr)> + '_`
  
  Converts all outputs to boolean expressions. Returns iterator of (name, expression) tuples.
  
  ```rust
  for (name, expr) in cover.to_exprs() {
      println!("{}: {}", name, expr);
  }
  ```

- `pub fn to_expr(&self, output_name: &str) -> Result<BoolExpr, EspressoError>`
  
  Converts a specific named output to an expression.
  
  ```rust
  let expr = cover.to_expr("result")?;
  ```

- `pub fn to_expr_by_index(&self, output_idx: usize) -> Result<BoolExpr, EspressoError>`
  
  Converts a specific output by index.
  
  ```rust
  let expr = cover.to_expr_by_index(0)?;
  ```

#### Example: Full Workflow

```rust
use espresso_logic::{BoolExpr, Cover, CoverType, expr};

// Create cover
let mut cover = Cover::new(CoverType::F);

// Add expressions to different outputs
let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");
let c = BoolExpr::variable("c");

cover.add_expr(expr!(a * b + a * b * c), "out1")?;  // Redundant
cover.add_expr(expr!(b + c), "out2")?;

println!("Before: {} cubes", cover.num_cubes());  // Multiple cubes
println!("Variables: {:?}", cover.input_labels()); // ["a", "b", "c"]

// Minimize
cover.minimize()?;

println!("After: {} cubes", cover.num_cubes());

// Convert back to expressions
for (name, expr) in cover.to_exprs() {
    println!("{}: {}", name, expr);
}
// out1: (a * b)
// out2: (c + b)
```

## PLA Serialization

All Cover instances can be serialized to PLA format using the `PLAWriter` trait methods:

- `pub fn write_pla<W: Write>(&self, writer: &mut W, pla_type: CoverType) -> io::Result<()>`
  
  Writes cover to PLA format using any `Write` implementation.
  
  ```rust
  use std::io::Write;
  let mut buffer = Vec::new();
  cover.write_pla(&mut buffer, CoverType::F)?;
  ```

- `pub fn to_pla_file<P: AsRef<Path>>(&self, path: P, pla_type: CoverType) -> io::Result<()>`
  
  Writes cover to a PLA file.
  
  ```rust
  cover.to_pla_file("output.pla", CoverType::F)?;
  ```

- `pub fn to_pla_string(&self, pla_type: CoverType) -> io::Result<String>`
  
  Converts cover to a PLA format string.
  
  ```rust
  let pla = cover.to_pla_string(CoverType::F)?;
  println!("{}", pla);
  ```

### `CoverType`

Output format for PLA files.

```rust
pub enum CoverType {
    F = 1,      // On-set only
    FD = 3,     // On-set and don't-care set
    FR = 5,     // On-set and off-set
    FDR = 7,    // All three sets
}
```

## Low-Level Espresso API

The low-level API provides direct access to the Espresso C library with maximum performance and fine-grained control. This API exposes thread-local state and requires understanding of its constraints.

### When to Use the Low-Level API

**Use the low-level API when you need:**
- Maximum performance with minimal overhead
- Direct control over the minimization process
- Access to intermediate results (F, D, R covers)
- Fine-grained configuration control

**Use high-level APIs (`BoolExpr`, `Cover`) when:**
- You want simple, safe, thread-safe APIs
- You don't need low-level control
- You're building multi-threaded applications without manual management

### Thread-Local State Constraints

⚠️ **Important:** The low-level API uses C11 thread-local storage for all global state. This has implications:

- **One Espresso instance per thread**: Only one active `Espresso` configuration per thread
- **Covers are thread-bound**: `EspressoCover` cannot be sent between threads (`!Send + !Sync`)
- **Dimension consistency**: All operations on a thread must use the same input/output dimensions
- **Independent threads**: Each thread has completely independent global state

The high-level APIs (`BoolExpr`, `Cover`) abstract these constraints away automatically.

### `Espresso`

The main Espresso instance that manages the C library state for a thread.

```rust
pub struct Espresso {
    num_inputs: usize,
    num_outputs: usize,
    config: EspressoConfig,
    initialized: bool,
    _marker: PhantomData<*const ()>,  // !Send + !Sync
}
```

#### Methods

- `pub fn new(num_inputs: usize, num_outputs: usize, config: &EspressoConfig) -> Rc<Self>`
  
  Creates a new Espresso instance with custom configuration.
  
  Initializes the cube structure for the specified dimensions and applies configuration settings.
  
  **⚠️ Important:** Only one Espresso configuration can exist per thread. If an instance with different dimensions already exists, this will panic. If an instance with the same dimensions exists, this returns a new handle to that instance.
  
  **Note:** Most users don't need to call this directly - use `EspressoCover::from_cubes()` which automatically creates an instance if needed.
  
  ```rust
  use espresso_logic::espresso::Espresso;
  use espresso_logic::EspressoConfig;
  
  let mut config = EspressoConfig::default();
  config.single_expand = true;
  let esp = Espresso::new(2, 1, &config);
  ```

- `pub fn try_new(num_inputs: usize, num_outputs: usize, config: Option<&EspressoConfig>) -> Result<Rc<Self>, String>`
  
  Fallible version of `new()` that returns an error instead of panicking.
  
  Returns an error if an Espresso instance with different dimensions already exists on the thread.
  
  ```rust
  match Espresso::try_new(2, 1, None) {
      Ok(esp) => { /* use esp */ },
      Err(e) => eprintln!("Cannot create Espresso: {}", e),
  }
  ```

- `pub fn current() -> Option<Rc<Self>>`
  
  Gets the current thread's Espresso instance, if one exists.
  
  Returns `None` if no instance has been created on this thread.
  
  ```rust
  if let Some(esp) = Espresso::current() {
      println!("Espresso configured for {} inputs", esp.num_inputs);
  }
  ```

- `pub fn minimize(self: &Rc<Self>, f: EspressoCover, d: Option<EspressoCover>, r: Option<EspressoCover>) -> (EspressoCover, EspressoCover, EspressoCover)`
  
  Minimizes a cover using the Espresso algorithm.
  
  **Parameters:**
  - `f`: The on-set cover to minimize
  - `d`: Optional don't-care set (computed if None)
  - `r`: Optional off-set (computed if None)
  
  **Returns:** Tuple of (minimized F, D, R) covers
  
  **Memory management:**
  - Input covers are cloned internally (original remains valid)
  - Returned covers are independently owned
  - All C memory is properly managed via RAII
  
  ```rust
  let esp = Espresso::new(2, 1, &EspressoConfig::default());
  let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
  let f = EspressoCover::from_cubes(cubes, 2, 1)?;
  
  let (minimized, d, r) = esp.minimize(f, None, None);
  println!("Minimized to {} cubes", minimized.to_cubes(2, 1, CubeType::F).len());
  ```

### `EspressoCover`

A cover (set of cubes) backed by C memory, tied to an Espresso instance.

```rust
pub struct EspressoCover {
    ptr: sys::pset_family,
    _espresso: Rc<Espresso>,        // Keeps Espresso alive
    _marker: PhantomData<*const ()>, // !Send + !Sync
}
```

#### Construction Methods

- `pub fn from_cubes(cubes: Vec<(Vec<u8>, Vec<u8>)>, num_inputs: usize, num_outputs: usize) -> Result<Self, String>`
  
  Creates a cover from a vector of cubes.
  
  **Cube format:** Each cube is `(inputs, outputs)` where values are:
  - `0` = variable must be 0
  - `1` = variable must be 1
  - `2` = don't care
  
  Automatically creates an Espresso instance if none exists on the thread.
  
  ```rust
  use espresso_logic::espresso::EspressoCover;
  
  let cubes = vec![
      (vec![0, 1], vec![1]),  // 01 -> 1
      (vec![1, 0], vec![1]),  // 10 -> 1
  ];
  let cover = EspressoCover::from_cubes(cubes, 2, 1)?;
  ```

#### Minimization

- `pub fn minimize(self, d: Option<EspressoCover>, r: Option<EspressoCover>) -> (EspressoCover, EspressoCover, EspressoCover)`
  
  Convenience method that minimizes this cover directly.
  
  Internally uses the Espresso instance associated with this cover.
  
  ```rust
  let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
  let f = EspressoCover::from_cubes(cubes, 2, 1)?;
  
  let (minimized, _d, _r) = f.minimize(None, None);
  ```

#### Extraction Methods

- `pub fn to_cubes(&self, num_inputs: usize, num_outputs: usize, cube_type: CubeType) -> Vec<(Vec<u8>, Vec<u8>)>`
  
  Extracts cubes from this cover as a vector.
  
  **Parameters:**
  - `num_inputs`: Number of input variables
  - `num_outputs`: Number of output variables
  - `cube_type`: Which cubes to extract (F, D, R, or FDR)
  
  ```rust
  let cubes = cover.to_cubes(2, 1, CubeType::F);
  for (inputs, outputs) in cubes {
      println!("Cube: {:?} -> {:?}", inputs, outputs);
  }
  ```

#### Memory Management

- `pub fn clone(&self) -> Self`
  
  Creates an independent clone of this cover.
  
  **Important:** Calls C `sf_save()` which allocates new C memory. The clone is completely independent - modifying one does not affect the other.
  
  ```rust
  let cover1 = EspressoCover::from_cubes(cubes, 2, 1)?;
  let cover2 = cover1.clone();  // Independent C memory
  // Both covers must be dropped separately
  ```

- `pub(crate) fn into_raw(self) -> sys::pset_family`
  
  Transfers ownership of the C pointer out of Rust.
  
  **⚠️ Unsafe contract:** The pointer must be either:
  - Passed to C code that takes ownership, OR
  - Wrapped back into `EspressoCover` via `from_raw()`
  
  Used internally for C interop. Not part of public API.

### `CubeType`

Specifies which cubes to extract from a cover.

```rust
pub enum CubeType {
    F = 1,      // On-set only
    D = 2,      // Don't-care set only
    R = 4,      // Off-set only
    FDR = 7,    // All three sets (F | D | R)
}
```

### `EspressoConfig`

Configuration options for the Espresso algorithm.

```rust
pub struct EspressoConfig {
    pub single_expand: bool,
    pub pos: bool,
    pub remove_essential: bool,
    pub force_irredundant: bool,
    // ... and more
}
```

See the main API documentation for complete field list.

### Thread Safety Example

Each thread gets independent state:

```rust
use espresso_logic::espresso::{EspressoCover, CubeType};
use std::thread;

fn main() -> Result<(), String> {
    let handles: Vec<_> = (0..4).map(|_| {
        thread::spawn(|| -> Result<usize, String> {
            // Each thread automatically gets its own Espresso instance
            let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];
            let f = EspressoCover::from_cubes(cubes, 2, 1)?;
            
            // Thread-safe: independent global state per thread
            let (result, _, _) = f.minimize(None, None);
            Ok(result.to_cubes(2, 1, CubeType::F).len())
        })
    }).collect();
    
    for handle in handles {
        let num_cubes = handle.join().unwrap()?;
        println!("Result: {} cubes", num_cubes);
    }
    Ok(())
}
```

### Memory Safety Guarantees

The low-level API maintains memory safety through:

- **RAII**: `EspressoCover` calls `sf_free()` on drop
- **Clone independence**: `clone()` uses `sf_save()` for independent C memory
- **Lifetime management**: Covers hold `Rc<Espresso>` to keep global state alive
- **Ownership transfer**: `into_raw()` nulls the pointer to prevent double-free

See [MEMORY_SAFETY.md](MEMORY_SAFETY.md) for detailed analysis.

### Performance Notes

The low-level API has minimal overhead:
- Direct C function calls with no IPC
- Zero-cost abstractions via RAII
- Thread-local storage eliminates locking overhead
- Slightly faster than high-level APIs due to less abstraction

For most applications, the performance difference is negligible. Use high-level APIs unless profiling shows a bottleneck.

## FFI Bindings

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

### Working with Cover and Expressions

```rust
use espresso_logic::{BoolExpr, Cover, CoverType, expr};

let a = BoolExpr::variable("a");
let b = BoolExpr::variable("b");

// Create expression
let expr = expr!(a * !b + !a * b);

// Convert to cover for more control
let mut cover = Cover::new(CoverType::F);
cover.add_expr(expr, "xor_output")?;

println!("Variables: {:?}", cover.input_labels());
println!("Inputs: {}", cover.num_inputs());
println!("Before: {} cubes", cover.num_cubes());

// Minimize
cover.minimize()?;

println!("After: {} cubes", cover.num_cubes());

// Convert back to expression
let minimized = cover.to_expr("xor_output")?;
println!("Result: {}", minimized);
```

### Manual Cube Construction

```rust
use espresso_logic::{Cover, CoverType};

// Create a cover for XOR function
let mut cover = Cover::new(CoverType::F);
cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);   // 01 -> 1
cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);   // 10 -> 1

// Minimize
cover.minimize()?;

println!("Result: {} cubes", cover.num_cubes());
```

### Reading and Minimizing a PLA File

```rust
use espresso_logic::{Cover, CoverType, PLAReader, PLAWriter};

// Read PLA file
let mut cover = Cover::from_pla_file("input.pla")?;

// Minimize
cover.minimize()?;

// Write result
cover.to_pla_file("output.pla", CoverType::F)?;
```

### Converting Between Formats

```rust
use espresso_logic::{BoolExpr, Cover, CoverType, PLAWriter};

// Expression to PLA
let expr = BoolExpr::parse("a * b + c")?;
let mut cover = Cover::new(CoverType::F);
cover.add_expr(expr, "output")?;
let pla_string = cover.to_pla_string(CoverType::F)?;

println!("{}", pla_string);
```

## Memory Management

All types implement proper RAII patterns:

- `Drop` implementations ensure C resources are freed
- `Clone` is implemented where appropriate (creates deep copies)
- Raw pointer conversions are `unsafe` and require manual memory management

The library handles the complexity of managing C memory while providing a safe Rust API.

## Thread Safety

**This library IS thread-safe!** All public APIs use **C11 thread-local storage**:

- `BoolExpr`, `Cover`, and `EspressoCover` are all safe to use concurrently
- The underlying C library uses `_Thread_local` for all global state
- Each thread gets its own independent copy of all global variables
- No manual synchronization needed
- Native C11 thread safety (not process isolation)

```rust
use espresso_logic::{Cover, CoverType};
use std::thread;

// Safe concurrent execution
let handles: Vec<_> = (0..4).map(|_| {
    thread::spawn(|| {
        let mut cover = Cover::new(CoverType::F);
        cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);
        cover.minimize()
    })
}).collect();

for handle in handles {
    handle.join().unwrap()?;  // All operations succeed
}
```

See [THREAD_LOCAL_IMPLEMENTATION.md](THREAD_LOCAL_IMPLEMENTATION.md) for technical details.

## Performance Notes

- **Rust wrapper overhead**: Negligible compared to C
- **Boolean expression parsing**: Very fast (microseconds for typical expressions)
- **DNF conversion**: Linear in expression size
- **Minimization**: 
  - Heuristic algorithm is fast and produces good results for most cases
  - Large Boolean functions (>1000 cubes) may take significant time
- **Thread-local storage overhead**: Minimal (native C11 thread-local variables)
  - Near-zero overhead for thread safety
  - Each thread has independent state

## Error Handling

Most operations return `Result` types for error handling:

- File I/O operations return `io::Result`
- Parsing errors return descriptive `String` messages
- Minimization operations return `io::Result`
- Invalid cube dimensions are caught at compile time (const generics)

