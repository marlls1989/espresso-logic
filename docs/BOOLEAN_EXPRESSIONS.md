# Boolean Expression API

This document describes the Boolean expression API in espresso-logic and the canonical BDD layer
that sits beside it.

## Two layers

The API is split into two layers with distinct responsibilities:

- **[`BoolExpr`]** is an **owned, syntactic** Boolean expression. It carries no manager, builder or
  brand: build it, compose it with the bitwise operators, parse it from text, display it, and fold over
  its structure — all as a plain value. It does **not** canonicalise. `a & b` and `b & a` are different
  expressions, and equality compares syntax, not the Boolean function. Semantic operations (evaluation,
  equivalence) are done through the `Bdd` layer.

- **[`Bdd`]** is the **canonical, semantic** layer. A `Bdd` handle is built from a [`BddBuilder`]
  (single-threaded or thread-safe), which owns a private BDD manager. Within one builder every
  Boolean function has exactly one root, so logical equivalence is an O(1) comparison and operations
  such as cofactors, quantification and tautology checks are available.

Reach for `BoolExpr` to construct and carry expressions around; build into a `Bdd` when a question is
about the *function* rather than the *syntax*.

## The `BoolExpr` layer

### Creating variables and constants

```rust
use espresso_logic::BoolExpr;

let a = BoolExpr::var("a");        // variable constructor
let b = BoolExpr::var("b");
let t = BoolExpr::constant(true);
let f = BoolExpr::constant(false);
```

Variable names can be any string: a Rust-style identifier, a multi-character name such as
`"clk_enable"`, or any other `&str`. Names are case-sensitive, so `"A"` and `"a"` are distinct.

### Composing with operators

`BoolExpr` composes through the bitwise operators: `&` (AND), `|` (OR), `^` (XOR), `!` (NOT). Each is
available by value and by reference, so the operands need not be cloned:

```rust
use espresso_logic::BoolExpr;

let a = BoolExpr::var("a");
let b = BoolExpr::var("b");

let and = &a & &b;     // AND
let or  = &a | &b;     // OR
let xor = &a ^ &b;     // XOR
let not = !&a;         // NOT

// XNOR is the negation of XOR.
let xnor = !(&a ^ &b);
```

The equivalent named methods are also available:

```rust
use espresso_logic::BoolExpr;

let a = BoolExpr::var("a");
let b = BoolExpr::var("b");

let and = a.and(&b);
let or  = a.or(&b);
let xor = a.xor(&b);
let not = a.not();
```

Composition concatenates token streams; the result is always a new syntactic expression, never a
canonical form.

### Parsing from text

[`BoolExpr::parse`] reads an expression from a string, as does [`str::parse`]:

```rust
use espresso_logic::BoolExpr;

# fn main() -> Result<(), espresso_logic::expression::ParseBoolExprError> {
let f = BoolExpr::parse("a & b | !c")?;
let g: BoolExpr = "a & b | !c".parse()?;
assert_eq!(f, g);
# Ok(())
# }
```

The grammar accepts two spellings for AND/OR/NOT, which may be mixed in one expression:

| Meaning | Spellings | Precedence (loose → tight) |
|---------|-----------|----------------------------|
| OR  | `+` or `\|` | lowest |
| XOR | `^`         |        |
| AND | `*` or `&`  |        |
| NOT | `~` or `!`  | highest (after parentheses) |

Parentheses override precedence. The `0`/`1` and `false`/`true` constants are recognised.

```rust
use espresso_logic::BoolExpr;

# fn main() -> Result<(), espresso_logic::expression::ParseBoolExprError> {
// The `*`/`+`/`~` and `&`/`|`/`!` spellings parse to the same operators.
let maths   = BoolExpr::parse("a * b + ~c")?;
let bitwise = BoolExpr::parse("a & b | !c")?;
assert_eq!(maths, bitwise);

// Mixed spellings in one expression.
let mixed = BoolExpr::parse("a * b | c")?;

// Parentheses and constants.
let grouped  = BoolExpr::parse("(a + b) * c")?;
let with_one = BoolExpr::parse("a * 1")?;
# Ok(())
# }
```

