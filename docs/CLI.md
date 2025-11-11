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
# Minimize a PLA file
espresso input.pla > output.pla

# With summary
espresso -s input.pla

# With trace information
espresso -t input.pla

# Exact minimization
espresso --do exact input.pla

# Output to file
espresso input.pla --out-file output.pla
```

If building locally, use `./target/release/espresso` after running `cargo build --release --features cli`.

## Command Line Options

### Main Options

- `-D, --do <COMMAND>` - Select algorithm
  - `espresso` - Heuristic minimization (default, fast)
  - `exact` - Exact minimization (slower, optimal)
  - `qm` - Quine-McCluskey method
  - `echo` - Echo input without modification
  - `stats` - Print statistics only
  - `simplify` - Simplify the function

- `-o, --output <FORMAT>` - Output format
  - `f` - ON-set only (default)
  - `fd` - ON-set and don't-care set  
  - `fr` - ON-set and OFF-set
  - `fdr` - All three sets

### Espresso Options

- `--fast` - Use fast mode (single expand)
- `--ness` - Don't remove essential primes
- `--nirr` - Don't force irredundant
- `--strong` - Use strong gasp
- `--random` - Use random order

### Output Control

- `-s, --summary` - Print execution summary
- `-t, --trace` - Print execution trace
- `-x, --no-output` - Suppress output
- `-O, --out-file <FILE>` - Write to file instead of stdout

### Other Options

- `-d, --debug` - Enable debugging
- `-h, --help` - Show help
- `-V, --version` - Show version

## Examples

### Basic Minimization

```bash
# Minimize a Boolean function
espresso examples/ex5 > output.pla
```

### With Options

```bash
# Fast mode with summary
espresso --fast -s examples/ex5

# Exact minimization with trace
espresso --do exact -t examples/ex5

# Output both F and D sets
espresso -o fd examples/ex5
```

### Using in Scripts

```bash
#!/bin/bash
# Minimize all PLA files in a directory

for file in *.pla; do
    echo "Processing $file..."
    espresso "$file" > "min_$file"
done
```

## Comparison with Original

The Rust CLI aims to be compatible with the original C version:

| Feature | Original C | Rust Wrapper | Status |
|---------|-----------|--------------|---------|
| Basic minimization | ✅ | ✅ | Working |
| Exact minimization | ✅ | ⚠️  | Partial |
| Output formats | ✅ | ✅ | Working |
| Espresso options | ✅ | ✅ | Working |
| Summary/trace | ✅ | ✅ | Working |
| Multiple input files | ✅ | ❌ | Planned |
| Stdin input | ✅ | ❌ | Planned |
| All subcommands | ✅ | ⚠️  | Partial |

## Current Limitations

1. **Stdin not supported** - Must specify input file
2. **Single file only** - Cannot process multiple files at once
3. **Some subcommands incomplete** - verify, so, so_both need work
4. **No backward compatibility mode** - Old -do/-out syntax not supported

## Programmatic API Alternative

For more control, use the Rust API directly:

```rust,no_run
use espresso_logic::{Cover, CoverType, Minimizable, PLAReader, PLAWriter};

fn main() -> std::io::Result<()> {
    let mut cover = Cover::from_pla_file("input.pla")?;
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

## Future Enhancements

- [ ] Stdin support for piping
- [ ] Multiple file processing
- [ ] All original subcommands
- [ ] Backward compatibility mode
- [ ] Progress bars for large files
- [ ] JSON output format
- [ ] Parallel processing option

## See Also

- [PLA File Format](../espresso-src/README)
- [Examples](../examples/)

