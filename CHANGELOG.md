# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.6.2] - 2024-11-06

### Fixed

- **Build System:** Lalrpop parser generation now outputs to `OUT_DIR` instead of source tree, fixing `cargo publish` verification failures
- **API:** Parser module is now properly private (was incorrectly exported as public)

### Removed

**Process Isolation Architecture:**
- Removed worker process spawning infrastructure (fork/exec pattern)
- Removed `worker.rs` module entirely
- Removed IPC layer (shared memory communication)
- Removed serialization layer (`SerializedCube`, `SerializedCover`, `WorkerSerializable` trait)
- Removed `IpcConfig` type (now uses `EspressoConfig` directly)
- Removed all serialization/deserialization in minimization path

**Dependencies:**
- `ctor` - No longer needed without worker mode detection
- `nix` - No longer needed without fork/IPC
- `memmap2` - No longer needed without shared memory
- `serde` - No longer needed without serialization
- `bincode` - No longer needed without serialization

### Changed

**Implementation:**
- Switched from process isolation to direct C calls using thread-local storage
- Minimization now calls C functions directly in the same thread
- No serialization overhead - direct type conversions only
- Simplified architecture with fewer layers

**Performance:**
- Eliminated ~10-20ms process spawning overhead per operation
- Eliminated serialization/deserialization overhead
- Better memory efficiency (no worker processes or shared memory buffers)

**Documentation:**
- Updated README.md to reflect thread-local implementation
- Removed `docs/PROCESS_ISOLATION.md` (historical pre-2.6.2 implementation)
- Updated all examples and API documentation

### Technical Notes

The C library uses C11 `_Thread_local` storage for all global variables (~50+ variables across 17 C files), enabling safe concurrent execution without process isolation or mutexes. Each thread gets independent global state. Accessor functions provide Rust FFI compatibility.

**C Code Modifications:**
- All global and static variables converted to `_Thread_local`
- `main.c` modified to use runtime initialization instead of static initialization (thread-local variables cannot use static initializers with complex values)
- Accessor functions added for Rust FFI compatibility
- C source synchronized with reference implementation while preserving thread-local modifications

### Migration

**No API changes** - This is a patch release. All public APIs remain unchanged. Users will automatically benefit from improved performance and simpler architecture.

## [2.6.1] - 2024-11-06

### Removed

