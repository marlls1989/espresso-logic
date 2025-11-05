# Espresso Logic Minimizer - API Documentation

## Core Types

### `Espresso`

The main interface to the Espresso logic minimizer.

```rust
pub struct Espresso
```

#### Methods

- `pub fn new(num_inputs: usize, num_outputs: usize) -> Self`
  
  Creates a new Espresso instance. Initializes the cube data structures for the specified number of inputs and outputs.

- `pub fn minimize(&mut self, f: Cover, d: Option<Cover>, r: Option<Cover>) -> Cover`
  
  Minimizes a Boolean function using the Espresso heuristic algorithm.
  
  **Parameters:**
  - `f`: The ON-set (minterms where the function evaluates to true)
  - `d`: Optional don't-care set (minterms where function value is unspecified)
  - `r`: Optional OFF-set (minterms where the function evaluates to false)
  
  **Returns:** A minimized cover

- `pub fn minimize_exact(&mut self, f: Cover, d: Option<Cover>, r: Option<Cover>) -> Cover`
  
  Performs exact minimization (guarantees minimal result but slower than heuristic).

### `Cover`

Represents a cover (set of cubes) in a Boolean function.

```rust
pub struct Cover
```

#### Methods

- `pub fn new(capacity: usize, cube_size: usize) -> Self`
  
  Creates a new empty cover with specified capacity and cube size.

- `pub fn count(&self) -> usize`
  
  Returns the number of cubes in this cover.

- `pub fn cube_size(&self) -> usize`
  
  Returns the size of each cube.

- `pub unsafe fn from_raw(ptr: sys::pset_family) -> Self`
  
  Creates a Cover from a raw C pointer (takes ownership).

- `pub fn into_raw(self) -> sys::pset_family`
  
  Converts to a raw C pointer (releases ownership).

### `CoverBuilder`

Helper for building covers programmatically.

```rust
pub struct CoverBuilder
```

#### Methods

- `pub fn new(num_inputs: usize, num_outputs: usize) -> Self`
  
  Creates a new cover builder.

- `pub fn add_cube(&mut self, inputs: &[u8], outputs: &[u8]) -> &mut Self`
  
  Adds a cube to the cover.
  
  **Parameters:**
  - `inputs`: Input values where 0 = must be 0, 1 = must be 1, 2 = don't care
  - `outputs`: Output values (0 or 1)

- `pub fn build(self) -> Cover`
  
  Builds and returns the cover.

### `PLA`

Represents a Programmable Logic Array structure.

```rust
pub struct PLA
```

#### Methods

- `pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Self>`
  
  Reads a PLA from a file in Berkeley PLA format.

- `pub fn from_string(s: &str) -> io::Result<Self>`
  
  Reads a PLA from a string.

- `pub fn minimize(&self) -> Self`
  
  Minimizes this PLA using Espresso.

- `pub fn to_file<P: AsRef<Path>>(&self, path: P, pla_type: PLAType) -> io::Result<()>`
  
  Writes this PLA to a file.

- `pub fn print_summary(&self)`
  
  Prints a summary of this PLA to stdout.

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

### Basic Boolean Function Minimization

```rust
use espresso_logic::{Espresso, CoverBuilder};

let mut esp = Espresso::new(2, 1);
let mut builder = CoverBuilder::new(2, 1);

// Add minterms
builder.add_cube(&[0, 1], &[1]);
builder.add_cube(&[1, 0], &[1]);

let cover = builder.build();
let minimized = esp.minimize(cover, None, None);
```

### Reading and Minimizing a PLA File

```rust
use espresso_logic::{PLA, PLAType};

let pla = PLA::from_file("input.pla")?;
let minimized = pla.minimize();
minimized.to_file("output.pla", PLAType::F)?;
```

### With Don't-Care Set

```rust
use espresso_logic::{Espresso, CoverBuilder};

let mut esp = Espresso::new(2, 1);

// Build ON-set
let mut f_builder = CoverBuilder::new(2, 1);
f_builder.add_cube(&[0, 1], &[1]);
f_builder.add_cube(&[1, 0], &[1]);
let f = f_builder.build();

// Build don't-care set
let mut d_builder = CoverBuilder::new(2, 1);
d_builder.add_cube(&[1, 1], &[1]);
let d = d_builder.build();

let minimized = esp.minimize(f, Some(d), None);
```

## Memory Management

All types implement proper RAII patterns:

- `Drop` implementations ensure C resources are freed
- `Clone` is implemented where appropriate (creates deep copies)
- Raw pointer conversions are `unsafe` and require manual memory management

The library handles the complexity of managing C memory while providing a safe Rust API.

## Thread Safety

The Espresso library is not thread-safe. Each `Espresso` instance maintains global state through the C library. Use separate instances per thread if parallel processing is needed.

## Performance Notes

- The Rust wrapper has negligible overhead compared to C
- Heuristic minimization (`minimize`) is fast but may not be optimal
- Exact minimization (`minimize_exact`) guarantees optimality but is slower
- Large Boolean functions (>1000 cubes) may take significant time

## Error Handling

Most operations return `Result` types for error handling:

- File I/O operations return `io::Result`
- Invalid inputs panic with descriptive messages
- C library errors are propagated as `io::Error`

