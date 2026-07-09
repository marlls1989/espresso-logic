# CLAUDE.md

Guide for Claude Code (claude.ai/code) working in this repo.

## What this is

Rust bindings to UC Berkeley Espresso heuristic logic minimiser. `build.rs` compiles vendored C (`espresso-src/`) and generates FFI bindings with bindgen; Rust crate layers safe, thread-safe, idiomatic APIs on top. Published as `espresso-logic` on crates.io (now v5.0.x).

## Common commands

```bash
cargo build                      # builds C + bindgen + Rust
cargo build --features cli       # also builds the `espresso` CLI binary (needs clap)
cargo test                       # unit + integration + ~209 doctests
cargo test --test test_integration         # single integration test file
cargo test --test test_memory_safety -- --nocapture --test-threads=1
cargo test name_of_test          # single test by name substring
cargo run --example xor_function # run an example (see Cargo.toml [[example]] list)
cargo bench                      # criterion benchmarks (benches/pla_benchmarks.rs)

./tests/quick_regression.sh      # 4 cases, ~1s: Rust CLI vs C CLI
./tests/regression_test.sh       # ~363 cases, ~45s; builds C binary via espresso-src/Makefile
ESPRESSO_REF_BPI=32 ./tests/regression_test.sh   # cross-width: native-width Rust CLI vs 32-bit C reference
ESPRESSO_BPI=32 cargo test                       # build crate C+bindings at 32-bit on a 64-bit host
./scripts/check_leaks_macos.sh   # leak checks
```

Regression tests compare Rust CLI against reference C binary (`./bin/espresso`), built from `espresso-src` with `make`. Rust CLI must produce byte-identical PLA output to C across all `-o {f,fd,fr,fdr}` format variants.

## Build system notes

- `build.rs` compiles all `espresso-src/*.c` **except `main.c`** as library, runs `lalrpop::process_root()` for expression grammar, runs bindgen against `thread_local_accessors.h`. Bindgen uses explicit `allowlist_*` — **new C functions/types/vars exposed to Rust must be added to allowlist in `build.rs`** or they miss from `sys.rs`.
- C compiled with `-std=c11`; thread safety relies on C11 `_Thread_local` storage (see `thread_local_accessors.c/.h`), each thread own copy of Espresso global cube state.
- WebAssembly target is `wasm32-unknown-emscripten` (NOT `wasm32-unknown-unknown` — C code needs libc). Needs Emscripten SDK with `EMSDK` set.

## Architecture

Two API levels over C FFI. Crate-root re-exports defined in `src/lib.rs`.