- **`CoverBuilder::cubes()`** - Exposed internal `Cube` type which was not part of the public API
- **`CoverBuilder::num_cubes()`** - Duplicated the trait method with incorrect behavior (didn't filter by cube type)
- **`CoverBuilder::iter_cubes()`** - Duplicated functionality of `Cover::cubes_iter()` trait method

### Changed

- Updated crate-level documentation to include boolean expression API examples
- Added "Three Ways to Use Espresso" section with clear examples
- Added cover types documentation with usage examples
- Improved documentation structure and completeness

### Fixed

- Fixed unclosed HTML tag warning in `pla.rs` documentation
- Fixed clippy warnings about length comparisons in tests

### Migration Guide

If you were using the removed methods on `CoverBuilder`:

- **Instead of `cover.cubes()`** - This method exposed internal types and has been removed. Use `cover.cubes_iter()` from the `Cover` trait to iterate over cubes in the public format.
- **Instead of `cover.num_cubes()`** - Use the trait method (same name, automatically available via `Cover` trait). The trait method correctly filters cubes by type.
- **Instead of `cover.iter_cubes()`** - Use `cover.cubes_iter()` from the `Cover` trait (same functionality, standard API).

## [2.6.0] - 2024-11-06

### Added

#### High-Level Boolean Expression API
- **`BoolExpr`** - A new high-level type for representing boolean expressions
  - Programmatic construction with `.and()`, `.or()`, `.not()` methods
  - Operator overloading support (`*` for AND, `+` for OR, `!` for NOT)
  - Direct minimization with `.minimize()` method
  - Variable collection and inspection
  - Debug and Display implementations for readable output
- **`expr!` macro** - Clean syntax for building expressions without explicit references
  - Supports `*`, `+`, `!`, and parentheses
  - Example: `expr!(a * b + !a * !b)` for XNOR
- **Expression parser** - Parse boolean expressions from strings using lalrpop
  - Supports `+` (OR), `*` (AND), `~`/`!` (NOT)
  - Parentheses for grouping
  - Constants: `0`, `1`, `true`, `false`
  - Multi-character variable names (alphanumeric with underscores)
  - Proper operator precedence (NOT > AND > OR)
- **`ExprCover`** - Cover implementation for boolean expressions
  - Converts expressions to Disjunctive Normal Form (DNF)
  - Integrates with Espresso minimization
  - Converts minimized covers back to expressions
  - Implements all `Minimizable` trait methods
  - Supports PLA file export

#### New Examples and Tests
- `examples/boolean_expressions.rs` - Comprehensive examples (11 scenarios)
- `tests/test_boolean_expressions.rs` - 37 test cases covering:
  - Parsing (variables, operators, constants, precedence)
  - Expression construction (method API, macro, operators)
  - Minimization (various boolean functions)
  - PLA conversion
  - Edge cases and complex expressions

#### Build Infrastructure
- **lalrpop** integration for grammar-based parsing
  - Grammar file: `src/expression/bool_expr.lalrpop`
  - Build-time parser generation
- New dependencies: `lalrpop`, `lalrpop-util`

### Changed
- **API organization** - Added `expression` module to public exports
  - `pub use expression::{BoolExpr, ExprCover};`
- **Documentation** - Extensively updated for new features:
  - README.md now features boolean expressions prominently
  - docs/API.md has dedicated "High-Level API" section
  - All examples updated to show expression API first
- **Cargo.toml** - Added `boolean_expressions` example binary

### Technical Details
- Boolean expressions use `Arc<str>` for efficient variable name sharing
- Expressions are converted to DNF using De Morgan's laws
- Variables are stored in alphabetical order (BTreeSet) for consistency
- DNF cubes are directly compatible with Espresso's cover format
- Expression parsing is type-safe and returns helpful error messages
- All expression operations preserve structural sharing via Arc

### Performance
- Expression parsing: microseconds for typical expressions
- DNF conversion: linear in expression size
- No overhead vs. direct cover construction for minimization
- Operator overloading is zero-cost (inlined)

## [2.5.1] - 2025-11-05

### Fixed
- **CRITICAL**: Segfault from NULL pointers passed to espresso() - now create empty covers instead
- **CRITICAL**: Incorrect minimization results - OFF-set now auto-computed as complement(F,D) when not provided
- ACTIVE flag interference in CoverBuilder causing wrong results
- Cube structure initialization in Espresso::new() - properly initialize global state
- Memory leak in Espresso::drop() - now frees part_size
- PLA::from_file() dimension conflicts - tears down existing cube state before loading
- PLA::minimize() NULL pointer inconsistency - now matches Espresso::minimize()

### Added
- Comprehensive thread safety documentation (library is NOT thread-safe)
- Mutex usage example for multi-threaded applications
- CoverBuilder initialization requirement documentation
- Debug methods: Cover::debug_dump(), PLA::debug_dump_f(), PLA::debug_check_d_r(), PLA::get_f()
- Extensive test coverage: test_unsafe_api.rs (19 tests), test_pla_unsafe.rs (11 tests)

### Changed
- CoverBuilder::build() now uses cube.temp[0] following C API patterns from cvrin.c
- Espresso::minimize() and minimize_exact() now clone input covers (espresso makes own copies)
- PLA struct ptr field now pub(crate) for internal testing access

### Breaking
- Library explicitly documented as single-threaded only
- Tests must run with --test-threads=1
- CoverBuilder requires Espresso::new() to be called first

## [2.3.0] - 2024-11-05

### Added

#### Rust Wrapper (632f5c0)
- Complete Rust API with safe wrappers around C implementation
- `Espresso` struct for minimization operations
- `Cover` and `CoverBuilder` for programmatic truth table construction
- `PLA` struct for PLA file format operations
- FFI bindings auto-generated by bindgen
- Memory-safe RAII patterns for automatic resource management
- Support for both heuristic and exact minimization algorithms
- CLI binary 100% compatible with original C implementation
- Comprehensive documentation (README, API.md, CLI.md)
- Three working examples: minimize, xor_function, pla_file
- Contributing guidelines

#### Testing Infrastructure (fac5d08)
- Regression test scripts with automatic binary rebuilding
- Quick regression test suite (4 test cases, ~1 second)
- Comprehensive regression test suite (38 test cases, ~5 seconds)
- Integration tests for cover builder and PLA operations
- All 38 regression tests passing (byte-for-byte identical output to C)
- Test documentation in tests/README.md

#### Cross-Compilation Support (4704743)
- cargo-zigbuild integration for better cross-compilation
- Automatic Zig compiler detection in build.rs
- Optional UBSan flag configuration when using zigbuild
- Graceful fallback to standard cargo build
- Maintains full compatibility with all build methods

#### Documentation
- Comprehensive README with quick start and examples
- API reference documentation (docs/API.md)
- CLI usage guide (docs/CLI.md)
- ACKNOWLEDGMENTS.md with complete attribution
- CONTRIBUTING.md with development guidelines
- Man pages for espresso(1) and espresso(5)

#### Project Infrastructure
- Cargo.toml with proper metadata for crates.io
- build.rs for C compilation and FFI binding generation
- MIT License with proper UC Berkeley attribution
- GitHub-ready repository structure

### Changed
- Transformed from pure C project to Rust library + CLI
- Updated build system to use Cargo with cc and bindgen
- Modernized project structure for Rust ecosystem

### Maintained
- Original C implementation in espresso-src/ (preserved without modification)
- 100% algorithm compatibility with original Espresso
- PLA file format compatibility
- CLI interface and behavior

## [v1.1.1] - 2024-04-26 (Upstream)

Base fork from classabbyamp/espresso-logic

### Changed
- Don't redefine strdup
- Updated Makefile

## Previous Versions

See upstream repository: https://github.com/classabbyamp/espresso-logic

Original work by:
- 1988: UC Berkeley (Robert K. Brayton et al.)
- 2016: Sébastien Cottinet (modernized C version)
- 2024: classabbyamp (maintenance)

---

[2.3.0]: https://github.com/marlls1989/espresso-logic/compare/v1.1.1...v2.3.0
[v1.1.1]: https://github.com/classabbyamp/espresso-logic/releases/tag/v1.1.1
