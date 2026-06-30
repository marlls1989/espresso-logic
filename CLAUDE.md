# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Rust bindings to the UC Berkeley Espresso heuristic logic minimiser. A `build.rs` compiles the vendored C implementation (`espresso-src/`) and generates FFI bindings with bindgen; the Rust crate layers safe, thread-safe, idiomatic APIs on top. Published as `espresso-logic` on crates.io (currently v5.0.x).

## Common commands

```bash
cargo build                      # builds C + bindgen + Rust
cargo build --features cli       # also builds the `espresso` CLI binary (needs clap)
cargo test                       # unit + integration + ~161 doctests
cargo test --test test_integration         # single integration test file
cargo test --test test_memory_safety -- --nocapture --test-threads=1
cargo test name_of_test          # single test by name substring
cargo run --example xor_function # run an example (see Cargo.toml [[example]] list)
cargo bench                      # criterion benchmarks (benches/pla_benchmarks.rs)

./tests/quick_regression.sh      # 4 cases, ~1s: Rust CLI vs C CLI
./tests/regression_test.sh       # ~363 cases, ~45s; builds C binary via espresso-src/Makefile
./scripts/check_leaks_macos.sh   # leak checks
```

Regression tests compare the Rust CLI against the reference C binary (`./bin/espresso`), built from `espresso-src` with `make`. The Rust CLI must produce byte-identical PLA output to C across all `-o {f,fd,fr,fdr}` format variations.

## Build system notes

- `build.rs` compiles all `espresso-src/*.c` **except `main.c`** as a library, runs `lalrpop::process_root()` for the expression grammar, and runs bindgen against `thread_local_accessors.h`. Bindgen uses an explicit `allowlist_*` — **new C functions/types/vars exposed to Rust must be added to the allowlist in `build.rs`** or they won't appear in `sys.rs`.
- C is compiled with `-std=c11`; thread safety relies on C11 `_Thread_local` storage (see `thread_local_accessors.c/.h`), giving each thread its own copy of Espresso's global cube state.
- WebAssembly target is `wasm32-unknown-emscripten` (NOT `wasm32-unknown-unknown` — the C code needs libc). Requires the Emscripten SDK with `EMSDK` set.

## Architecture

Two API levels sit over the C FFI. Re-exports at the crate root are defined in `src/lib.rs`.

**FFI layer**
- `src/sys.rs` — raw bindgen output (`include!` of `OUT_DIR/bindings.rs`), all `unsafe`. Don't use directly outside the wrappers.
- `src/espresso/` — low-level safe wrapper (`Espresso`, `EspressoCover`, `EspressoConfig`). Thread-local singleton with reference counting. **Critical constraint:** all covers/instances on a thread must share the same dimensions (#inputs, #outputs); creating different dimensions fails until every `EspressoCover` is dropped. Use this layer for access to separate ON/DC/OFF-set covers, or for lower per-call overhead (the high-level API also validates the cover and rebuilds an output `Cover`): measured ~10–14% faster on small covers but only ~1–5% / within noise on large ones — see the `api_overhead` group in `benches/pla_benchmarks.rs`.

**High-level layer (the recommended/default API)**
- `src/expression/` — `BoolExpr`, an **owned, syntactic** Boolean expression: a flat reverse-Polish `Token` stream (`rpn.rs`) with no canonicalisation (`a & b` and `b & a` are distinct values). Composed via the `expr!` macro (from the `espresso-logic-macros` proc-macro crate) and the `BoolExpr::build` arena builder (`builder.rs`, `ExprBuilder`/`Expr`), parsed from text via a lalrpop grammar (`parser`, `bool_expr.lalrpop`); `ast.rs` (`ExprNode` plus `fold`/`fold_with_context`), `operators.rs`, `display.rs`, and `factorization.rs` round it out. `manager.rs`/`manager_cell.rs` hold the shared BDD manager internals (`BddManager`, the `ManagerCell` storage cells) the BDD layer is built over.
- `src/bdd/` — the **canonical BDD layer**, where the semantic operations live. `Bdd<B, C>` (an owned, refcounted `Clone` handle; `handle.rs`) is minted by a `BddBuilder<B, C>` (`builder.rs`) parameterised by two orthogonal type parameters: a sealed `Brand` `B` (`brand.rs`, uniqueness only) and a `ManagerCell` `C` (`LocalCell`/`SyncCell`, storage backend and thread-safety). Builders come from the `bdd_builder!` / `sync_bdd_builder!` macros — there is **no process-global manager**; each builder owns its own. Operations: `&`/`|`/`^`/`!`, `ite`, `restrict`/`cofactor`/`forall`/`exists`, `equivalent_to`, `evaluate`, `to_cubes`/`to_minterms`, `minimize`, `to_expr`, `fold`; `Bdd::builder` recovers a builder onto a stored handle's manager. `scope.rs` adds `BddBuilder::scope`, composing `Copy`, by-reference `ScopedBdd` handles (and `Scope::lift` to splice an owned handle in).
- `src/cover/` — `Cover<I, O>`, `Cube<I, O>`, `Minterm<L>`, `Symbols<L>`, `CoverType` (F / FD / FR / FDR), `CubeType`. Sum-of-products / truth-table representation with **automatic dynamic dimension management** (hides the low-level dimension constraint). Multi-output capable. The input/output **label types are generic with no default** — `Symbol` is not privileged; a variable side is either a real label (`Symbol`/`String`/`Arc<str>`/`u32`/…) or the zero-sized `Anonymous` (positional). The label trait family lives in `label.rs` (`Label` + its `Identity`, plus `StringLabel`/`PlaLabel`/`ReconcilableLabel`, all sealed); `symbols.rs` is the per-cover label table (immutable, identity order built eagerly), `minterm.rs` the unified tri-state row, `cubes.rs` the `Cube`. `minimisation.rs` implements the `Minimizable` trait (`minimize`, `minimize_exact`, `minimize_with_config`); `pla/` handles Berkeley PLA file I/O — **reading yields a `PlaCover<S>`** (a sum type whose variant records which `.ilb`/`.ob` label sections the file carried), writing goes through the `PLAWriter` trait.

**CLI**
- `src/bin/espresso.rs` — clap-based CLI behind the `cli` feature; mirrors the C tool's behaviour (this is what regression tests validate).

### Cover types (recur throughout the API)
F = ON-set only; FD = ON-set + don't-cares; FR = ON-set + OFF-set; FDR = all three. Cubes use `Option<bool>` per variable where `None` is a don't-care (`-`).

## Workspace

Two crates: the root `espresso-logic` and `espresso-logic-macros/` (proc-macro providing `expr!`). The macros crate is path-depended from the root.

## Conventions

- British spelling ("minimise", "optimisation") is used in prose and docs throughout; match it in new documentation.
- Doctests are part of the suite (~161 of them) — code examples in `//!`/`///` comments must compile and pass.