**FFI layer**
- `src/sys.rs` — raw bindgen output (`include!` of `OUT_DIR/bindings.rs`), all `unsafe`. No direct use outside wrappers.
- `src/espresso/` — low-level safe wrapper (`Espresso`, `EspressoCover`, `EspressoConfig`). Thread-local singleton with reference counting. **Critical constraint:** all covers/instances on a thread must share same dimensions (#inputs, #outputs); creating different dimensions fails until every `EspressoCover` dropped. Use this layer for access to separate ON/DC/OFF-set covers, or lower per-call overhead (high-level API also validates cover and rebuilds output `Cover`): measured ~10–14% faster on small covers but only ~1–5% / within noise on large ones — see `api_overhead` group in `benches/pla_benchmarks.rs`.

**High-level layer (recommended/default API)**
- `src/expression/` — `BoolExpr`, an **owned, syntactic** Boolean expression: flat reverse-Polish `Token` stream (`rpn.rs`), no canonicalisation (`a & b` and `b & a` are distinct values). Composed via `expr!` macro (from `espresso-logic-macros` proc-macro crate) and `BoolExpr::build` arena builder (`builder.rs`, `ExprBuilder`/`Expr`), parsed from text via lalrpop grammar (`parser`, `bool_expr.lalrpop`); `ast.rs` (`ExprNode` plus `fold`/`fold_with_context`), `operators.rs`, `display.rs`, `factorization.rs` round it out.
- `src/bdd/` — **canonical BDD layer**, where semantic operations live. `Bdd<B, C>` (owned, refcounted `Clone` handle; `handle.rs`) minted by `BddBuilder<B, C>` (`builder.rs`) parameterised by two orthogonal type parameters: sealed `Brand` `B` (`brand.rs`, uniqueness only) and `ManagerCell` `C` (`LocalCell`/`SyncCell`, storage backend and thread-safety). Builders come from `bdd_builder!` / `sync_bdd_builder!` macros — **no process-global manager**; each builder owns own. Operations: `&`/`|`/`^`/`!`, `ite`, `restrict`/`restrict_many`/`cofactor`/`forall`/`exists`, `equivalent_to`, `evaluate`/`evaluate_fast`, `compose`/`compose_map`, `cover`/`cover_fr` (on-set / on+off-set extraction; `to_cubes` is deprecated alias of `cover`), `cover_over`/`cover_over_fr` (extract + universal projection onto variable subset), `primes` (all prime implicants), `maximize`/`maximize_fr`, `minimize`/`minimize_fr`, `to_expr`, `fold`, `variables`; `Bdd::builder` recovers builder onto stored handle's manager. `scope.rs` adds `BddBuilder::scope`, composing `Copy`, by-reference `ScopedBdd` handles (and `Scope::lift` to splice owned handle in), now mirroring `restrict`/`restrict_many`/`restrict_to` and `compose`/`compose_map` on the scoped layer. `manager.rs`/`manager_cell.rs` hold the manager internals (`BddManager`, keyed on `Symbol`; the sealed `ManagerCell` storage cells `LocalCell`/`SyncCell`; the crate-internal `BddOps` engine trait). `batch.rs` adds the streaming `Composer` trait (`compose`/`compose_map` over an iterator of handles, sharing a per-batch cache).
- `src/cover/` — `Cover<I, O>`, `Cube<I, O>`, `Minterm<L>`, `Symbols<L>`, `CoverType` (F / FD / FR / FDR), `CubeType`. Sum-of-products / truth-table representation with **automatic dynamic dimension management** (hides low-level dimension constraint). Multi-output capable. Input/output **label types generic with no default** — `Symbol` not privileged; variable side is either real label (`Symbol`/`String`/`Arc<str>`/`u32`/…) or zero-sized `Anonymous` (positional). Label trait family lives in `label.rs` (`Label` + its `Identity`, plus `StringLabel`/`PlaLabel`/`ReconcilableLabel`, all sealed); `symbols.rs` is per-cover label table (immutable, identity order built eagerly), `minterm.rs` the unified tri-state row, `cubes.rs` the `Cube`. `minimisation.rs` implements `Minimizable` trait (`minimize`, `minimize_exact`, `minimize_with_config`) plus `Cover::maximize` (arg-free inverse of minimisation), `Cover::over_vars` (widen + universal projection onto variable subset), `Cover::primes` (complete prime-implicant set, backing `over_vars`); `pla/` handles Berkeley PLA file I/O — **reading yields `PlaCover<S>`** (sum type whose variant records which `.ilb`/`.ob` label sections file carried), writing goes through `PLAWriter` trait.

**CLI**
- `src/bin/espresso.rs` — clap-based CLI behind `cli` feature; mirrors C tool behaviour (this is what regression tests validate).

### Cover types (recur throughout API)
F = ON-set only; FD = ON-set + don't-cares; FR = ON-set + OFF-set; FDR = all three. Cubes use `Option<bool>` per variable where `None` is don't-care (`-`).

## Workspace

Two crates: root `espresso-logic` and `espresso-logic-macros/` (proc-macro providing `expr!`). Macros crate path-depended from root.

## Conventions

- British spelling ("minimise", "optimisation") used in prose and docs throughout; match it in new docs.
- Doctests part of suite (~209 of them) — code examples in `//!`/`///` comments must compile and pass.
