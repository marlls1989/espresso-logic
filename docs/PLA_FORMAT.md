# PLA File Format

## Overview

The PLA (Programmable Logic Array) format is a text-based format for representing Boolean functions. It was developed at UC Berkeley and is the standard input/output format for the Espresso logic minimizer.

## File Structure

A PLA file consists of several sections, each starting with a dot (`.`) directive:

```text
.i <num_inputs>
.o <num_outputs>
.ilb <input_label_1> <input_label_2> ... <input_label_n>
.ob <output_label_1> <output_label_2> ... <output_label_m>
.p <num_products>
<product_term_1>
<product_term_2>
...
<product_term_n>
.e
```

## Directives

### `.i` - Number of Inputs

Specifies the number of input variables.

```text
.i 3
```

This declares 3 input variables.

### `.o` - Number of Outputs

Specifies the number of output variables.

```text
.o 2
```

This declares 2 output variables.

### `.ilb` - Input Labels (Optional)

Assigns names to input variables. If omitted, inputs are numbered 0, 1, 2, ...

```text
.ilb a b c
```

Names the three inputs as `a`, `b`, and `c`.

### `.ob` - Output Labels (Optional)

Assigns names to output variables. If omitted, outputs are numbered 0, 1, 2, ...

```text
.ob f g
```

Names the two outputs as `f` and `g`.

### `.p` - Number of Product Terms

Specifies how many product terms (cubes) follow.

```text
.p 4
```

Indicates 4 product terms will be listed.

### `.e` - End of File

Marks the end of the PLA description.

```text
.e
```

## Product Terms (Cubes)

Each product term is a single line describing when outputs should be active. The format is:

```text
<input_pattern> <output_pattern>
```

### Input Pattern Encoding

Each input position can have one of three values:

| Symbol | Meaning | Interpretation |
|--------|---------|----------------|
| `0` | Input must be 0 | Variable is complemented (NOT) |
| `1` | Input must be 1 | Variable is true |
| `-` | Don't care | Variable can be either 0 or 1 |

### Output Pattern Encoding

Each output position can have:

| Symbol | Meaning |
|--------|---------|
| `1` | Output is 1 (ON) |
| `0` | Output is 0 (OFF) |
| `~` | Output is complemented (rarely used) |

## Examples

### Example 1: XOR Function

A simple 2-input XOR function:

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

This represents: `f = ~a*b + a*~b`

### Example 2: 2-bit Adder

A 2-bit half adder with sum and carry outputs:

```text
.i 2
.o 2
.ilb a b
.ob sum carry
.p 3
00 00       # a=0, b=0 => sum=0, carry=0
01 10       # a=0, b=1 => sum=1, carry=0
10 10       # a=1, b=0 => sum=1, carry=0
11 01       # a=1, b=1 => sum=0, carry=1
.e
```

### Example 3: Using Don't Cares

A function with don't care conditions:

```text
.i 3
.o 1
.ilb x y z
.ob f
.p 2
1-- 1       # f=1 when x=1 (y and z don't matter)
-1- 1       # f=1 when y=1 (x and z don't matter)
.e
```

This represents: `f = x + y`

### Example 4: Multi-Output Function

```text
.i 3
.o 2
.ilb a b c
.ob and_out or_out
.p 3
111 11      # All inputs high => both outputs high
110 10      # a=1,b=1,c=0 => and_out=0, or_out=1
011 01      # a=0,b=1,c=1 => and_out=0, or_out=1
.e
```

## Cover Types

Espresso supports different types of covers:

### F Type (ON-set only)

The default type. Lists only the conditions where outputs are 1.

### FD Type (ON-set + Don't-cares)

Includes both ON conditions and don't-care conditions. Don't-care product terms have `~` in output positions.

```text
.i 2
.o 1
.type fd
.p 3
00 1        # ON-set
11 1        # ON-set
01 ~        # Don't-care
.e
```

### FR Type (ON-set + OFF-set)

Includes both ON conditions (output=1) and explicit OFF conditions (output=0).

### FDR Type (All three sets)

Includes ON-set, Don't-care set, and OFF-set.

## Comments

Lines starting with `#` are comments and are ignored:

```text
# This is a comment
.i 2        # Comments can also appear after directives
.o 1
```

## Whitespace

- Spaces and tabs separate fields
- Empty lines are ignored
- Leading/trailing whitespace is ignored

## Working with PLA Files in Rust

### Reading PLA Files

```rust
use espresso_logic::{Cover, Minimizable, PLAReader};

fn main() -> std::io::Result<()> {
    // Read from string
    let pla_text = ".i 2\n.o 1\n.p 1\n01 1\n.e\n";
    let cover = Cover::from_pla_string(pla_text)?;
    
    println!("Loaded cover with {} inputs", cover.num_inputs());
    
    Ok(())
}
```

### Writing PLA Files

```rust
use espresso_logic::{Cover, CoverType, Minimizable, PLAReader, PLAWriter};

fn main() -> std::io::Result<()> {
    // Create or load a cover
    let cover = Cover::from_pla_string(".i 2\n.o 1\n.p 1\n01 1\n.e\n")?;
    
    // Write to file
    cover.to_pla_file("output.pla", CoverType::F)?;

    // Convert to string
    let pla_string = cover.to_pla_string(CoverType::F)?;
    println!("{}", pla_string);
    
    Ok(())
}
```

### Minimizing PLA Files

```rust,no_run
use espresso_logic::{Cover, CoverType, Minimizable, PLAReader, PLAWriter};

fn main() -> std::io::Result<()> {
    // Read PLA file
    let mut cover = Cover::from_pla_file("input.pla")?;
    
    println!("Before: {} cubes", cover.num_cubes());
    
    // Minimize
    cover = cover.minimize()?;
    
    println!("After: {} cubes", cover.num_cubes());
    
    // Write result
    cover.to_pla_file("output.pla", CoverType::F)?;
    
    Ok(())
}
```

## Format Variations

### Minimal Format

The minimal valid PLA file requires only `.i`, `.o`, `.p`, and `.e`:

```text
.i 2
.o 1
.p 1
11 1
.e
```

### Extended Format

Can include additional directives for documentation:

- `.type` - Specify cover type (f, fd, fr, fdr)
- `.phase` - Output phase information
- Comments for documentation

## References

- [Original Espresso Documentation](../espresso-src/README)
- [UC Berkeley Espresso Page](https://embedded.eecs.berkeley.edu/pubs/downloads/espresso/index.htm)
- [Wikipedia: Espresso Heuristic Logic Minimizer](https://en.wikipedia.org/wiki/Espresso_heuristic_logic_minimizer)

## See Also

- [Examples Guide](EXAMPLES.md) - Code examples using PLA files
- [API Documentation](API.md) - Complete API reference
- [CLI Guide](CLI.md) - Command-line usage with PLA files

