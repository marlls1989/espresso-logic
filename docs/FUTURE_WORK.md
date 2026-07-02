# Future work

Items identified during the 2026-07 repository audit that were deliberately left out of the
non-breaking fix branch. They fall into two groups: changes that would break the public API or
alter accepted behaviour (candidates for a major or feature release), and smaller follow-ups that
need no compatibility decision but were out of the audit's scope.

## Breaking or behaviour-changing candidates (major/feature release)

### API naming and ownership consistency

`Bdd::complement(self)` consumes its receiver while `BoolExpr::not(&self)` borrows; similarly
`Bdd::ite` takes its arguments by value where `and`/`or`/`xor` take them by reference. Unifying the
receiver/argument conventions changes signatures and is a semver-major change. Decide one
convention (by-reference, matching the operator impls, is the natural fit for refcounted handles)
and apply it across the BDD and expression layers in one release.

### `forall` / `exists` argument type

Both take `&[S]`. `impl IntoIterator<Item = S>` would accept more callers, but changing the bound
can break type inference at existing call sites, so it is grouped here rather than with the
additive polish.

### CLI/C option divergence

The Rust CLI (`src/bin/espresso.rs`, `cli` feature) mirrors the C tool's core behaviour — the
regression suite holds it byte-identical across `-o {f,fd,fr,fdr}` — but not its full option set:

- Rust `-e` and `-v` are booleans where the C tool takes an argument (`-e <opt>`, `-v <type>`).
- Roughly 36 C `-D` subcommands, plus `-S` and `-r`, are unimplemented.
- The shipped Berkeley man pages (`man/espresso.1`, 1988) document the C option set, not the
  Rust CLI.

Bringing the CLI to parity (or explicitly documenting the supported subset and refreshing the man
page) is a coherent feature-release work item. Changing `-e`/`-v` to take arguments alters existing
command lines.

### Repeated `.ilb`/`.ob` sections silently last-wins

The PLA reader now rejects duplicate names *within* a label section and any `.i`/`.o`
redeclaration, but a second `.ilb`/`.ob` *line* still silently replaces the first. Rejecting it
would be consistent with the new strictness; it also rejects files currently accepted, so it is a
behaviour change to schedule deliberately (new `PLAError` variant, same pattern as the duplicate
directives).

### `expr!` graft-operand syntax gaps

Inside `expr!`, `&foo` parses as a dangling AND rather than a reference operand, and macro-call
operands (`foo!()`) are rejected. A real fix extends the accepted grammar, which changes what the
macro compiles — a feature-release item. The current behaviour is a documented limitation; the
audit improved only the error message.

### Tracking `Cargo.lock`

The crate ships a binary (the `cli` feature), for which the Cargo guidance is to commit
`Cargo.lock`; it is currently gitignored. Tracking it changes contributor workflow and CI caching
rather than the API, but should be decided once and documented.

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
