# Project guidelines

Standing rules for `espresso-logic`. They apply to every change, whoever makes it —
human contributor or AI agent — and they are requirements rather than preferences.

If a task cannot be done without breaking one of these, stop and raise it rather than
working around it. A rule that keeps blocking real work is a rule that is wrong, and
the fix is to change this file, not to route around it quietly.

For what the crate is and how it is laid out, see `CLAUDE.md` and the module docs.
This file covers only the rules that are not readable off the code.

## Language and style

- British spelling in prose and documentation ("minimise", "optimisation",
  "behaviour"). Existing identifiers keep the spelling they already have — do not
  rename `minimize` and friends to match.
- Explain Boolean logic in C/Verilog notation: `!` `~` `&` `|` `^`, with `-` for a
  don't-care. No overlines, no `∀`/`∃`.
- Documentation is technical reference, not a product launch. No pitching against
  alternatives that were never shipped, no permission-granting ("you may now…"), no
  selling adjectives ("cheap", "elegant", "blazing"). Let the code speak.
- The downstream characteriser this crate feeds handles any logic gate. Do not frame
  the crate around NCL or any other specific gate family.

## Public API design

- **Return iterators, not collections.** An API that produces a sequence returns an
  iterator and lets the caller decide whether to collect it, and into what.
- Prefer a genuinely lazy iterator — one that computes per `next()`. Building a
  collection and handing back its `IntoIter` is a collection return wearing a costume:
  it pays the whole cost up front and denies the caller any early exit.
- When a collection genuinely must be returned — a write-once result that is stored,
  shared, or handed out repeatedly — it is `Arc<[T]>`, not `Vec<T>`. `Vec` is for
  long-lived mutable storage only.
- **Take generic traits, not concrete types.** Bind a parameter to what the API
  actually needs — `impl IntoIterator<Item = _>`, `impl AsRef<str>`, `impl Into<_>` —
  rather than to `&[T]`, `Vec<T>`, `&str` or `String`. A concrete type in a public
  signature is a compatibility liability: loosening one later is free, tightening it is
  not.
- String-accepting APIs take `impl AsRef<str>`, for names and free text alike; no
  string type is privileged. A blanket `From<S: AsRef<str>>` is illegal against the
  reflexive `From<T>`, so put the genericity on the function bound and write concrete
  `From` impls.
- The cover layer is generic over its label types with no defaults. `Symbol` is not
  privileged; do not reintroduce it into bounds or as a default type parameter.
- Make misuse unrepresentable in the type system rather than guarding an API with
  runtime asserts or panics.
- Invalid input returns `Err`. It never panics.
- Prefer an additive change over a breaking one. "Additive" means existing code
  compiles unchanged, auto-trait impls (`Send`/`Sync`) included.
- Lock poisoning propagates. Never recover from a poisoned lock.
- Before adding a dedup, sort or buffer, check that duplicates are possible at all and
  that the cost is worth it.

## The C boundary

- New C functions, types or variables exposed to Rust **must** be added to the
  `allowlist_*` calls in `build.rs`. Bindgen is explicit-allowlist only, so anything
  missing is silently absent from `sys.rs` rather than a build error.
- The Rust CLI must produce byte-identical PLA output to the reference C binary across
  every `-o {f,fd,fr,fdr}` variant. Consult `espresso-src/cvrin.c` and the Berkeley PLA
  format spec before changing the parser; the regression suite only covers well-formed
  files, so it will not catch a divergence on malformed input.
- A reachable C `fatal()`/`exit(1)` is fixed in the C itself, via the longjmp guard and
  its setjmp trampolines — keeping the standalone C binary's behaviour intact for
  regression parity.
- Machine word width follows the target architecture (64/32/wasm) through the
  `UINTPTR_MAX` detection in `espresso.h`. Never hardcode `-DBPI=64`.

## Testing

- `./tests/regression_test.sh` is the gate: it checks the Rust CLI byte-for-byte
  against the reference C binary. `quick_regression.sh` is a smoke test, not a
  substitute.
- Doctests are part of the suite. Every example in a `//!` or `///` comment must
  compile and pass.
- Never bend the implementation to keep a test green. If behaviour changed
  deliberately, amend the test and add tests covering the new behaviour. Distinguish a
  test pinning a *decision* (change it freely) from one pinning a *guarantee* (do not).
- Do not hide a hang or a failure behind a timeout or a retry. Diagnose the cause.
- A green build, "pre-existing", or "not gating CI" are not reasons to downgrade a real
  defect.

## Lints and formatting

- Run `cargo fmt` before every commit.
- Fix warnings wherever you find them, including pre-existing ones in files you did not
  otherwise touch. Do not disclaim code as "not mine".
- **A lint is a defect report — fix the code.** Silencing it is not a fix.
- `#[allow(...)]` is a last resort, not an option on equal footing with fixing the
  code. Reach for it only when suppression is extremely well justified: the lint is
  demonstrably wrong about this code, and there is no reasonable way to satisfy it.
  Exhaust the real fix first, including restructuring the code so the lint no longer
  applies.
- Every allow that survives that bar carries a comment directly above it stating what
  the lint asserts and why it is wrong *here*, and is agreed with the maintainer before
  it lands. "It is noisy", "it was already there", and "the real fix is awkward" are
  not justifications. An unexplained allow is worse than the lint it hides, because it
  hides it permanently.
- The crate-level allows in `src/sys.rs` are the bar being described: they cover
  machine-generated bindgen output, which cannot be restructured to satisfy the lints,
  and the reason is written down next to them.

## Git

- Never commit to `master`. Cut a feature branch before implementation starts.
- A commit message describes the delta against the previous commit — not the drafts and
  dead ends a reader never saw, and never internal planning labels ("Phase 2",
  "Group A", "Wave 1").
- No `Claude-Session` trailer, and no AI-attribution line of any kind.
- Push the branch's true history. Do not rebase, linearise or squash it into a tidier
  shape than it had.
- A pull request description covers every commit on the branch (`git log base..HEAD`),
  not just the most recent one.
- Stage `Cargo.lock` alongside `Cargo.toml` on a version bump — its own-package version
  changes, and a stale lock breaks `--locked` builds.
- Destructive commands take the minimal scope asked for. "Recoverable from the reflog"
  is not a reason to delete something.

## Changelog and versioning

- Update `CHANGELOG.md` in the same change that alters user-visible behaviour, rather
  than sweeping up later.
- The changelog records deltas *between releases*. Do not log refinements to an API
  that is itself new in the same unreleased version.
- Breaking means users must edit their code to upgrade. Widening `&[S]` to
  `impl IntoIterator`, or a concrete type to a trait, is not breaking.
- A major bump is earned by depth of impact on how the crate is used, weighted by how
  prominent the API is — not by the mere presence of a breaking change. A niche
  function gaining a `Result` is a minor.
- Bump the version only after all work intended for that release has merged.

## Project context

- `origin` (`marlls1989/espresso-logic`) is the canonical release target. The remote
  named `upstream` (`classabbyamp`) holds the archived C sources only — never push
  there.
- `marlls1989/cellsmith` is the reference downstream consumer. Price every public API
  change against its diff; that it still compiles is the release acceptance test.
- Author contact in crate metadata and commits is `marcos.sartori@ncl.ac.uk`.
