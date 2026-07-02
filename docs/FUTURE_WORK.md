# Future work

Items identified during the 2026-07 repository audit that remain open. The audit's non-breaking
fixes and a follow-up round of breaking API clean-ups (removing the operator-duplicate named
methods, unifying `Bdd` receivers to by-reference, `IntoIterator` quantifiers, rejecting repeated
`.ilb`/`.ob` sections, extending `expr!` operands, tracking `Cargo.lock`) have already landed; what
is listed here is deferred.

## Breaking or behaviour-changing candidates

### CLI reimplementation or removal

The Rust CLI (`src/bin/espresso.rs`, `cli` feature) mirrors the C tool's core behaviour — the
regression suite holds it byte-identical across `-o {f,fd,fr,fdr}` — but not its full option set:
`-e`/`-v` are booleans where the C tool takes arguments, roughly 36 `-D` subcommands plus `-S`/`-r`
are unimplemented, and the shipped 1988 Berkeley man pages document the C option set rather than
this CLI. The divergence is now documented honestly in `docs/CLI.md`; the open decision is whether
to bring the CLI to full parity with the C tool or remove it in favour of the library API. Either
is a deliberate release-level change.

## Follow-ups (no compatibility impact)

### C-side allocation-failure hardening

The FFI boundary now null-checks `sf_new`/`sf_save`/`sf_addset` results, but some vendored C paths
dereference a failed allocation internally before any value reaches Rust:

- `set.c` (`sf_addset`): `REALLOC` result is written through before being returned, so the
  function can never return null on realloc failure — it crashes first. The Rust boundary check is
  defence-in-depth only.
- `cofactor.c` (`cube2list`, `new_cube`): bare `ALLOC` followed by an immediate write, outside the
  fatal guard.

A systematic sweep converting these to checked allocations (routing failures through `fatal()`,
which the guard already catches) would make allocation exhaustion inside the algorithms
recoverable. Keep the standalone C reference binary's observable behaviour identical — the
regression suite is the gate.

### Test coverage

- No test drives `complement` itself into `fatal()` (the `guarded_complement` catch path). The
  heuristic and exact trampoline catches are tested; the third trampoline is line-identical but
  unexercised.
- `tests/test_cli.rs` does not exercise `-o {fd,fr,fdr}` (the shell regression suite covers them;
  the Rust-level CLI tests do not).
- Several modules rely solely on integration tests and doctests rather than unit tests.

### Documentation

- The Emscripten `ERROR_ON_UNDEFINED_SYMBOLS=0` link argument emitted by `build.rs` is
  package-scoped: it applies when this crate is the final link, not to a downstream wasm
  consumer's link. Worth a caveat in the WebAssembly build notes.
- The `# Panics` sections on `minimize`/`minimize_exact` (both API levels) sit after `# Examples`;
  convention places them before.

### Build script

- ASan detection in `build.rs` reads `RUSTFLAGS`-style environment variables and may miss
  sanitizer flags configured via `.cargo/config.toml`; affects the dev-only leak-checking path.