All binary operators are left-associative. XOR sits between AND and OR (mirroring Rust's `| < ^ < &`),
so `a + b ^ c` parses as `a + (b ^ c)` and `a ^ b * c` as `a ^ (b * c)`.

### Display

`Display` and `Debug` render an expression's own structure with minimal parentheses, using the
canonical spellings `&`, `|`, `^`, `!` and `1`/`0`. This is the syntactic structure of the value, not
a canonical or factored form:

```rust
use espresso_logic::BoolExpr;

let a = BoolExpr::var("a");
let b = BoolExpr::var("b");
let c = BoolExpr::var("c");

assert_eq!(format!("{}", &a & &b), "a & b");
assert_eq!(format!("{}", &a & &b | &c), "a & b | c");

// Parentheses appear only where precedence requires them.
assert_eq!(format!("{}", (&a | &b) & &c), "(a | b) & c");
assert_eq!(format!("{}", !(&a & &b)), "!(a & b)");
```

### Evaluation

Evaluation is a semantic operation, so it lives on [`Bdd`], not on the syntactic `BoolExpr`: build the
expression into a builder and evaluate the resulting handle. The assignment is a [`Minterm`] carrying
the fixed variables; [`Bdd::evaluate`] restricts the function by each fixed variable and returns
`Ok(true)`/`Ok(false)` once the result is determined. A variable absent from the assignment is left
*free*, not defaulted to `false`: a partial assignment that does not determine the function returns
`Err(residual)`, the function over the still-free variables. A complete assignment over the support
therefore always yields `Ok`:

```rust
use espresso_logic::{bdd_builder, BoolExpr, Minterm, Symbol, Symbols};

let expr = BoolExpr::var("a") & BoolExpr::var("b") | !BoolExpr::var("a");
let builder = bdd_builder!();
let f = builder.build(&expr);

let vars = Symbols::new(["a", "b"].iter().map(Symbol::new).collect());

let assignment = Minterm::from_symbols(vars.clone(), [Some(true), Some(false)]);
// (a & b) | !a  with a=true, b=false  =  false | false  =  false
assert_eq!(f.evaluate(&assignment), Ok(false));

let assignment = Minterm::from_symbols(vars, [Some(false), Some(false)]);
// (a & b) | !a  with a=false  =  false | true  =  true
assert_eq!(f.evaluate(&assignment), Ok(true));
```

### Syntactic variables

[`BoolExpr::variables`] returns the variables that occur in the expression's text, as a
`BTreeSet<Symbol>` in sorted order. This is a syntactic scan: `a & !a` still reports `a`, even though
the function does not depend on it. For the semantic support of a function, build a `Bdd` and use
[`Bdd::collect_variables`].

```rust
use espresso_logic::{BoolExpr, Symbol};
use std::collections::BTreeSet;

let expr = BoolExpr::parse("x & y | z").unwrap();
let vars: BTreeSet<Symbol> = expr.variables();
let names: Vec<String> = vars.iter().map(|s| s.to_string()).collect();
assert_eq!(names, ["x", "y", "z"]);
```

### Equality is syntactic, not logical

`PartialEq`/`Eq`/`Hash` compare the token structure. Two expressions are equal exactly when they are
the same syntactic tree:

```rust
use espresso_logic::BoolExpr;

let a = BoolExpr::var("a");
let b = BoolExpr::var("b");

assert_eq!(a.clone() & b.clone(), a.clone() & b.clone()); // identical structure
assert_ne!(a.clone() & b.clone(), b.clone() & a.clone()); // a & b is not b & a syntactically
assert_ne!(a.clone() & b.clone(), a.clone() | b.clone()); // different operator
```

`a & b` and `b & a` denote the same Boolean function but are different `BoolExpr` values. For logical
equality, build both into the BDD layer and use [`Bdd::equivalent_to`].

## The `Bdd` layer

### Contexts

A BDD builder owns a private manager and hands out [`Bdd`] handles branded to it. Mint one with the
[`bdd_builder!`] macro (single-threaded, `!Send`) or [`sync_bdd_builder!`] (`Send + Sync`). Each call
mints a distinct brand, so handles from two different builders cannot be combined — a compile error,
not a runtime check. A `Bdd` is `Clone` (a refcount bump), not `Copy`.

```rust
use espresso_logic::bdd_builder;

let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");
let f = a & b;            // handles are Clone (a refcount bump), not Copy
assert!(f.equivalent_to(&(builder.var("a") & builder.var("b"))));
```

An optional readable brand name appears in mismatch diagnostics; each call still mints a distinct
brand even when two are named the same:

```rust
use espresso_logic::bdd_builder;

let routing = bdd_builder!(Routing);
let _ = routing.var("a");
```

### Building handles

A builder builds handles directly, from a [`BoolExpr`], or from a [`Cover`]:

```rust
use espresso_logic::{bdd_builder, BoolExpr};

# fn main() -> Result<(), espresso_logic::expression::ParseBoolExprError> {
let builder = bdd_builder!();

let a = builder.var("a");
let one = builder.constant(true);

let expr = BoolExpr::parse("a & b")?;
let from_expr = builder.build(&expr);     // build a syntactic expression
let parsed = builder.parse("a & b")?;     // parse and build in one step

assert!(from_expr.equivalent_to(&parsed));
# Ok(())
# }
```

### Operations

`Bdd` handles support the same operators as `BoolExpr` (`&`, `|`, `^`, `!`), plus the BDD primitives:

```rust
use espresso_logic::bdd_builder;

let builder = bdd_builder!();
let s = builder.var("s");
let a = builder.var("a");
let b = builder.var("b");

// if-then-else: s ? a : b
let mux = s.clone().ite(a.clone(), b.clone());
assert!(mux.equivalent_to(&((s.clone() & a) | (!s & b))));
```

### Logical equivalence

[`Bdd::equivalent_to`] compares two handles for logical equality in O(1), because equivalent functions
share a canonical root:

```rust
use espresso_logic::bdd_builder;

let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");

// Commutativity and the consensus theorem hold at the function level.
assert!((a.clone() & b.clone()).equivalent_to(&(b.clone() & a.clone())));

let consensus = (a.clone() & b.clone()) | (!a.clone() & builder.var("c")) | (b.clone() & builder.var("c"));
let reduced   = (a.clone() & b.clone()) | (!a & builder.var("c"));
assert!(consensus.equivalent_to(&reduced)); // the b & c term is redundant
```

### Cofactors and quantification

[`Bdd::restrict`] (alias [`Bdd::cofactor`]) substitutes a variable with a constant; [`Bdd::forall`]
and [`Bdd::exists`] quantify over a set of variables. A name absent from the function is a no-op.

```rust
use espresso_logic::bdd_builder;

let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");
let f = a & b.clone();

// f|a=true == b
assert!(f.restrict("a", true).equivalent_to(&b.clone()));

// ∀a. (a & b) == false; ∃a. (a & b) == b
assert!(f.forall(&["a"]).is_contradiction());
assert!(f.exists(&["a"]).equivalent_to(&b));
```

### Constant queries

```rust
use espresso_logic::bdd_builder;

let builder = bdd_builder!();
let a = builder.var("a");

assert!((a.clone() | !a.clone()).is_tautology());
assert!((a.clone() & !a).is_contradiction());
```

### Introspection

```rust
use espresso_logic::bdd_builder;

let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");
let f = (a.clone() & b.clone()) | (!a & b);

// f depends only on b after canonicalisation.
assert_eq!(f.var_count(), 1);
assert_eq!(
    f.collect_variables().iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    ["b"]
);
let _ = f.node_count();
```

### Materialisation

[`Bdd::to_cubes`] enumerates the paths to TRUE as a single-output sum-of-products [`Cover`];
[`Bdd::to_minterms`] expands the function over an explicit variable set:

```rust
use espresso_logic::bdd_builder;

let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");
let f = a & b;

let cover = f.to_cubes();
assert_eq!(cover.num_outputs(), 1);

// One fully-assigned minterm over [a, b]: a=1, b=1.
let minterms = f.to_minterms(&["a", "b"]);
assert_eq!(minterms.len(), 1);
```

### Minimisation and lowering

[`Bdd::minimize`] minimises the function's ON-set with Espresso and returns a [`Cover`];
[`Bdd::to_expr`] lowers the function to a factored [`BoolExpr`]:

```rust
use espresso_logic::bdd_builder;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");
let c = builder.var("c");

// (a & b) | (a & b & c) is just a & b.
let f = (a.clone() & b.clone()) | (a.clone() & b.clone() & c);

let minimized = f.minimize()?;
assert_eq!(minimized.num_cubes(), 1);

let factored = f.to_expr();
assert!(builder.build(&factored).equivalent_to(&(a & b)));
# Ok(())
# }
```

## Covers and minimisation

A [`Cover`] is the sum-of-products / truth-table representation that Espresso minimises. Boolean
functions cross into it through several entry points.

### Converting a function to a cover

`Cover::from` accepts a `Bdd` handle or a `BoolExpr` (the expression forms build through a private
temporary builder):

```rust
use espresso_logic::{bdd_builder, Anonymous, BoolExpr, Cover, Symbol};

let builder = bdd_builder!();
let from_bdd: Cover<Symbol, Anonymous> = Cover::from(builder.var("a") & builder.var("b"));
let from_expr: Cover<Symbol, Anonymous> = Cover::from(BoolExpr::parse("a & b").unwrap());

assert_eq!(from_bdd.num_outputs(), 1);
assert_eq!(from_expr.num_outputs(), 1);
```

These covers have a single anonymous output. To recover a factored expression from one, use
[`Cover::to_expr_by_index`]:

```rust
use espresso_logic::{Cover, BoolExpr, Minimizable};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let cover = Cover::from(BoolExpr::parse("a & b | a & b & c")?);
let minimized = cover.minimize()?;
let expr = minimized.to_expr_by_index(0)?;
println!("{expr}");
# Ok(())
# }
```

### Named outputs

[`Cover::add_bdd`] and [`Cover::add_expr`] add a function as a named output. Adding several outputs
and minimising once optimises them together:

```rust
use espresso_logic::{bdd_builder, Cover, CoverType, Minimizable};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");
let c = builder.var("c");

let mut cover = Cover::new(CoverType::F);
cover.add_bdd(&(a.clone() & b.clone()), "p")?;
cover.add_bdd(&((a & b.clone()) | (b & c)), "q")?;

let minimized = cover.minimize()?;

// Recover each named output as a factored expression.
let p = minimized.to_expr("p")?;
let q = minimized.to_expr("q")?;
println!("p = {p}");
println!("q = {q}");
# Ok(())
# }
```

[`Cover::add_expr`] is the syntactic counterpart, building each expression through a temporary builder:

```rust
use espresso_logic::{BoolExpr, Cover, CoverType, Minimizable};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut cover = Cover::new(CoverType::F);
cover.add_expr(&BoolExpr::parse("a & b")?, "and_out")?;
cover.add_expr(&BoolExpr::parse("a | c")?, "or_out")?;

let minimized = cover.minimize()?;
for (name, expr) in minimized.to_exprs() {
    println!("{name}: {expr}");
}
# Ok(())
# }
```

### Heuristic and exact minimisation

[`Minimizable`] is implemented for [`Cover`]. `minimize` runs the fast heuristic algorithm;
`minimize_exact` is slower but guaranteed minimal:

```rust
use espresso_logic::{BoolExpr, Cover, Minimizable};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let cover = Cover::from(BoolExpr::parse("a & b | a & b & c")?);

let heuristic = cover.minimize()?;
let exact = cover.minimize_exact()?;
assert_eq!(heuristic.num_cubes(), exact.num_cubes());
# Ok(())
# }
```

To minimise an expression all the way to a factored expression, compose the cover, minimise, and
lower:

```rust
use espresso_logic::{BoolExpr, Cover, Minimizable};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let expr = BoolExpr::parse("a & b | a & b & c")?;
let factored = Cover::from(expr).minimize()?.to_expr_by_index(0)?;
println!("{factored}");
# Ok(())
# }
```

## Common patterns

### XOR and XNOR

```rust
use espresso_logic::bdd_builder;

let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");

let xor = a.clone() ^ b.clone();
assert!(xor.equivalent_to(&((a.clone() & !b.clone()) | (!a.clone() & b.clone()))));

let xnor = !(a.clone() ^ b.clone());
assert!(xnor.equivalent_to(&((a.clone() & b.clone()) | (!a & !b))));
```

### Majority function

```rust
use espresso_logic::{bdd_builder, BoolExpr};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");
let c = builder.var("c");

let majority = (a.clone() & b.clone()) | (b & c.clone()) | (a & c);
let parsed = builder.build(&BoolExpr::parse("a & b | b & c | a & c")?);
assert!(majority.equivalent_to(&parsed));
# Ok(())
# }
```

### De Morgan's laws

```rust
use espresso_logic::bdd_builder;

let builder = bdd_builder!();
let a = builder.var("a");
let b = builder.var("b");

assert!((!(a.clone() & b.clone())).equivalent_to(&(!a.clone() | !b.clone())));
assert!((!(a.clone() | b.clone())).equivalent_to(&(!a & !b)));
```

## Error handling

### Parse errors

[`BoolExpr::parse`] returns a [`ParseBoolExprError`] on malformed input:

```rust
use espresso_logic::BoolExpr;

assert!(BoolExpr::parse("a & & b").is_err()); // double operator
assert!(BoolExpr::parse("a @ b").is_err());   // @ is not an operator
assert!(BoolExpr::parse("").is_err());        // empty input
```

### Minimisation errors

`minimize` returns a `Result`:

```rust
use espresso_logic::{BoolExpr, Cover, Minimizable};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let cover = Cover::from(BoolExpr::parse("a & b")?);
match cover.minimize() {
    Ok(minimized) => println!("{} cubes", minimized.num_cubes()),
    Err(e) => eprintln!("minimisation failed: {e}"),
}
# Ok(())
# }
```

## See Also

- [PLA Format](PLA_FORMAT.md) — PLA file format specification
- [Examples](../examples/) — working code examples

[`BoolExpr`]: crate::BoolExpr
[`BoolExpr::parse`]: crate::BoolExpr::parse
[`BoolExpr::variables`]: crate::BoolExpr::variables
[`Bdd`]: crate::bdd::Bdd
[`Bdd::evaluate`]: crate::bdd::Bdd::evaluate
[`Bdd::equivalent_to`]: crate::bdd::Bdd::equivalent_to
[`Bdd::restrict`]: crate::bdd::Bdd::restrict
[`Bdd::cofactor`]: crate::bdd::Bdd::cofactor
[`Bdd::forall`]: crate::bdd::Bdd::forall
[`Bdd::exists`]: crate::bdd::Bdd::exists
[`Bdd::collect_variables`]: crate::bdd::Bdd::collect_variables
[`Bdd::to_cubes`]: crate::bdd::Bdd::to_cubes
[`Bdd::to_minterms`]: crate::bdd::Bdd::to_minterms
[`Bdd::minimize`]: crate::bdd::Bdd::minimize
[`Bdd::to_expr`]: crate::bdd::Bdd::to_expr
[`BddBuilder`]: crate::bdd::BddBuilder
[`Cover`]: crate::Cover
[`Minterm`]: crate::Minterm
[`Cover::add_bdd`]: crate::Cover::add_bdd
[`Cover::add_expr`]: crate::Cover::add_expr
[`Cover::to_expr_by_index`]: crate::Cover::to_expr_by_index
[`Minimizable`]: crate::Minimizable
[`bdd_builder!`]: crate::bdd_builder
[`sync_bdd_builder!`]: crate::sync_bdd_builder
[`ParseBoolExprError`]: crate::expression::ParseBoolExprError
