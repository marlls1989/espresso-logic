# Espresso Command Line Interface

## Overview

The Rust wrapper provides both a library API and a command-line interface compatible with the original Espresso CLI.

**Note:** The CLI is an optional feature. Library users get minimal dependencies by default.

## Installation

### Install as System Command (Recommended)

```bash
cargo install espresso-logic --features cli
```

This installs the `espresso` binary to your cargo bin directory (usually `~/.cargo/bin/`).

### Build Locally

The CLI requires the `cli` feature:

```bash
cargo build --release --bin espresso --features cli
```

The binary will be at `target/release/espresso`.

## Basic Usage

Assuming you've installed via `cargo install espresso-logic --features cli`:

```bash
# Minimise a PLA file
espresso input.pla > output.pla

# With summary
espresso -s input.pla

# With trace information
espresso -t input.pla

# Exact minimisation
espresso --do exact input.pla

# Output to file
espresso input.pla --out-file output.pla
```

> **Note:** the minimised PLA is written to **stdout** (or `--out-file`), while `-s`/`--summary`,
> `-t`/`--trace`, `--debug` and `--verbose` diagnostics go to **stderr** — so they never pollute a
> redirected `> output.pla`.

If building locally, use `./target/release/espresso` after running `cargo build --release --features cli`.

## Command Line Options

### Main Options

- `-D, --do <COMMAND>` - Select algorithm
  - `espresso` - Heuristic minimisation (default, fast)
  - `exact` - Exact minimisation (guarantees a minimal result; slower on large inputs)
  - `echo` - Echo input without modification
  - `stats` - Print statistics only

- `-o, --output <FORMAT>` - Output format
  - `f` - ON-set only (default)
  - `fd` - ON-set and don't-care set  
  - `fr` - ON-set and OFF-set
  - `fdr` - All three sets

### Espresso Options

- `--fast` - Use fast mode (single expand)
- `-e, --exact` - Use exact minimisation (alias for `--do exact`)

### Output Control

- `-s, --summary` - Print execution summary
- `-t, --trace` - Print execution trace
- `-x, --no-output` - Suppress output
- `-O, --out-file <FILE>` - Write to file instead of stdout

### Other Options

- `-d, --debug` - Enable debugging
- `-v, --verbose` - Enable verbose debug output
- `-h, --help` - Show help
- `-V, --version` - Show version

## Examples

### Basic Minimisation

```bash
# Minimise a Boolean function
espresso pla/ex5 > output.pla
```

### With Options

```bash
# Fast mode with summary
espresso --fast -s pla/ex5

# Exact minimisation with trace
espresso --do exact -t pla/ex5

# Output both F and D sets
espresso -o fd pla/ex5
```

### Using in Scripts

```bash
#!/bin/bash
# Minimise all PLA files in a directory

for file in *.pla; do
    echo "Processing $file..."
    espresso "$file" > "min_$file"
done
```

## Divergence from the original C tool

The Rust CLI reproduces the C tool's *core* behaviour exactly — for the supported options, its
PLA output is byte-identical to the reference binary across all `-o {f,fd,fr,fdr}` formats (this
is enforced by the regression suite). Its *option surface*, however, is a subset, and some
options that share a letter with the C tool differ in shape:

| Area | Original C | Rust CLI |
|---------|-----------|--------------|
| Core minimisation (`espresso`, `exact`, `echo`, `stats`) | ✅ | ✅ byte-identical output |
| Output formats `-o {f,fd,fr,fdr}` | ✅ | ✅ byte-identical output |
| `-D` subcommands (~36 in C: `verify`, `so`, `so_both`, `simplify`, `expand`, …) | ✅ | only `espresso`, `exact`, `echo`, `stats` |
| `-e <opt>` (C: takes an argument, e.g. `-e fast`) | ✅ | `-e` is a boolean alias for `--do exact` |
| `-v <type>` (C: takes a verbosity type argument) | ✅ | `-v` is a boolean verbose flag |
| `-S <strategy>`, `-r <n-m>` | ✅ | not implemented |
| Multiple input files / stdin input | ✅ | not implemented (single named file) |

The Berkeley man pages shipped in `man/` (`espresso.1`, `espresso.5`, dated 1988) document the
**C tool's** option set, not this CLI — for the Rust CLI, `espresso --help` is authoritative.

## Status and future

The CLI exists primarily to validate the library against the C reference implementation (the
regression suite drives it). It is not being extended incrementally: a later release will either
reimplement it against the full C option set or remove it in favour of the library API.

## Programmatic API Alternative

For more control, use the Rust API directly:

```rust,no_run
use espresso_logic::{Cover, CoverType, Minimizable, PlaCover, Symbol, PLAWriter};

fn main() -> std::io::Result<()> {
    let mut cover = PlaCover::<Symbol>::from_pla_file("input.pla")?;
    cover = cover.minimize()?;
    cover.to_pla_file("output.pla", CoverType::F)?;
    Ok(())
}
```

## Development Installation

To install from a local checkout:

```bash
# Install from current directory with cli feature
cargo install --path . --features cli

# Now available as 'espresso' command
espresso input.pla > output.pla
```

## Compatibility Notes

- Version string matches original: "UC Berkeley, Espresso Version #2.3"
- PLA file format is 100% compatible
- Output matches original Espresso
- Performance is equivalent (same C backend)

## Troubleshooting

### Segmentation Fault

If you encounter crashes:
1. Ensure PLA file format is correct
2. Check file permissions
3. Try with `--debug` flag
4. Report issue with sample file

### File Not Found

```bash
# Use absolute paths if needed
espresso /full/path/to/input.pla
```

### No Output

- Check if `-x` flag is set (suppresses output)
- Verify input file is valid PLA format
- Use `-s` to see summary

## See Also

- [PLA File Format](../espresso-src/README)
- [Examples](../examples/)

