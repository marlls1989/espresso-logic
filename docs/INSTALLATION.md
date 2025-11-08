# Installation Guide

## As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
espresso-logic = "3.0"
```

The library has **minimal dependencies** (only `libc` and `lalrpop-util` for core functionality).

## As a CLI Tool

Install the command-line tool:

```bash
cargo install espresso-logic --features cli
```

This installs the `espresso` binary to your cargo bin directory (usually `~/.cargo/bin/`).

## Prerequisites

- Rust 1.70 or later
- C compiler (gcc, clang, or msvc)
- libclang (for bindgen during build)

### macOS

```bash
xcode-select --install
```

### Ubuntu/Debian

```bash
sudo apt-get install build-essential libclang-dev
```

### Fedora/RHEL

```bash
sudo dnf install gcc clang-devel
```

### Windows

**Option 1: Visual Studio (Recommended)**
- Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) with C++ support

**Option 2: MSYS2**
- Install [MSYS2](https://www.msys2.org/)
- Install mingw-w64 toolchain:
  ```bash
  pacman -S mingw-w64-x86_64-gcc mingw-w64-x86_64-clang
  ```

## Building from Source

Clone the repository:

```bash
git clone https://github.com/marlls1989/espresso-logic.git
cd espresso-logic
```

**Library only:**
```bash
cargo build --release --lib
```

**With CLI:**
```bash
cargo build --release --features cli
```

The build script automatically compiles the C source code and generates FFI bindings.

## Verification

Test your installation:

```bash
cargo test
```

Run an example:

```bash
cargo run --example boolean_expressions
```

## Troubleshooting

### "Could not find libclang"

Make sure libclang is installed and `LIBCLANG_PATH` is set if needed:

```bash
# macOS
export LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib

# Linux (example)
export LIBCLANG_PATH=/usr/lib/llvm-14/lib
```

### Build failures on Windows

Ensure you have either Visual Studio Build Tools or MSYS2 properly configured. The Rust toolchain should match your C compiler (MSVC or GNU).

### Link errors

Make sure you have a C compiler installed and accessible in your PATH.

## Platform Compatibility

- **Linux:** Tested on Ubuntu 20.04+, Debian, Fedora, Arch
- **macOS:** Tested on macOS 11+ (Intel and Apple Silicon)
- **Windows:** Tested on Windows 10+ (MSVC and MinGW)

