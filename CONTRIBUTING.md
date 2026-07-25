# Contributing

Thanks for your interest in the project. This document covers how to get set up and
what a change is expected to look like.

The binding rules — API conventions, testing requirements, commit and changelog rules —
live in [GUIDELINES.md](GUIDELINES.md). Read that before writing code; it is the
authority, and this file does not repeat it.

## Getting started

1. Fork the repository and clone your fork.
2. Cut a branch: `git checkout -b fix/short-description`. Never commit to `master`.

## Prerequisites

- Rust 1.82 or later (`rust-version` in `Cargo.toml`).
- A C compiler — gcc, clang or MSVC. The vendored Espresso sources are compiled from
  source on every build.
- libclang, which bindgen uses to generate the FFI bindings.

On macOS the Xcode command line tools supply all three:

```bash
xcode-select --install
```

On Debian/Ubuntu:

```bash
sudo apt-get install build-essential libclang-dev
```

On RHEL and derivatives, `clang-libs` installs libclang under a versioned prefix where
it cannot find its own builtin headers. The build script detects that case and recovers,
so `clang-devel` is not required.

## Building

```bash
cargo build                   # C sources + bindgen + the Rust crate
cargo build --features cli    # also builds the `espresso` CLI binary
```

WebAssembly builds target `wasm32-unknown-emscripten` — **not**
`wasm32-unknown-unknown`, which cannot work, as the C needs libc. It requires the
Emscripten SDK with `EMSDK` set.

The packed-set word width follows the target architecture. `ESPRESSO_BPI=32` overrides
it, so both widths can be exercised on one host; it is a testing knob, not a supported
build configuration.

## Testing

```bash
cargo test                          # unit + integration + ~209 doctests
cargo test --test test_integration  # one integration file
cargo test some_test_name           # by name substring

./tests/regression_test.sh          # ~388 cases, ~45s — the gate
./tests/quick_regression.sh         # 4 cases, ~1s — smoke test only

ESPRESSO_REF_BPI=32 ./tests/regression_test.sh   # native-width Rust vs 32-bit C
./scripts/check_leaks_macos.sh                   # leak checks
```

The regression suite is what actually gates a change: it builds the reference C binary
from `espresso-src` via its own `Makefile` and requires the Rust CLI to produce
**byte-identical** PLA output across every `-o {f,fd,fr,fdr}` format. `quick_regression.sh`
is a smoke test and does not substitute for it.

Doctests are part of the suite — every example in a `//!` or `///` comment is compiled
and run.

## Examples

```bash
cargo run --example xor_function
cargo run --example boolean_expressions
cargo run --example expr_macro_demo
cargo run --example pla_file
```

See the `[[example]]` entries in `Cargo.toml` for the full list.

## How the crate is organised

Two API levels sit over the C, in a workspace of two crates.

**FFI**

- `src/sys.rs` — raw bindgen output, all `unsafe`. Not used outside the wrapper layer.
- `src/espresso/` — the low-level safe wrapper (`Espresso`, `EspressoCover`,
  `EspressoConfig`), a thread-local reference-counted singleton. All 57 `sys::` call
  sites in the crate live here.

**High-level (the recommended API)**

- `src/cover/` — `Cover`, `Cube`, `Minterm`, `Symbols`, sum-of-products and truth-table
  representations, plus PLA file I/O under `src/cover/pla/`.
- `src/bdd/` — the canonical BDD layer, where semantic operations live. `Bdd<B, C>`
  handles minted by a `BddBuilder`; there is no process-global manager.
- `src/expression/` — `BoolExpr`, an owned syntactic expression with no
  canonicalisation, its lalrpop grammar, and the arena builder.
- `src/bin/espresso.rs` — the CLI behind the `cli` feature. This is what the regression
  suite validates.

`espresso-logic-macros/` is a separate workspace member providing the `expr!` proc
macro. It is versioned and published independently of the root crate.

## Adding a new C function

1. Add it to the `allowlist_*` calls in `build.rs`. Bindgen is explicit-allowlist only,
   so anything you miss is silently absent from `sys.rs` rather than a build error.
2. Wrap it safely in `src/espresso/`.
3. Surface it through the cover, BDD or expression layer if it belongs in the
   high-level API.
4. Add tests, and doctests on anything public.
5. Add a `CHANGELOG.md` entry.

## Pull requests

Before opening one:

- `cargo fmt`, then `cargo clippy` clean — see the lint rules in `GUIDELINES.md`.
- `cargo test` and `./tests/regression_test.sh` both green.
- `CHANGELOG.md` updated if the change is user-visible.

The description should cover every commit on the branch, not just the most recent one,
and reference any related issue.

## Common problems

**A cover fails to be created.** At the `src/espresso/` layer, every cover and instance
on a thread must share the same dimensions; creating one with different dimensions fails
until every `EspressoCover` on that thread has been dropped. The high-level `Cover` API
manages dimensions dynamically and hides this.

**`'stddef.h' file not found` during the build.** libclang cannot locate its builtin
headers. The build script recovers from the common cases automatically; if it still
fails, `BINDGEN_EXTRA_CLANG_ARGS` or `LIBCLANG_PATH` will override the discovery.

**Thread-safety.** Each thread gets its own copy of the Espresso global cube state via
C11 `_Thread_local` storage (`espresso-src/thread_local_accessors.c`). Nothing is shared
across threads at the C level.

## Questions

Open an issue for bug reports, feature requests, or anything unclear in the
documentation.

## License

By contributing, you agree that your contributions will be licensed under the MIT
License.

This project includes the original UC Berkeley Espresso code (Copyright (c) 1988, 1989,
Regents of the University of California), which must be properly acknowledged in all
distributions. See [ACKNOWLEDGMENTS.md](ACKNOWLEDGMENTS.md) for complete details.
