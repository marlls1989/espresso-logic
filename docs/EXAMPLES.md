# Usage Examples

This guide provides examples for using espresso-logic across its two layers: the owned, syntactic
[`BoolExpr`], and the canonical [`Bdd`] builder layer. See
[BOOLEAN_EXPRESSIONS.md](BOOLEAN_EXPRESSIONS.md) for the deep dive.

## Table of Contents

- [Boolean Expressions](#boolean-expressions)
- [Binary Decision Diagrams](#binary-decision-diagrams)
- [Truth Tables](#truth-tables)
- [Multiple Outputs](#multiple-outputs)
- [PLA Files](#pla-files)
- [Low-Level API](#low-level-api)
- [Concurrent Execution](#concurrent-execution)

## Boolean Expressions

### Building expressions

`BoolExpr` is an owned value. Compose it with the [`expr!`](crate::expr) macro — infix syntax
where string literals are fresh variables (`*`/`&` AND, `+`/`|` OR, `^` XOR, `~`/`!` NOT):

```rust
use espresso_logic::expr;

let xor = expr!("a" ^ "b");
let sop = expr!("a" & "b" | !"a" & "c");

println!("{xor}");
println!("{sop}");
```

`expr!` is sugar for [`BoolExpr::build`](crate::BoolExpr::build), the closure builder it lowers
to. Use `build` directly when construction is data-driven (looping or folding a runtime set of
variables); see [Boolean Expression API](BOOLEAN_EXPRESSIONS.md) for that comparison.

### Parsing expressions

```rust
use espresso_logic::BoolExpr;

# fn main() -> Result<(), espresso_logic::expression::ParseBoolExprError> {
// The grammar accepts both `*`/`+`/`~` and `&`/`|`/`!`, which may be mixed.
let maths   = BoolExpr::parse("(a + b) * (c + d)")?;
let bitwise = BoolExpr::parse("(a | b) & (c | d)")?;
assert_eq!(maths, bitwise);

let mixed = BoolExpr::parse("a * b | c & d")?;
println!("{mixed}");
# Ok(())
# }
```

### Evaluation

Evaluation is a semantic operation, so it goes through the `Bdd` layer: build the expression into a
builder and evaluate the handle.

```rust
use espresso_logic::{bdd_builder, expr, Minterm, Symbol, Symbols};

let expr = expr!("a" & "b" | !"a");
let builder = bdd_builder!();
let f = builder.build(&expr);

// The assignment is a Minterm fixing each variable; a complete one over the support yields `Ok`.
let vars = Symbols::new(["a", "b"].iter().map(Symbol::new).collect());
let assignment = Minterm::from_symbols(vars, [Some(true), Some(false)]);
// (a & b) | !a  with a=true, b=false  =  false
assert_eq!(f.evaluate(&assignment), Ok(false));
```

### Variables

```rust
use espresso_logic::BoolExpr;

let expr = BoolExpr::parse("a & b | c & d").unwrap();
// `variables()` is a lazy iterator in token order; sort to compare.
let mut names: Vec<String> = expr.variables().map(|s| s.to_string()).collect();
names.sort();
assert_eq!(names, ["a", "b", "c", "d"]);
```

## Binary Decision Diagrams

`BoolExpr` is purely syntactic. For canonical, semantic work — logical equivalence, cofactors,
quantification — build into a [`Bdd`] handle from a [`BddBuilder`]. Each builder owns a private
manager; mint one with [`bdd_builder!`] (or [`sync_bdd_builder!`] for a thread-safe builder).

### Construction and operations

```rust
use espresso_logic::bdd_builder;

# fn main() -> Result<(), espresso_logic::expression::ParseBoolExprError> {
let builder = bdd_builder!();

// Compose without `.clone()` in a scope: `ScopedBdd` handles are Copy, so an operand is reused for free.
let f = builder.scope(|s| {
    let a = s.var("a");
    (a & s.var("b")) | (!a & s.var("c"))
});

// Or parse the function straight into the builder.
let g = builder.parse("a & b | !a & c")?;

assert!(f.equivalent_to(&g));
# Ok(())
# }
```

### Equivalence checking

```rust
use espresso_logic::bdd_builder;

let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");

// Commutativity holds at the function level (BoolExpr equality would not see this).
assert!((a.clone() & b.clone()).equivalent_to(&(b.clone() & a.clone())));

// The consensus term is redundant.
let consensus = (a.clone() & b.clone()) | (!a.clone() & builder.var("c")) | (b.clone() & builder.var("c"));
let reduced   = (a.clone() & b) | (!a & builder.var("c"));
assert!(consensus.equivalent_to(&reduced));
```

### Cofactors, quantification and queries

```rust
use espresso_logic::bdd_builder;

let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");
let f = a.clone() & b.clone();

// Restrict (Shannon cofactor): f|a=true == b.
assert!(f.restrict("a", true).equivalent_to(&b));

// Quantification.
assert!(f.forall(&["a"]).is_contradiction()); // ∀a. (a & b) == false
assert!(f.exists(&["a"]).equivalent_to(&b));    // ∃a. (a & b) == b

// Constant queries.
assert!((a.clone() | !a.clone()).is_tautology());
assert!((a.clone() & !a).is_contradiction());
```

### Materialisation and lowering

```rust
use espresso_logic::bdd_builder;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");
let c = builder.var("c");

let f = (a.clone() & b.clone()) | (a.clone() & b.clone() & c); // logically just a & b

// Enumerate cubes / minterms.
let cubes = f.to_cubes();
assert_eq!(cubes.num_cubes(), 1);
let minterms: Vec<_> = f.to_minterms(&["a", "b"]).collect();
assert_eq!(minterms.len(), 1);

// Minimise the ON-set, and lower to a factored expression.
let minimized = f.minimize()?;
assert_eq!(minimized.num_cubes(), 1);
let factored = f.to_expr();
assert!(builder.build(&factored).equivalent_to(&(a & b)));
# Ok(())
# }
```

## Truth Tables

### Building from truth tables

```rust
use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);

// XOR function: inputs [a, b], output [f]
cover.push(Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F));  // 01 -> 1
cover.push(Cube::anonymous(&[Some(true),  Some(false)], &[true], CubeType::F)); // 10 -> 1

cover = cover.minimize()?;
println!("Minimised to {} cubes", cover.num_cubes());
# Ok(())
# }
```

### Using don't-cares

```rust
use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);

// `None` is a don't-care input value.
cover.push(Cube::anonymous(&[Some(true), None], &[true], CubeType::F)); // 1- -> 1
cover.push(Cube::anonymous(&[None, Some(true)], &[true], CubeType::F)); // -1 -> 1

cover = cover.minimize()?;
# Ok(())
# }
```

## Multiple Outputs

A [`Cover`] is multi-output. Add each function as a named output and minimise once to optimise them
together.

```rust
use espresso_logic::{bdd_builder, Cover, CoverType, Minimizable};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");
let c = builder.var("c");

let mut cover = Cover::new(CoverType::F);
cover.add_bdd(&(a.clone() & b.clone()), "and_out")?;
cover.add_bdd(&(a.clone() | c.clone()), "or_out")?;
cover.add_bdd(&((a & b.clone()) | (b & c)), "complex_out")?;

let minimized = cover.minimize()?;

for (name, expr) in minimized.to_exprs() {
    println!("{name}: {expr}");
}
# Ok(())
# }
```

The same works from syntactic expressions with [`Cover::add_expr`]:

```rust
use espresso_logic::{BoolExpr, Cover, CoverType, Minimizable};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut cover = Cover::new(CoverType::F);
cover.add_expr(&BoolExpr::parse("a & b | c & d")?, "f1")?;
cover.add_expr(&BoolExpr::parse("x & y | z")?, "f2")?;

let minimized = cover.minimize()?;
for (name, expr) in minimized.to_exprs() {
    println!("{name}: {expr}");
}
# Ok(())
# }
```

### Composing functions: a 5-input threshold gate

A threshold gate that activates when at least four of five inputs are high, deactivates when at most
one is high, and holds otherwise. The activation and deactivation regions are composed at the `Bdd`
layer, and the outputs are minimised together. The BDD layer canonicalises each function, so the
negation in the next-state logic does not blow up into an exponential sum-of-products.

```rust
use espresso_logic::{bdd_builder, Cover, CoverType, Minimizable};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let builder = bdd_builder!();

// Activation: at least 4 of 5 inputs high.
let activation = builder.parse(
    "a & b & c & d & e \
   | a & b & c & d & !e \
   | a & b & c & !d & e \
   | a & b & !c & d & e \
   | a & !b & c & d & e \
   | !a & b & c & d & e",
)?;

// Deactivation: at most 1 of 5 inputs high.
let deactivation = builder.parse(
    "!a & !b & !c & !d & !e \
   | a & !b & !c & !d & !e \
   | !a & b & !c & !d & !e \
   | !a & !b & c & !d & !e \
   | !a & !b & !c & d & !e \
   | !a & !b & !c & !d & e",
)?;

// Next-state function: set on activation, hold while not deactivating. Compose in a scope, lifting the
// already-built activation/deactivation handles in (a zero-cost re-view) — no `.clone()`.
let next_q = builder.scope(|s| (s.lift(&activation) | s.var("q")) & !s.lift(&deactivation));

let mut cover = Cover::new(CoverType::F);
cover.add_bdd(&activation, "activation")?;
cover.add_bdd(&deactivation, "deactivation")?;
cover.add_bdd(&next_q, "next_q")?;

let minimized = cover.minimize()?;
for (name, expr) in minimized.to_exprs() {
    println!("{name} = {expr}");
}
# Ok(())
# }
```

## PLA Files

### Reading and writing

```rust,no_run
use espresso_logic::{CoverType, Minimizable, PlaCover, Symbol, PLAWriter};

# fn main() -> std::io::Result<()> {
// Read from a PLA file into a `PlaCover` (the variant reflects which label sections were present).
let mut cover = PlaCover::<Symbol>::from_pla_file("input.pla")?;

println!("Inputs: {}", cover.num_inputs());
println!("Outputs: {}", cover.num_outputs());
println!("Cubes before: {}", cover.num_cubes());

cover = cover.minimize()?;

println!("Cubes after: {}", cover.num_cubes());

cover.to_pla_file("output.pla", CoverType::F)?;
# Ok(())
# }
```

### PLA file format

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

See [PLA_FORMAT.md](PLA_FORMAT.md) for the complete specification.

### Inspecting cubes

```rust,no_run
use espresso_logic::{Minimizable, PlaCover, Symbol};

# fn main() -> std::io::Result<()> {
let mut cover = PlaCover::<Symbol>::from_pla_file("input.pla")?;

println!("Before: {} cubes", cover.num_cubes());
cover = cover.minimize()?;
println!("After: {} cubes", cover.num_cubes());

// `PlaCover` is a sum type over which label sections the file carried; match to reach the concrete
// `Cover` and inspect its cubes.
if let PlaCover::InputsOutputsNamed(c) = &cover {
    for cube in c.cubes() {
        println!("Cube: {:?}", cube);
    }
}
# Ok(())
# }
```

## Low-Level API

### Direct Espresso usage

```rust
use espresso_logic::espresso::{EspressoCover, CubeType};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let cubes = [
    (&[0, 1][..], &[1][..]), // 01 -> 1
    (&[1, 0][..], &[1][..]), // 10 -> 1
];
let f = EspressoCover::from_cubes(&cubes, 2, 1)?;

let (minimized, _d, _r) = f.minimize(None, None);

let result_cubes = minimized.to_cubes(2, 1, CubeType::F);
println!("Result: {} cubes", result_cubes.len());
# Ok(())
# }
```

### Custom configuration

```rust
use espresso_logic::espresso::Espresso;
use espresso_logic::EspressoConfig;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut config = EspressoConfig::default();
config.single_expand = true;
config.use_super_gasp = false;

let _esp = Espresso::new(2, 1, &config);
# Ok(())
# }
```

## Concurrent Execution

### Covers across threads

A [`Cover`] is `Send + Sync`; the thread-local Espresso instance is created lazily on the first
`minimize()`:

```rust
use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType, Minimizable};
use std::thread;

# fn main() -> std::io::Result<()> {
let handles: Vec<_> = (0..4).map(|_| {
    thread::spawn(move || -> std::io::Result<usize> {
        let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
        cover.push(Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F));
        cover.push(Cube::anonymous(&[Some(true), Some(false)], &[true], CubeType::F));
        cover = cover.minimize()?;
        Ok(cover.num_cubes())
    })
}).collect();

for handle in handles {
    println!("Result: {} cubes", handle.join().unwrap()?);
}
# Ok(())
# }
```

### A thread-safe BDD builder

[`sync_bdd_builder!`] mints a `Send + Sync` builder that can be shared by reference across threads:

```rust
use espresso_logic::sync_bdd_builder;
use std::thread;

let builder = sync_bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");
let f = a & b;

thread::scope(|s| {
    s.spawn(|| {
        assert!(f.equivalent_to(&(builder.var("a") & builder.var("b"))));
    });
});
```

### Parallel cover processing

```rust,no_run
use espresso_logic::{Minimizable, PlaCover, Symbol};
use std::thread;

# fn main() -> std::io::Result<()> {
let files = vec!["a.pla", "b.pla", "c.pla", "d.pla"];

let handles: Vec<_> = files.into_iter().map(|file| {
    thread::spawn(move || -> std::io::Result<usize> {
        let mut cover = PlaCover::<Symbol>::from_pla_file(file)?;
        cover = cover.minimize()?;
        Ok(cover.num_cubes())
    })
}).collect();

for handle in handles {
    println!("Result: {} cubes", handle.join().unwrap()?);
}
# Ok(())
# }
```

## Running Examples

The repository includes runnable examples:

```bash
cargo run --example xor_function
cargo run --example pla_file
cargo run --example espresso_direct_api
```

[`BoolExpr`]: crate::BoolExpr
[`Bdd`]: crate::bdd::Bdd
[`BddBuilder`]: crate::bdd::BddBuilder
[`BddBuilder::scope`]: crate::bdd::BddBuilder::scope
[`Scope::lift`]: crate::bdd::Scope::lift
[`Cover`]: crate::Cover
[`Cover::add_expr`]: crate::Cover::add_expr
[`bdd_builder!`]: crate::bdd_builder
[`sync_bdd_builder!`]: crate::sync_bdd_builder
