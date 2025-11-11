# Contributing to Espresso Logic Minimizer Rust Bindings

Thank you for your interest in contributing! This document provides guidelines for contributing to this project.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/yourusername/espresso-logic.git`
3. Create a new branch: `git checkout -b feature/your-feature-name`

## Development Setup

### Prerequisites

- Rust 1.70 or later
- C compiler (gcc, clang, or msvc)
- libclang (for bindgen)

On macOS:
```bash
xcode-select --install
```

On Ubuntu/Debian:
```bash
sudo apt-get install build-essential libclang-dev
```

### Building

```bash
# Standard build
cargo build

# Using Zig for better cross-compilation (recommended)
cargo install cargo-zigbuild
cargo zigbuild --release
```

### Running Tests

```bash
cargo test
```

### Running Examples

```bash
cargo run --example minimize
cargo run --example xor_function
cargo run --example pla_file
```

## Code Style

- Follow standard Rust conventions (use `rustfmt`)
- Run `cargo clippy` and address warnings
- Add documentation for public APIs
- Include examples in doc comments where appropriate

## Testing

- Add tests for new functionality
- Ensure all tests pass before submitting PR
- Consider adding integration tests for complex features

## Documentation

- Update rustdocs (doc comments) for new public APIs
- Add examples for new features
- Update README if adding significant functionality

## Pull Request Process

1. Ensure your code compiles and all tests pass
2. Update documentation as needed
3. Add a clear description of your changes
4. Reference any related issues
5. Be responsive to code review feedback

## Code Organization

- `src/lib.rs` - High-level safe API
- `src/sys.rs` - Low-level FFI bindings
- `build.rs` - Build script for C compilation
- `examples/` - Example programs
- `tests/` - Integration tests
- `docs/` - Additional documentation

## Adding New Features

When adding new Espresso functions:

1. Add to allowlist in `build.rs`
2. Create safe wrapper in `src/lib.rs`
3. Add tests
4. Document the API
5. Consider adding an example

## Performance Considerations

- The C library does the heavy lifting
- Rust wrapper should have minimal overhead
- Profile before optimizing
- Document any performance implications

## Memory Management

- Use RAII patterns (Drop trait)
- Ensure C resources are properly freed
- Document ownership transfer with raw pointers
- Use `unsafe` only when necessary and document why

## Common Issues

### Build Failures

- Check that libclang is installed
- Verify C compiler is in PATH
- Ensure espresso-src files are present

### Test Failures

- Some tests may fail if Espresso's global state isn't properly initialized
- Tests that use C FFI should be careful about initialization order

## Questions?

Feel free to open an issue for:
- Bug reports
- Feature requests
- Questions about usage
- Clarification on documentation

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

This project includes the original UC Berkeley Espresso code (Copyright (c) 1988, 1989, Regents of the University of California), which must be properly acknowledged in all distributions. See [ACKNOWLEDGMENTS.md](ACKNOWLEDGMENTS.md) for complete details.

