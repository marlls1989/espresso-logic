# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **`BoolExpr` is now genuinely generic over its stored label type** (`BoolExpr<V = Symbol>`): every
  constructor and inherent method — `var`, `constant`, `parse`, `build`, `Default` — lives on the
  generic `impl`, so the stored label type follows the binding, turbofish, or consuming context rather
  than being fixed to `Symbol`. `let e: BoolExpr = BoolExpr::var("a")` still resolves to `Symbol` via
  the type-level default, and `BoolExpr::<String>::var("a")` selects `String` directly. A bare
  `let e = BoolExpr::var("a")` that is neither annotated nor consumed no longer infers `Symbol` and
  needs an annotation or turbofish. The public `BoolExpr::relabel` method is removed; construct the
  target label type directly (annotate, turbofish, or `"a & b".parse::<BoolExpr<String>>()`).
- `Symbol` reshaped from 24 bytes to 16 bytes on 64-bit (8 bytes on 32-bit), sized *exactly* to its
  `Arc<str>` container rather than to `String`'s. The inline capacity is still derived at compile
  time and guarded by compile-time size asserts, now superseding 5.6.0's `size_of::<String>() - 2`
  derivation with `size_of::<Arc<str>>() / 2 - 1` (7 bytes on 64-bit, 3 bytes on 32-bit).
  Size/performance change only — no public API surface (`INLINE_CAP` is crate-private). One residual
  effect: variable names of 8..=22 bytes that were stored inline under 5.6.0 now intern to the heap
  instead — the content is identical, only the storage location changes.
- **Corrective redesign of the stored-label type parameter (5.6.0 yanked).** 5.6.0 shipped the label
  type `S` as a phantom marker over a `Symbol`-keyed manager; this release reverts that and stores the
  label genuinely. `Bdd`, `BddBuilder` and `Scope` collapse back to **two** type parameters (`Brand`,
  `ManagerCell`): the stored label now lives on the storage cell — `LocalCell<S>` / `SyncCell<S>`,
  surfaced as the `ManagerCell::Label` associated type — so the manager keys variable names by that type
  directly, and every label-producing output (`variables`, the `cover`/`primes`/`maximize`/`minimize`
  families, `to_expr`) comes back as `ManagerCell::Label` with no relabelling step. The `relabel` methods
  are removed from `Bdd`, `BddBuilder` and `BoolExpr`; build under the target label type directly (an
  annotated binding, a turbofish, or `parse`). See also the `Symbol` reshape above.

  Only 5.5.x backward-compatibility is a goal here (5.6.0 was withdrawn). The residual 5.5 → this-release
  breaks, all logged and none silently accepted:

  - **Label-agnostic builder and cover/expression bindings need a one-time type annotation.** A binding
    that neither pins the label nor consumes a labelled output can no longer fall through to `Symbol`,
    because the cell's stored label is an inference placeholder (`LocalCell<_>`). Add the minimal
    annotation — `let b: BddBuilder<_, LocalCell> = bdd_builder!();` (which resolves to `Symbol` via the
    cell's own default) — or pin by consumption (`let vars: Vec<Symbol> = f.variables().collect();`).
    This release's sweep added roughly 32 builder annotations and 67 cover/expression annotations across
    the doctests, unit and integration tests, examples and doc guides; the parallel `BoolExpr<V>` change
    above needs the same one-time annotation at bare-path expression sites.
  - **`Cover::to_expr` / `to_expr_by_index` / `to_exprs` are now bounded `I: StringLabel` and return
    `BoolExpr<I>`** (previously `I: AsRef<str>`, always returning `BoolExpr<Symbol>`). A `Cover<String, _>`
    now yields `BoolExpr<String>`, carrying the cover's own input label rather than collapsing to `Symbol`.
  - **`Cover::add_bdd` / `add_expr` are now defined on `Cover<L, L>`** and require the handle's cell label
    to equal the cover's label `L` (`ManagerCell<Label = L>`). Adding a handle built under a different
    label type no longer type-checks — build or recover the handle under `L` first.
  - **Cell label types additionally require `Ord + Borrow<str>`** (and `Send + Sync` for `SyncCell`).
    `Symbol`, `String`, `Arc<str>` and `Box<str>` all qualify; `Cow<'_, str>` qualifies too, since the
    standard library provides `impl Borrow<B> for Cow<'_, B>`.

## [5.6.0] - 2026-07-08

### Added

- **Stored-label type parameter on the expression and BDD layers**: `BoolExpr<S = Symbol>`,
  `Bdd<B, C, S = Symbol>`, `BddBuilder<B, C, S = Symbol>`, and `Scope<'s, B, C, S = Symbol>`, all
  bounded by the existing `StringLabel`. Label-producing outputs — `variables`, the
  `cover`/`cover_fr`/`primes`/`maximize`/`maximize_fr`/`minimize`/`minimize_fr` families, `to_expr` —
  now follow `S`. New `relabel` conversions carry a value from one stored label type to another:
  `BoolExpr::relabel` re-interns each token's variable name, while `Bdd::relabel`/`BddBuilder::relabel`
  are free cell rewraps (variable names live in the manager as `Symbol` regardless of the phantom `S`).
  Non-`Symbol` construction goes through the annotation-driven `parse::<BoolExpr<S>>()` (`str::parse`)
  and `bdd_builder!().relabel::<S>()`; the bare-path constructors (`var`/`constant`/`build`/`Default`/
  `expr!`/`bdd_builder!`/`sync_bdd_builder!`/`scope`) stay concrete on `Symbol` so existing call sites
  need no annotation. `BddBuilder::build`/`minimize`, `Scope::build`/`lift`, `Cover::add_bdd`/`add_expr`
  and the `Cover`/`Bdd`/`BoolExpr` `From` bridges now accept an operand under any string-labelled `S`,
  not just `Symbol`. Additive and non-breaking: every existing call site elaborates the same defaults
  and compiles unchanged. One asterisk — an unannotated `.parse()` fed straight into the now-generic
  `build`/`minimize` no longer has a type to infer from and needs a turbofish, e.g.
  `builder.build(&"a & b".parse::<BoolExpr<String>>()?)`.
- `Bdd::restrict_to` (mirrored on `ScopedBdd`): restrict a function to the subspace pinned by a
  `Minterm` — every field the minterm fixes is applied, a don't-care (or a variable the minterm does
  not carry) leaves that variable free, and an unknown name is ignored. Accepts any `StringLabel`.
  `Bdd::evaluate` now builds its residual through this path. Internally the single- and
  multi-variable restrict engines are unified into one minterm-keyed operation, so the two can no
  longer drift.
- `bdd::Composer::compose` / `compose_map` — apply one substitution across an iterator of handles
  (`Bdd` or `ScopedBdd`) as a lazy iterator that shares a single short-lived memo, so functions
  overlapping in structure are composed once. This is the efficient path for substituting a variable
  across many related functions; it recovers the cross-function reuse the removed per-manager compose
  cache used to give, without that cache's manager-lifetime growth.

### Changed

- `compose` and `compose_map` (on both `Bdd` and `ScopedBdd`) now share one engine: single-variable
  `compose` routes through `compose_map` instead of a separate fused pair-walk, so the two can no
  longer drift. This drops the persistent per-manager compose cache added in 5.4.0 — repeated
  identical compositions now re-walk the function rather than returning a cached result. Results and
  the public API are unchanged (hash-consing still prevents duplicate nodes, and the ITE cache still
  carries cross-operation reuse); only a workload composing the same substitution repeatedly loses
  that saved re-walk — recovered within a batch by the new `bdd::Composer`.
- Vendored Espresso packed-cube machine word now follows the native word width: 64-bit words on
  64-bit targets; 32-bit words on 32-bit targets (including wasm32-unknown-emscripten after
  cross-compilation). Derived at compile time from pointer width in `espresso.h` rather than a
  hardcoded 32-bit BPI. Performance change only — no public API surface. Minimised covers are
  byte-identical across widths; cross-width regression coverage (ESPRESSO_REF_BPI=32
  ./tests/regression_test.sh) runs in CI.
- `Symbol`'s inline capacity now follows the native platform: derived at compile time from
  `size_of::<String>()` rather than a hardcoded constant, and guarded by a compile-time size-class
  assertion. 22 bytes on 64-bit (unchanged); 10 bytes on 32-bit. Size/performance change only — no
  public API surface (the constant is crate-private).

### Fixed

- `wasm32-unknown-emscripten` now links and runs, not just compiles. The vendored C is built with
  `-fwasm-exceptions` so its `setjmp`/`longjmp` fatal-error trampoline uses the same wasm
  exception-handling model rust's target links with; previously the mismatched model left the
  `invoke_` trampolines unresolved at link. The bench-only `criterion` dev-dependency is now scoped
  to non-wasm targets (its default `rayon` feature does not compile for wasm/wasi), so the crate's
  own test suite builds on the target. Verified under Node: the word width self-detects to 32-bit and
  minimisation (including fatal-error recovery through the guard) runs.

## [5.5.0] - 2026-07-06

### Added

- `Bdd::restrict_many` (mirrored on `ScopedBdd`): simultaneous multi-variable Shannon cofactor,
  one iterative walk interning only residual nodes, equal to chaining restrict in any order;
  absent names/empty assignment no-op; a repeated name takes its last entry.
- `ScopedBdd::restrict`: the single-variable cofactor now mirrored on the scoped handle, closing a
  parity gap with the owned `Bdd::restrict`.
- `Bdd::evaluate_fast`: write-free probe deciding whether a possibly-partial assignment forces a
  constant; folds over a read-only snapshot, no exclusive borrow, interns nothing.

### Changed

- `Bdd::evaluate` now layers on both — a determined assignment is decided without an exclusive
  borrow, a partial one builds its residual in a single pass instead of one restrict per fixed
  variable; public contract unchanged.

## [5.4.0] - 2026-07-03

### Added

- `Bdd::compose` and `Bdd::compose_map` (mirrored on `ScopedBdd`), native composition
  primitives on the canonical BDD layer: `compose` substitutes a Boolean function for a
  variable (`f[var := g]`) and `compose_map` performs several substitutions at once in a
  single fused traversal — every substitution reads the original function, so
  `f.compose_map([("a", &b), ("b", &a)])` swaps the two variables. Both are single-pass
  engines (not `ite`-of-`restrict` chains), evaluated iteratively so a deep variable
  ordering cannot overflow the call stack, and memoised through a persistent per-manager
  compose cache alongside the ITE cache. Substituting for a name the function does not
  test — or passing an empty map — is a no-op; a substituting function may mention any
  variables, including ones ordered above the substituted variable, and `compose_map`
  takes the last entry for a repeated name.

## [5.3.0] - 2026-07-03

### Added

- `Minterm::is_vacuous`, a public predicate reporting whether any variable in the row holds the
  empty value set (`?`/`00`) — the lattice opposite of the don't-care `-`. A cube with even one such
  field denotes the empty set (covers no assignment); such cubes are dropped before minimisation.
- `InputField`, the faithful, four-state value of a cube's input field (`Zero`/`One`/`DontCare`/
  `Empty`), alongside a matching read/write surface on `Minterm`: `field_at`/`set_field_at` (by
  position), `field_of`/`set_field_of` (by label), and `fields` (a `FieldsIter` iterator). Unlike the
  existing `value_*`/`iter` family, which folds the empty literal (`?`) down to don't-care because
  `Option<bool>` has no fourth state, this view reads and writes `?` verbatim — `set_field_at`/
  `set_field_of` can write `InputField::Empty`, making the containing cube vacuous. `Display`/`Debug`
  now render the faithful view too, so `?` shows up rather than being folded to `-`. `InputField`
  also carries its own four-state Boolean algebra: the bitwise operators `& | ^` and complement `!`,
  the scalar form of the `Minterm` element-wise operators, computing the value-set image on each
  field (`Empty`/`?` propagates as the empty set).
- `Minterm`'s bitwise operators `& | ^` and complement `!`, combining rows element-wise as
  value-set image operations — each field is read as the values it allows (`0`={0}, `1`={1},
  `-`={0,1}, empty `?`={}) and the result is `{ a op b : a ∈ x, b ∈ y }`. On non-empty fields this
  restricts to three-valued (Kleene) logic where `None` is the don't-care value `-`: AND shortcuts on
  `0`, OR on `1`, XOR/complement propagate `-`. The empty literal `?` denotes the empty set and
  propagates — `? op x = ?` and `!? = ?` — making the containing cube vacuous. Operands are
  auto-aligned by variable identity — a variable present in only one operand reads as `-` — so the
  operation is independent of header ordering (`a op b == b op a`). This is truth-value logic, not
  cube/set intersection.
- `Cover::over_labels`, the label-value counterpart of `Cover::over_vars`: it names the target
  variable set by input label value (any `NamedLabel`, e.g. `u32`) rather than by string, driving the
  same universal projection.
- **`Cube` can now be assembled from separately-built halves.** `Cube::new` — pairing a pre-built
  input `Minterm` with a per-output `OutputSet` — is now public, and both halves gain the full
  constructor set: `Minterm::labeled`/`with_labels` and `OutputSet::labeled`/`with_labels` build a
  labelled half from `(label, value)` / `(name, value)` pairs (rejecting duplicate labels with
  `DuplicateLabel`), and `OutputSet::anonymous` builds a positional one (mirroring the existing
  `Minterm::anonymous`). The label types flow through `Cube::new`: two labelled halves compose into a
  labelled `Cube<I, O>`, two anonymous halves into an anonymous one.
- `NamedLabel`, a sealed marker trait for labels whose alignment identity is a real name/value
  (as opposed to `Anonymous`'s positional identity). Every `Ord + Eq + Hash + Clone` label qualifies
  via a single blanket impl.
- `Cover::rename`/`rename_inputs`/`rename_outputs`, string-name forms of the `relabel*` methods:
  they take `&str`-like names (`impl IntoIterator<Item: AsRef<str>>`) and convert each into the
  chosen `StringLabel` target type. Type-changing conversions to non-string labels stay on
  `relabel*`.
- `Minterm::project_to`, `project_to_labels`, and `project_to_arity`, `Arc`-free faces for
  structurally re-homing a minterm onto a target variable set. Variables shared by the minterm and
  the target keep their value (aligned by identity, reordered as needed), target-only variables come
  in as don't-care, and self-only variables are dropped. `project_to` names the target set with any
  `&str`-like type, `project_to_labels` with label values (excluded from `Minterm<Anonymous>` by the
  `NamedLabel` bound), and `project_to_arity` projects an anonymous minterm by arity — widening with
  trailing don't-cares, dropping trailing positions, or a no-op at equal arity. A repeated target
  variable is deduplicated, keeping the first occurrence.
- `Cover::labeled`, the label-value dual of the string-name `Cover::with_labels`: builds a cover
  from label values directly, so it works for any `Label` type, not just `StringLabel`s.
- `IndexOutOfRange` and `LabelNotFound`, error types backing the in-place setter methods on
  `Minterm`/`OutputSet`: `IndexOutOfRange` reports a positional setter given an index at or past
  the row's arity, `LabelNotFound` a by-label setter given a label absent from the row.
- `Minterm::set_value_at`/`set_value_of`, in-place counterparts to `value_at`/`value_of`: they mutate
  the packed word buffer copy-on-write (via `Arc::make_mut`, so a shared buffer is cloned first and a
  uniquely-held one is mutated directly), returning `IndexOutOfRange`/`LabelNotFound` respectively for
  an out-of-range index or an absent label.
- `OutputSet::value_of`, the by-label counterpart of `value_at` (`false` if the label is absent),
  and two copy-on-write in-place setters: `set_value_at` (positional, `Err(IndexOutOfRange)` past
  the arity) and `set_value_of` (by-label, `Err(LabelNotFound)` if absent, even when clearing).
- `OutputSet`'s binary bitwise operators `&`, `|`, `^`, and the unary complement `!` (with named
  `and`/`or`/`xor`/`not` methods). Outputs are two-state, so these are plain bitwise operations on the
  packed membership bitmap — **binary, not `None`-aware**, unlike the tri-state `Minterm` operators.
  The binary operators align outputs by variable identity (widening onto the identity union of the two
  headers, an output present in only one operand reading as unasserted on the absent side); the
  complement flips every output over the row's own arity.

### Changed

- `Minterm` now treats the empty literal `?` as the empty cube: equality, ordering and hashing
  compare by denoted set, so all vacuous minterms are equal and sort after non-vacuous ones.
  `Minterm::hamming_distance`/`disagreement` follow the same reading, now using Espresso's cube
  distance — a `?` field counts as a disagreement — restoring `is_disjoint_with ⟺ distance > 0`.
  `Minterm::is_subset_of` follows it too: `∅ ⊆ X` for every `X`, and `X ⊆ ∅` only if `X` is itself
  vacuous; `is_disjoint_with` already read this way unchanged — a vacuous cube is disjoint from
  everything, including itself.
- **Breaking:** `Symbols` and the `Arc<Symbols>`-taking methods are no longer part of the public API.
  `Symbols` itself, `Minterm::from_symbols`/`symbols`/`project_onto`/`expand_over`,
  `OutputSet::symbols`, and the `DuplicateSymbol` error are now crate-internal. Construct minterms and
  output sets with `Minterm::labeled`/`with_labels`/`anonymous` (and the `OutputSet` equivalents);
  read a header's labels with `vars()`; re-home a minterm with
  `project_to`/`project_to_labels`/`project_to_arity`; expand a cube with `Cube::expand_to`; and
  relabel a cover with `relabel*`/`rename*`.
- **Breaking:** `Cover::with_labels` now returns `Result<Self, DuplicateLabel>` and takes
  `impl IntoIterator` label lists. Existing slice-reference call sites (e.g. `&["a", "b"]`) continue
  to compile unchanged.
- **Breaking:** `Cover::relabel`/`relabel_inputs`/`relabel_outputs` now take label iterators
  (`impl IntoIterator<Item = L>`) rather than `Arc<Symbols>`, and return `RelabelError` — either an
  arity mismatch (`RelabelError::Arity`, wrapping `ArityMismatch`) or a duplicate label
  (`RelabelError::Duplicate`, wrapping `DuplicateLabel`).
- `Cover::over_vars`, `Bdd::cover_over`, and `Bdd::cover_over_fr` now take
  `impl IntoIterator<Item = S>` instead of `&[S]` and deduplicate a repeated variable — the argument
  names a variable *set*, so `["a", "b", "a"]` and `["a", "b"]` behave identically. These remain
  infallible, and ordinary slice-reference call sites (e.g. `&["a", "b"]`) continue to compile
  unchanged.
- `Cover::input_labels`/`output_labels` are now available for every label type, not just
  string-labelled covers.
- `Cover::maximize` now requires the output label type to be `Label` (previously `Clone`), a
  consequence of `OutputSet`/`Cube` equality now aligning by label identity: deduplicating expanded
  cubes needs `Cube: Eq + Hash`, which now needs `O: Label`. This affects only generic code bounded
  `O: Clone` — every concrete cover already satisfies `O: Label`, since building one requires it.

### Fixed

- `OutputSet` equality, ordering, and hashing now align by output-label identity (like `Minterm`), so
  output sets that assert the same labels compare equal regardless of column order; previously they
  were compared positionally, which could report two logically-equal sets as unequal once their
  headers differed.
- User labels that repeat an identity no longer silently collapse two columns onto one and drop a
  value. The labelled cube/cover constructors reject a repeated label with `DuplicateLabel`, while the
  variable-set operations (`Cover::over_vars`, `Cube::expand_to`, `Bdd::cover_over`/`cover_over_fr`)
  treat their argument as a set and deduplicate a repeated variable.

## [5.2.0] - 2026-07-02

### Added

- **BDD on+off-set cover extraction.** `Bdd::cover_fr`, `Bdd::maximize_fr`, and `Bdd::minimize_fr`
  extract the on-set together with the off-set as a `CoverType::FR` cover (off-set tagged
  `CubeType::R`), letting Espresso minimise against the exact off-set rather than a recomputed
  complement. `Bdd::cover` is the on-set-only counterpart (the renamed `to_cubes`).
- **Universal variable projection.** `Cover::over_vars` re-bases a cover onto an explicit set of
  variable names: variables it does not mention are widened in as don't-cares, and variables it drops
  are eliminated by **universal** projection (the assignments that force the output for *every* value
  of the eliminated variables). `Bdd::cover_over` / `Bdd::cover_over_fr` extract and project in one
  step. The on- and off-sets are derived independently, so they stay orthogonal but need not be
  complementary — where the output still depends on an eliminated variable, that assignment is left
  undefined (a genuine don't-care gap; the Muller C-element case).
- **Complete prime-implicant generation** at every layer — `Espresso::primes` / `Espresso::try_primes`,
  `Cover::primes`, and `Bdd::primes` return *all* prime implicants (the C tool's `-Dprimes`), not the
  reduced, irredundant cover `minimize` yields. This also backs `over_vars`.

### Deprecated

- **`Bdd::to_cubes` is renamed to `Bdd::cover`.** The old name still works but is deprecated; the
  low-level `EspressoCover::to_cubes` is unaffected.

### Fixed

- **Subset variable extraction now projects universally, not existentially.** Extracting a cover over
  a strict subset of a function's variables (the old `maximize(vars)` with a subset `vars`) merely
  dropped the excluded variables' literals and de-duplicated — an **existential** projection — so an
  `FR` cover's on- and off-sets could spuriously overlap. It is now a **universal** projection
  (`∀excluded`): each side is derived independently from the complete prime set, keeping only the
  primes that constrain nothing outside `vars`. On- and off-sets stay orthogonal, and where the
  output genuinely depends on an eliminated variable that assignment is left undefined instead of
  being forced into both sets (the Muller C-element gap). Exposed through the new `Cover::over_vars` /
  `Bdd::cover_over` / `Bdd::cover_over_fr`.
- **The PLA reader rejects duplicate `.ilb`/`.ob` labels.** A `.ilb`/`.ob` section naming the same
  variable twice (e.g. `.ilb a a b`) used to build a `Symbols` table that silently violated its
  documented uniqueness invariant, misaligning later lookups (`merge`/`relabel`/`push`). It is now
  rejected at parse time with the new `PLAError::DuplicateLabel` variant.
- **The PLA reader rejects a mid-file `.i`/`.o` redeclaration.** A repeated `.i`/`.o` directive after
  cube data had already been read used to overwrite the declared dimensions while the accumulated
  cube character stream was still split at the old width, mis-splitting subsequent cubes. Any second
  `.i`/`.o` declaration — before or after cube data — is now rejected with the new
  `PLAError::DuplicateInputDirective` / `PLAError::DuplicateOutputDirective` variants.

- **Invalid input to the low-level minimiser no longer aborts the process.** The vendored C core
  reports unrecoverable conditions by calling `fatal()`, which printed to stderr and `exit()`ed.
  Some of these are reachable from safe Rust — most notably an explicit OFF-set that overlaps the
  ON-set (a non-orthogonal cover) driving `expand`/`complement` into `fatal()`. A thread-local
  recovery point now catches such a fatal and turns it into an error instead of killing the process;
  the thread stays usable for further minimisations. The standalone C reference binary is unchanged.
- **Allocation failure during instance creation errors instead of panicking.** `Espresso::try_new`
  used to `panic!` if the `part_size` array could not be allocated, leaving the thread-local C cube
  state partially written. It now surfaces the new `InstanceError::AllocationFailure` variant, and
  the cube state is restored first, so a later call on the same thread is unaffected.
- **Parse-error positions are consistent byte offsets.** `ExpressionParseError.position` is now
  extracted structurally from lalrpop's `ParseError` variants as a byte offset into the input; it
  was previously scraped from the human-readable message text, mixing a 0-indexed column with a raw
  byte offset depending on which message pattern matched.
- **MSVC builds no longer fail on the C standard flag.** `build.rs` now probes both `-std=c11` and
  `/std:c11` and uses whichever the active C compiler supports (C11 `_Thread_local` is required
  either way).
- **C allocation failures no longer dereference null.** Results of `sf_new`, `sf_save`,
  `sf_addset`, and the guarded minimisation/complement calls are now null-checked at the FFI
  boundary and panic with a clear "out of memory" message instead of crashing on a null pointer.
- **The Emscripten `ERROR_ON_UNDEFINED_SYMBOLS=0` setting is applied at link time.** `build.rs`
  passed it as a C compile flag, where `-s KEY=VALUE` is a no-op; it is now emitted as a linker
  argument for the `wasm32-unknown-emscripten` target, where emcc actually honours it.

### Changed

- **`Cover::maximize` and `Bdd::maximize` are now argument-free** (breaking). Maximisation is the true
  inverse of minimisation — it expands a cover to minterms over its *own* variables — so it no longer
  takes a variable set. Re-basing onto a different variable set now lives in `Cover::over_vars` /
  `Bdd::cover_over` (see Added). Callers passing the function's own support should drop the argument;
  callers passing a different set should switch to `over_vars` / `cover_over`.
- **Clearer `expr!` diagnostics for invalid operands.** The macro now reports an invalid operand
  with a message naming the accepted operand forms (a string literal, the constants `0`/`1`, a
  parenthesised expression, or an expression yielding a `BoolExpr`) at the offending token, instead
  of syn's generic "expected identifier".
- **PLA reading streams the input.** `from_pla_reader` iterates the reader line by line instead of
  buffering the whole file into memory first; an I/O error is reported at the point in the stream
  where it occurs.
- **`Cargo.lock` is now tracked.** The crate ships a binary behind the `cli` feature, so a committed
  lockfile gives it and CI reproducible dependency resolution. Dependents are unaffected — Cargo
  ignores a library dependency's lockfile.
- **The bundled CLI documentation now records the CLI's real divergence from the C tool.** `docs/CLI.md`
  (embedded into the crate docs) previously claimed option parity that does not exist; it now states the
  supported option subset and notes that the shipped 1988 Berkeley man pages describe the C tool rather
  than this CLI.
- **The PLA reader rejects a repeated `.ilb`/`.ob` label section.** A second `.ilb` or `.ob`
  directive used to silently overwrite the labels declared by the first; it is now rejected with the
  new `PLAError::DuplicateInputLabelDirective` / `DuplicateOutputLabelDirective` variants, completing
  the rejection family alongside duplicate names within a section and a repeated `.i`/`.o`. Breaking
  for any (malformed) file that relied on the last section silently winning.
- **`Bdd::ite` takes its operands by reference.** `ite(&self, g: &Self, h: &Self)` instead of
  consuming three handles by value, matching the `&`/`|`/`^`/`!` operators. Breaking for by-value
  callers, which now pass references.
- **`Bdd::complement` takes `&self`.** It borrows rather than consuming the handle, matching the `!`
  operator. Breaking for by-value callers, which now pass a reference (or use `!`).
- **`Bdd::forall` / `Bdd::exists` accept any iterable of names.** The `vars` parameter is now
  `impl IntoIterator<Item = impl AsRef<str>>` rather than `&[S]`, so an owned `Vec<String>` or an
  iterator adaptor works as well as a borrowed slice. Existing `&["a", "b"]` calls are unaffected.

### Added

- **`Hash` for `Bdd<B, C>`**, agreeing with the existing root-identity `PartialEq` (hashes the
  manager identity and canonical root id), so equal handles work as `HashSet`/`HashMap` keys.
- **`Debug`** for `ScopedBdd`, `Scope`, `ExprBuilder`, and `Expr`.
- **`Default` for `BoolExpr`**, equal to `BoolExpr::constant(false)`.
- **`Clone`, `PartialEq`, `Eq` for `ParseBoolExprError`.**
- **`#[must_use]` on `ExprBuilder::var`/`constant`/`graft`**, matching `Scope`'s methods. Code that
  discards these return values now gets an `unused_must_use` warning.

- **`try_minimize` / `try_minimize_exact`** on both `Espresso` and `EspressoCover`, the fallible
  counterparts of `minimize` / `minimize_exact`. They return
  `Result<(EspressoCover, EspressoCover, EspressoCover), MinimizationError>`, surfacing a caught C
  fatal as the new **`MinimizationError::EspressoFatal { message }`** variant. The infallible
  `minimize` / `minimize_exact` now delegate to these and panic on error (documented under `# Panics`).
- **`expr!` accepts `&`-referenced and bang-macro-call operands.** An operand may now be a reference
  (`expr!(&foo)`, through any number of reference levels) or a macro call (`expr!(make!())`, with
  `()`, `[]`, or `{}` delimiters), in addition to the identifiers, paths, field accesses, method and
  function calls, and indexes already accepted.
- **`Bdd::not`**, a named alias of `Bdd::complement` (both take `&self` and are equivalent to the
  unary `!` operator). Negation is offered under both names because `complement` reads naturally in a
  method chain while `!` reads naturally in an expression.

## [5.1.0] - 2026-07-01

Collection-returning query methods now return **lazy iterators** instead of owned collections, so
callers compose downstream and expansion happens on demand. A small breaking change, released as a
minor version.

### Changed

- **Iterator returns instead of owned collections.** These methods now return named, lazy iterator
  types rather than a `Vec`/`BTreeSet`/`Arc<[…]>`:
  - `Minterm::expand_over` / `Cube::expand_to` → `ExpandedMinterms` (packs each of the `2^k` minterms
    on demand — O(1) memory instead of materialising the whole set).
  - `Minterm::disagreement` → `Disagreement`.
  - `EspressoCover::to_cubes` → `EspressoCubes` (decodes one cube from the C `pset_family` per step).
  - `BoolExpr::variables` → `ExprVariables`.
  - `Bdd::variables` → `BddVariables`, now a genuinely lazy incremental graph walk that borrows the
    `Bdd` and resolves one support variable per step (so `.next()`/`.any(..)`/`.take(n)` skip the rest
    of the walk); no longer `ExactSizeIterator`/`DoubleEndedIterator`.
- **`Cover::maximize` takes variable *names*.** It now accepts `&[impl AsRef<str>]` on a `StringLabel`
  input and builds the target header directly, instead of `&[I]` label values. `&[Symbol]` calls are
  unaffected (`Symbol: AsRef<str>`); `&["a", "b"]` now works too.
- **Ordering relaxed.** The variable enumerations (`BoolExpr::variables`, `Bdd::variables`) and the
  minterm expansions yield in traversal order rather than sorted; they still deduplicate. Collect the
  result (`.collect::<Vec<_>>()`, `.collect::<BTreeSet<_>>()`) to recover the previous container, and
  sort explicitly if you relied on ordering.

### Removed

- **`Bdd::to_minterms`** (returned `Vec<Minterm>`) — replaced by **`Bdd::maximize(&[names]) -> Cover`**,
  the inverse of `Bdd::minimize`: it returns the fully-expanded, **deduplicated** maximal cover over the
  given variable names, each cube of which is a minterm (iterate `cover.cubes()`).
- **`Bdd::collect_variables`** — folded into `Bdd::variables`, which is now the single (iterator)
  accessor for a function's support.

## [5.0.0] - 2026-06-30

Major redesign splitting the **syntactic expression** from the **canonical BDD**. `BoolExpr` is now an
owned, syntactic value; all canonical and semantic operations move to a new owned `Bdd` handle obtained
from a branded builder. The process-global BDD manager is removed; the `expr!` macro is retained, now
lowering to the new `BoolExpr::build` arena builder. This release is **not** backward compatible.

### Changed

- **`BoolExpr` is now an owned, syntactic value** — a reverse-Polish token stream. It is brand-free:
  the `BoolExpr<B>` type parameter is gone. Build it (`BoolExpr::var`, `constant`), compose it, `parse`
  it, `Display` it, and `fold` over its structure. It does **not** canonicalise.
- **Equality is syntactic.** `PartialEq`/`Eq`/`Hash` compare the token structure, not the Boolean
  function: `a & b != b & a`. For logical equality build a `Bdd` and use `Bdd::equivalent_to`.
- **Operators are bitwise**: `&` (AND), `|` (OR), `^` (XOR), `!` (NOT), on both `BoolExpr` and `Bdd`.
  The arithmetic spellings `*` (AND) and `+` (OR) are removed as Rust operators (the text parser still
  accepts `*`/`+`/`~` as input).
- **`ExprNode` gains an `Xor` variant.** `BoolExpr::fold`/`fold_with_context` now walk the syntactic
  token structure, so exhaustive `match`es on `ExprNode` must handle `Xor`.
- **`Minimizable` is implemented only for `Cover`.** `Cover::to_expr` lowers a minimised cover to a
  factored `BoolExpr` by direct algebraic factorisation (never round-tripping through a BDD).

### Added

- **`Bdd<B, C>`** — an owned handle into a builder, parameterised by a uniqueness `Brand` `B` and a
  storage backend `C` (`ManagerCell`). It holds a refcounted clone of the builder's manager, so it can
  be stored, returned, and outlive the builder; it is `Clone` (a refcount bump), not `Copy`. The
  canonical and semantic operations live here: bitwise operators (by value, with `&` reference
  variants), `ite`, `restrict`/`cofactor`/`forall`/`exists`, `is_tautology`/`is_contradiction`,
  `equivalent_to` (O(1)), `evaluate`, `to_cubes`, `to_minterms`, `minimize`, `to_expr`,
  `fold`/`fold_with_context`, `collect_variables`/`node_count`/`var_count`, and `builder` (recovers a
  `BddBuilder` onto the same manager, so a stored handle can seed further construction in its namespace
  after the original builder is dropped). The brand stops handles
  from two builders unifying (a compile error); an always-on pointer-identity assert is the runtime
  backstop, panicking if handles from different managers are ever combined. `evaluate` is a partial
  evaluator: it takes a `Minterm` and returns `Result<bool, Bdd<B, C>>` — `Ok` when the assignment fixes
  the function to a constant, `Err` carrying the residual function over the still-free variables.
- **One generic builder, no global manager.** `BddBuilder<B, C>` is parameterised by a uniqueness
  `Brand` and a storage backend `C`. `bdd_builder!()` mints a builder over `LocalCell` (single-threaded,
  `!Send`); `sync_bdd_builder!()` mints one over `SyncCell` (`Send + Sync`). Each call mints a fresh
  sealed `Brand`, which marks one namespace for uniqueness and selects no behaviour — the backend is the
  orthogonal `C` choice. The builder provides `var`, `constant`, `build(&BoolExpr)`, `parse`,
  `build_cover`, and `minimize`. `BddBuilder::scope(|s| …)` composes a single result through `Copy`,
  by-reference `ScopedBdd` handles (a `Scope` of `&`/`|`/`^`/`!`, `var`/`constant`/`build`/`parse`, and
  `lift` to splice in an existing `Bdd`), returning the owned `Bdd` for the root — allocation-free
  composition with no `.clone()` at the call site.
- **`ManagerCell`** — the public, sealed storage-backend trait, the second `Bdd`/`BddBuilder` type
  parameter, orthogonal to the brand. Implemented by `LocalCell` (`Rc<RefCell<…>>`, single-threaded) and
  `SyncCell` (`Arc<RwLock<…>>`, thread-safe).
- **`Cover` from a `Bdd`.** `From<Bdd>`/`From<&Bdd>` for `Cover<Symbol, Anonymous>`, and `Cover::add_bdd`
  (the named-output primitive that `Cover::add_expr` now routes through via a temporary builder).
- **General Boolean-logic primitives on covers/minterms.** `Minterm::hamming_distance`/`disagreement`,
  `Minterm::expand_over`, `Cube::expand_to`, and `Cover::maximize` (fully-expanded minterm enumeration
  over an explicit, widenable variable set).
- **`BoolExpr::build`** — a closure constructor with an auxiliary `ExprBuilder`. The closure composes
  `Copy` `Expr<'b>` handles (the operators `& | ^ !`, plus `var`/`constant`/`graft`) into one arena that
  serialises to a single token stream, so assembling a large expression is linear rather than the
  quadratic token concatenation the operators on `BoolExpr` incur. The handle's lifetime confines it to
  the closure.
- **The `expr!` macro** (the re-introduced `espresso-logic-macros` crate). `expr!(…)` builds a `BoolExpr`
  from infix syntax, lowering to `BoolExpr::build`: an identifier grafts an existing `BoolExpr`, a string
  literal is a fresh variable, and `0`/`1` are constants; precedence is `+ < ^ < * < !`.

### Removed

- The **process-global BDD manager** and the global-brand API.
- **`BoolExpr::evaluate`, `BoolExpr::equivalent_to`, and the BDD-query methods** on `BoolExpr`
  (`node_count`, `var_count`, the semantic `collect_variables`, `to_cubes`): evaluation and all semantic
  queries are performed on `Bdd`. `BoolExpr::variables` remains as a syntactic scan.
- **`BoolExpr::variable`** — the duplicate of `BoolExpr::var`; use `var`.
- `impl Minimizable for BoolExpr` and the old 4.x BDD-handle API: the branded `BoolExpr<B>`, the old
  closure-based `BddBuilder`, and the `BddContext` scoped manager. (The 5.0 `BddBuilder<B, C>` is a new,
  unrelated type — the renamed scoped builder; the 5.0 `BoolExpr::build` is likewise new and unrelated,
  an arena builder yielding a syntactic `BoolExpr` rather than the old BDD-backed one.)

## [4.2.0] - 2026-06-29

Additive, fully backward-compatible: a scoped, branded alternative to the process-global BDD manager.
The global path is unchanged and stays the default, so existing code, doctests, and `Send`/`Sync`
behaviour are unaffected.

### Added

- Scoped, branded BDD contexts. `bdd_context!()` (an anonymous brand) or `bdd_context!(Name)` (a named
  brand, where the name is a readable label only) creates a `BddContext` that owns a private,
  independent BDD manager — its own node table, with no lock contention or cache pollution from
  unrelated global expressions. Every call mints a *distinct* brand. `ctx.var`, `ctx.constant`,
  `ctx.parse`, and `ctx.build` produce expressions branded to that context. Combining expressions from
  two distinct contexts is a compile error — the brand is an invariant type parameter, not a runtime
  check. `Brand` is sealed: every brand is either `Global` or minted by `bdd_context!`, so a brand
  always maps to exactly one manager.
- `BoolExpr` gained a defaulted brand parameter, `BoolExpr<B = Global>`. Bare `BoolExpr` is
  `BoolExpr<Global>` — the process-global expression every existing API already returns — so no
  annotation, signature, or trait-impl changes are needed. Every brand (global and scoped) is backed by
  `Arc<RwLock<BddManager>>`, so `BoolExpr` stays `Send`/`Sync` throughout; the brand `B` (a marker via
  the new `Brand` trait) only distinguishes namespaces.
- The `expr!` macro accepts an optional leading context: `expr!(ctx, a * b)` builds in `ctx`, while
  `expr!(a * b)` continues to build in the global manager.
- Operators, methods, `parse`, `evaluate`, display, and `Minimizable` now work for any brand. A scoped
  expression minimises to an expression in the same context.

## [4.1.1] - 2026-06-24

Ergonomic, fully backward-compatible patch: string-accepting APIs no longer privilege one string type,
and `Symbol` converts from every common string type. Existing `&str` call sites are unaffected.

### Added

- `Symbol` converts from every common string type via `From`/`.into()`: `&mut str`, `Box<str>`,
  `Arc<str>`, and `Cow<str>` join the existing `&str`/`String`/`&String` impls. `From<Arc<str>>` reuses
  the incoming allocation for a long (heap-interned) name rather than copying it.
- Labelled `Cube` constructors: `Cube::labeled` (from `(label, value)` pairs, any label type) and
  `Cube::with_labels` (the same with `&str` names). Pairing each label with its value makes a
  label/value length mismatch unrepresentable; both return `Result`, rejecting a side's duplicate
  labels with `DuplicateLabel` (duplicates would otherwise collapse to one column and drop a value).
- `Cover::push` and `Cover::from_cubes` now work for **any** label type, not just anonymous covers.
  A cube aligns onto the cover by variable identity — by name for labelled covers, by position for
  anonymous ones — and a cube carrying a new variable widens the cover by that identity (as `merge`
  does). Anonymous behaviour is unchanged (identity is position).

### Changed

- String-accepting entry points now take any `impl AsRef<str>` instead of `&str`, so no string type is
  privileged: `Symbol::new`, `BoolExpr::variable`, `BoolExpr::parse`, `BddBuilder::var`,
  `Cover::add_expr`, `Cover::to_expr`, and `PlaCover::from_pla_string`. Existing `&str` call sites are
  unaffected.

## [4.1.0] - 2026-06-24

This is an intentionally **API-breaking minor release** (low 4.0 adoption does not justify a major bump).

### Added

- Exclusive-or for boolean expressions: the `BoolExpr::xor` method and the `^` operator (for both
  `BoolExpr` and `&BoolExpr`), computed canonically as `ite(f, ¬g, g)`.
- `^` is now accepted by the string parser (`BoolExpr::parse`) and the `expr!` macro, with precedence
  **between** OR and AND (`a + b ^ c` parses as `a + (b ^ c)`; `a ^ b * c` as `a ^ (b * c)`),
  left-associative.
- A public low-level BDD builder: `BoolExpr::build(|b| ...)` composes `Bdd` handles via a `BddBuilder`
  (`var`/`constant`/`graft`/`not`/`and`/`or`/`xor`/`ite`). Handles are branded with the builder's lifetime,
  so they cannot escape the closure (a compile-time guarantee). Results are canonical, identical to the
  operator API.
- `BoolExpr::ite` — an if-then-else convenience over `build`.

### Changed

- **Breaking:** `Cube::outputs()` now returns `&OutputSet<O>` (was `&Minterm<O>`). A cube's output side is
  a binary, one-bit-per-output membership bitmap rather than a tri-state row, so per-output iteration
  yields `bool` instead of `Option<bool>` — migrate `out == Some(true)` to `out`. Input access via
  `Cube::inputs()` is unchanged (still `&Minterm<I>`).

## [4.0.1] - 2026-06-23

A polish and hardening release following a full code review: additive API conveniences, the PLA reader
and writer brought fully in line with the reference C implementation, two process-aborting C paths
turned into recoverable errors, and a slimmer published package. Well-formed PLA files parse, minimise,
and serialise byte-identically to 4.0.0.

### Added

- `Cover::with_labels` now takes independent type parameters for the input and output label slices, so
  the two may differ in concrete type (e.g. `&[String]` inputs with `&[&str]` outputs). Existing
  same-type calls still infer unchanged.
- `Symbol` now compares directly against `str` and `&str` in both directions (`PartialEq` and
  `PartialOrd`), mirroring how the standard library's `String` compares against string slices.
- `MintermIter` implements `ExactSizeIterator` (its remaining length is known in O(1)).
- `ExprNode` now derives `Hash`.
- `MinimizationError::NonOrthogonal` and `InstanceError::DimensionTooLarge` — the safe minimisation API
  now validates a cover and returns these instead of letting the C core abort the process (see below).
- `#[must_use]` on the remaining pure getters/constructors and on the low-level
  `EspressoCover`/`Espresso` `minimize`/`minimize_exact` methods.

### Changed

- The PLA reader now matches the reference C implementation (`cvrin.c`) when reading cube data: space,
  tab, `|` and newlines are all insignificant separators, and each cube is exactly `.i + .o`
  significant characters. In practice several cubes may now share a line, a single cube may span
  lines, and `.i`/`.o` are required up front (cube dimensions are never inferred from the data).
- The PLA reader's *input field* now matches C exactly: `0 1 2 - ?` are accepted (`?` being the empty
  literal), and `~`/`x`/`X` are rejected. A `?` makes a cube cover no minterm, so such a cube is
  dropped during minimisation, leaving the function unchanged; on a read-then-write (without
  minimising), the writer echoes `?` faithfully, matching C's `print_cube`.
- The PLA writer now groups cubes ON → DC → OFF (matching C's `fprint_pla`) for *any* cover, not just
  already-minimised ones — a directly-built or read-then-written multi-set cover no longer diverges.
- Minimisation now **validates** a cover before handing it to the C core, so two inputs that previously
  aborted the whole process (`exit(1)`) now return a recoverable error: a contradictory `FR`/`FDR`
  cover whose ON-set and OFF-set overlap (`NonOrthogonal`), and a dimension too large for the C core's
  32-bit cube indices (`DimensionTooLarge`).
- The published package now excludes development- and verification-only material (regression and
  benchmark data, dev scripts, hard test cases, CI configuration), making the download substantially
  smaller.

### Fixed

- Corrected stale or wrong rustdoc: the `PLAWriter` trait no longer references a nonexistent
  `PLASerialisable` trait and the `PLAReadError`/`PLAWriteError` docs point at the current
  `PlaCover::from_pla_*` / `PLAWriter` APIs; a worked `evaluate` example that printed the wrong result;
  a reference to a nonexistent `exact` configuration option (it is `minimize_exact`); the documented
  default don't-care set (an empty set, not the complement of F ∪ R); and the BDD variable ordering
  note (first-seen, not alphabetical).
- Replaced the unsubstantiated "~5–10% faster" low-level-API speed claim (README, crate/module docs)
  with measured figures from a new `api_overhead` benchmark: the low-level edge is a fixed per-call
  cost, ~10–14% on small covers but only ~1–5% (within noise) on large ones, and machine-/
  input-dependent.

## [4.0.0] - 2026-06-19

A breaking release with two themes: unifying the crate's four parallel product-term representations
onto a single label-carrying `Minterm` type, and reworking the cover layer around generic, first-class
variable **label types** (with no privileged default). It also modernises the internals (write-once
data returned as `Arc<[T]>`, iterator pipelines, recursion replaced by explicit work-stacks) and
tightens the public API.

### Breaking

Product-term representation:

- **Removed `Dnf`.** A disjunctive normal form is now just a single-output `Cover`; the
  `BTreeMap<Arc<str>, bool>` cube representation and the `BoolExpr ↔ Dnf` conversions are gone.
  Minimise a `BoolExpr` directly (`expr.minimize()`), or go through `Cover`.
- **Removed `Cover::cubes_iter()` and the `CubeData` tuple alias.** Use `Cover::cubes()`, which
  yields `&Cube`; read its set with `Cube::cube_type()`.
- **`Cube::inputs()` / `Cube::outputs()` now return `&Minterm`** (were `&[Option<bool>]` and
  `&[bool]`). Read individual values with `Minterm::value_at(i)` / `value_of(name)` or `iter()`.
- **`Cover::add_expr` now takes `&BoolExpr`** instead of a generic `&T: Into<Dnf>`.
- **`Minimizable` is implemented concretely for `Cover` and `BoolExpr`** instead of via a blanket
  `impl<T> where &T: Into<Dnf>, T: From<Dnf>`.
- **`Minimizable`'s required methods are now `try_minimize_with_config` and
  `try_minimize_exact_with_config`** (the fallible primitives); the panicking `minimize_with_config` /
  `minimize_exact_with_config` are now default methods layered on top. Callers are unaffected;
  downstream *implementors* of the trait must rename their two methods.
- **`BoolExpr::to_cubes()` now returns `Arc<[Minterm]>`** (was `Vec<BTreeMap<Arc<str>, bool>>`).
- **`EspressoCover::to_cubes()` now returns `Arc<[Cube]>`** (was `Vec<Cube>`).
- **Removed the `LabelManager` type;** `Cover` owns its canonical input/output headers directly.

Label types & cover construction:

- **`Symbol` is now the variable-name type** — a small-string-optimised, interned string — replacing
  `Arc<str>` as the default name representation.
- **`Cover`, `Cube`, `Minterm`, `Symbols` are generic over their label type(s) with no default type
  parameter.** `Cover<I, O>` and `Cube<I, O>` carry **separate input/output label types**, so `Symbol`
  is no more privileged than `String`/`Arc<str>`/`u32`. `Cover::new(..)` consequently needs a type
  annotation, e.g. `Cover::<Symbol, Symbol>::new(..)`.
- **Positional covers use the zero-sized `Anonymous` label** instead of `()`; build them with
  `Cover::<Anonymous, Anonymous>::anonymous(..)`.
- **Removed `Cover::add_cube(...)`.** Construct cubes explicitly with
  `Cube::anonymous(inputs, outputs, CubeType)` + `Cover::push`, or `Cover::from_cubes`.
- **`Cover::relabel` / `relabel_inputs` / `relabel_outputs` now return `Result<_, ArityMismatch>`**
  instead of panicking on a wrong-arity table. `anonymize()` stays infallible.
- **PLA reading now yields a `PlaCover<S>`** — a sum type whose variant records which `.ilb`/`.ob`
  label sections the file carried — via `PlaCover::from_pla_string` / `from_pla_file`. The old
  `Cover::from_pla_*` methods and the `PLAReader` trait are removed; writing still goes through
  `PLAWriter`.
- **`BoolExpr::fold_with_context` redesigned** from continuation-passing callbacks to a
  `(descend, combine)` pair (top-down context, bottom-up results), enabling an iterative walk.
- **`BoolExpr::collect_variables()` returns `BTreeSet<Symbol>`** (was `BTreeSet<Arc<str>>`).
- **All public error enums are `#[non_exhaustive]`;** `INLINE_CAP` is no longer public.
- **Removed the deprecated `Bdd` type alias and the `to_bdd` / `from_expr` / `to_expr` methods**
  (all no-op `clone()` shims). A `BoolExpr` is already a BDD; use it directly (and `clone()` where a
  copy is wanted).
- **Minimum supported Rust version is now 1.82.**

### Added

- **`Symbol`** (`src/symbol.rs`): a compact, interned variable-name type — inline for short names,
  pooled and shared for longer ones.
- **`Minterm`** (`src/cover/minterm.rs`): a new label-carrying row of tri-state values
  (`Some(true)`/`Some(false)`/`None`), bit-packed two bits per variable — the single representation
  that replaces the crate's former four parallel product-term types. Carries its variable header so
  comparisons align by variable identity, with a pointer-equality fast path for same-cover cubes.
  Set operations `is_subset_of` / `is_superset_of` / `is_disjoint_with`, plus `Ord`/`Eq`/`Hash`.
- **`Cube` and `CubeType` are public**, each cube being a pair of `Minterm`s plus an F/D/R set tag.
- **`Anonymous` label** and the sealed **`Label` / `StringLabel` / `PlaLabel` / `ReconcilableLabel`**
  trait family. Label-presence is **type-level**: a label is a *name* iff it is `Display`, so a
  `Cover<Anonymous, _>` cannot emit input names by construction.
- **`PlaCover<S>` PLA reader** with variants `InputsOutputsNamed` / `InputsNamed` / `OutputsNamed` /
  `Positional`.
- **`Cover::extend` and `Cover::merge`** for combining covers (append vs identity-overlay of outputs),
  renaming output-column collisions via `ReconcilableLabel`.
- **Non-panicking minimisation:** `Minimizable::try_minimize` / `try_minimize_exact` (and their
  `_with_config` forms) return `MinimizationError::Instance` on a cross-dimension Espresso instance
  conflict instead of panicking. The panicking `minimize*` methods now panic *only* on that conflict.

### Changed

- Write-once collections are returned as `Arc<[T]>` rather than `Vec<T>`; internal construction uses
  iterator pipelines instead of intermediate `Vec` buffers.
- **Recursive BDD/AST traversals are now explicit work-stack iteration** (the BDD `ite` apply, cube
  extraction, evaluation, the AST folds, and factorisation), removing the call-stack depth ceiling on
  deep inputs while preserving memoisation.
- **Malformed PLA input now errors instead of being silently skipped** (e.g. dimension mismatches,
  missing dimensions, and an unrecognised `.type` value); `.end` is accepted as a read terminator
  alongside `.e`.
- **The raw FFI `sys` module is now `#[doc(hidden)]`** — still reachable for the low-level layer, but
  off the documented public surface (its bindgen-generated types are not part of the stable API).

### Fixed

- BDD variable collection (`collect_variables` / `var_count`) now deduplicates by **node** rather
  than by variable, so it no longer misses variables that appear only in some branches.
- **`BoolExpr::equivalent_to` no longer swallows an internal error as `false`** — equivalence is an
  exact canonical-BDD root comparison (identical to `==`).
- **CLI `-e`/`--exact` now runs exact minimisation.** It previously only toggled fast single-expand
  mode while still running the heuristic algorithm; it is now an alias for `-D exact`.
- **`EspressoCover::from_cubes` now validates cube slice lengths** (new `CubeError::DimensionMismatch`)
  instead of writing out of the cube's bit region when a slice doesn't match the declared dimensions.

## [3.1.2] - 2025-11-12

### Documentation

**Comprehensive rustdoc overhaul** - Improved and reorganised all documentation to be more accurate and comprehensive:

- **lib.rs landing page**: Simplified structure, properly positioned Espresso as the main feature with BDDs as implementation detail
- **expression module**: Embedded comprehensive BOOLEAN_EXPRESSIONS.md guide into module documentation
- **cover module**: Enhanced with detailed explanations of covers, cover types, and when to use Cover vs BoolExpr
- **pla module**: Moved into `cover::pla` submodule and embedded PLA_FORMAT.md specification
- **examples module**: Created documentation-only module embedding EXAMPLES.md for comprehensive examples
- **Thread safety**: Fixed incorrect documentation - correctly explains Cover's lazy thread-local Espresso creation
- **BoolExpr struct**: Enhanced documentation explaining internal BDD representation, cloning behavior, and thread safety
- **Cover struct**: Comprehensive documentation of structure, dynamic dimensions, input/output encoding, and thread safety
- **Removed outdated references**: Cleaned up `fold_with_context` documentation that referenced old example code

### Changed

- **Deprecated `Bdd` type alias** - Added `#[deprecated]` attribute to encourage using `BoolExpr` directly
- **Removed error type re-exports** - Error types now accessed via their respective modules (`error::*`, `cover::error::*`, `expression::error::*`)
- **Code organization**: Moved blanket `Minimizable` implementation from removed `minimize.rs` to `minimisation.rs`

### Fixed

- **Cache sharing documentation**: Correctly documented that `OnceLock::clone()` copies content, so caches ARE shared between clones via Arc
- **Bdd/BoolExpr references**: Cleaned up all documentation treating them as separate types (they're unified since v3.1.1)

### Documentation Structure

All markdown documentation files remain in `docs/` for GitHub display, but are now embedded into rustdoc where appropriate:
- `docs/BOOLEAN_EXPRESSIONS.md` → embedded in `expression` module
- `docs/EXAMPLES.md` → embedded in `examples` module  
- `docs/PLA_FORMAT.md` → embedded in `cover::pla` module
- `docs/CLI.md`, `docs/INSTALLATION.md` → kept standalone for GitHub-friendly access

**Note:** This is a documentation-only release with no functional changes to the API or implementation.

## [3.1.1] - 2025-11-12

### Changed

**Internal Architecture (No Breaking Changes):**
- **Unified `Bdd` and `BoolExpr` types** - `Bdd` is now a type alias for `BoolExpr`. All boolean expressions now use BDD as their canonical internal representation, eliminating redundancy and providing significant advantages:
  - **Canonical representation**: Equivalent expressions have identical internal structure
  - **Efficient operations**: Polynomial-time AND/OR/NOT via hash consing and memoisation
  - **Memory efficiency**: Structural sharing across all operations
  - **Automatic simplification**: Redundancy elimination during construction
  - **Fast equality checks**: O(1) pointer comparison for equivalent expressions
- **Algebraic factorisation for expression display** - Expressions now display as multi-level logic with common factor extraction (e.g., `a*(b+c)` instead of `a*b + a*c`)
- **Simplified caching architecture** - Local-only DNF and AST caching with Arc-wrapped structures for efficient cloning
- **Reorganised expression module** - Split into focused submodules (ast.rs, bdd.rs, operators.rs, eval.rs, manager.rs) with 70% reduction in main module size

**Caching Architecture:**
- **DNF Cache**: Arc-wrapped Dnf for efficient cube extraction (local per-expression)
- **AST Cache**: Cached factored AST for beautiful expression display
- **BDD Representation**: Canonical form with hash consing (shared via manager)

### Improved

- **Expression display quality** - Produces factored multi-level logic expressions instead of flat DNF
- **Code organisation** - Better module structure with clearer separation of concerns
- **Performance** - Cheaper BoolExpr cloning with Arc-wrapped internal structures

### Deprecated

- **`BoolExpr::to_bdd()`** - Returns `self.clone()` (BoolExpr IS a BDD now)
- **`Bdd::from_expr()`** - Returns `expr.clone()` (redundant conversion)
- **`Bdd::to_expr()`** - Returns `self.clone()` (redundant conversion)

These methods remain for backwards compatibility but are no-ops in v3.1.1+.

### Documentation

- Updated performance metrics with actual measured values from threshold gate examples
- Clarified that BDD is now the internal representation (not a separate conversion)
- Improved explanation of BDD/BoolExpr unification and its advantages
- Enhanced example clarity in documentation

### Technical Notes

All changes are internal improvements with full backwards compatibility. The public API remains unchanged from v3.1.0. Existing code will continue to work without modification, though deprecated conversion methods can be removed for cleaner code.

## [3.1.0] - 2025-11-11

### Breaking Changes

**API Ownership & References:**
- **`Cover::minimize()`** - Now returns new instance instead of mutating: `self -> Self`
- **`Espresso::minimize()`** - Takes reference instead of owned value: `EspressoCover -> &EspressoCover`
- **`EspressoCover::minimize()`** - Takes reference instead of owned value: `self -> &self`
- **`Cover::add_expr()`** - Takes reference instead of owned expression: `BoolExpr -> &BoolExpr`
- **Ownership semantics** - More explicit throughout API, following Rust best practices

### Added

**Binary Decision Diagram (BDD) Support:**
- **`Bdd` type** - Canonical representation of boolean functions using reduced ordered BDDs (ROBDDs)
- **Global singleton manager** - Shared BDD manager with hash consing and operation caching
- **`BoolExpr::to_bdd()`** - Convert expressions to BDDs with internal caching for efficiency
- **Efficient operations** - AND, OR, NOT operations in polynomial time
- **Conversions** - Seamless conversion between BoolExpr ↔ BDD ↔ DNF ↔ Cover
- **Canonical representation** - Equivalent functions have identical BDD representations
- **Thread-safe** - Mutex-protected manager enables concurrent BDD operations
- **Comprehensive BDD tests** - Extensive test suite covering operations, caching, and conversions
- **Two-step minimization (BDD + Espresso):**
  - BDD provides automatic redundancy elimination and canonical form (ordering-dependent, uses alphabetical order)
  - Espresso provides optimal logic minimization (ordering-independent)
  - **BDD avoids exponential blowup**: Converts complex compositions to DNF in polynomial time vs exponential with naive De Morgan's law expansion
  - **Example**: XOR of two 6-term expressions → BDD: 14 cubes, naive De Morgan: ~150 cubes (10x improvement!)
  - BDD pre-minimization reduces cube count fed to Espresso, improving overall efficiency
  - Both steps are necessary: BDD efficiently converts to canonical DNF, Espresso achieves optimal minimization
  - Optimal BDD variable ordering is NP-complete, so Espresso's ordering-independent minimization is essential

**Enhanced Boolean Expression Parser:**
- **Alternative operator syntax** - Support both `&` and `*` for AND operations
- **Alternative OR syntax** - Support both `|` and `+` for OR operations
- **Mixed notation** - Allow mixing notations within the same expression
- **Enhanced `expr!` macro** - Compose existing `BoolExpr` values with string literals
- **Expression composition** - Build complex formulas from parsed, minimized, or constructed sub-expressions

**New Public API Exports:**
- **`Minimizable` trait** - Publicly exported to enable explicit trait-based minimization
- **`Dnf` type** - Disjunctive Normal Form type made public for advanced use cases
- **`Bdd` type** - Binary Decision Diagram type exposed at crate root for direct BDD manipulation
- **`ExprNode<'a, T>` enum** - New public type representing expression tree nodes for folding operations used with `fold()` and `fold_with_context()`

**Expression Tree Folding API:**
- **`BoolExpr::fold()`** - New method for bottom-up tree folding with custom transformations
- **`BoolExpr::fold_with_context()`** - New method for top-down context-based tree folding

**New Examples:**
- `examples/threshold_gate_example.rs` - 5-input threshold gate showing dramatic minimization (hold: 22→10 terms) and complex composition with XOR helper
- `examples/c_element_example.rs` - Simple C-element for basic demonstration

**Enhanced Tests:**
- Consolidated test suite with comprehensive coverage for all features
- **216 unit tests** - All passing with comprehensive coverage

### Changed

**Modular Codebase Restructuring:**
- **BDD module** - Moved from `expression::bdd` to top-level `bdd` module
- **Module directories** - Converted monolithic files to focused module directories:
  - `src/espresso.rs` → `src/espresso/mod.rs` + `src/espresso/error.rs`
  - `src/pla.rs` → `src/pla/mod.rs` + `src/pla/error.rs`
  - Split `src/expression/mod.rs` into specialized submodules:
    - `conversions.rs` - Type conversion implementations
    - `display.rs` - Display trait implementations
    - `error.rs` - Expression parsing error types
    - `eval.rs` - Evaluation and equivalence checking
    - `operators.rs` - Operator overloading implementations
    - `parser.rs` - Parsing logic
    - `tests.rs` - Comprehensive expression test suite
  - Split `src/cover/mod.rs` into focused submodules:
    - `cubes.rs` - Cube-related types
    - `labels.rs` - Label management utility
    - `iterators.rs` - Iterator types
    - `dnf.rs` - DNF and minimization functionality
    - `expressions.rs` - Expression integration methods
    - `minimisation.rs` - Minimizable trait implementation
    - `conversions.rs` - Trait implementations
    - `error.rs` - Cover-specific error types
    - `tests.rs` - All test cases

**API Improvements:**
- **Explicit ownership** - All methods now make ownership explicit (no implicit moves)
- **Reference-based minimize** - Allows reusing input covers without cloning
- **Better composition** - `expr!` macro seamlessly composes any `BoolExpr` values
- **Clearer documentation** - Updated rustdocs to reflect new patterns

**Documentation Updates:**
- **docs/BOOLEAN_EXPRESSIONS.md** - Added alternative syntax and composition patterns
- **docs/EXAMPLES.md** - Added BDD examples and new example file documentation
- **README.md** - Updated with BDD example and alternative operator syntax
- **All examples** - Updated to use new reference-based API
- **Rustdocs** - Comprehensive API documentation with all public types and methods

**Repository Reorganization:**
- **PLA test files** - Moved 127 PLA example files from `examples/` to `pla/` directory to separate test data from code examples
- **API documentation** - Removed `docs/API.md` in favor of comprehensive rustdocs for better integration with docs.rs
- **Test consolidation** - Merged standalone test example files into main test suite for better organization

### Fixed

- **Parser flexibility** - Now accepts both mathematical (`*`, `+`) and logical (`&`, `|`) operator notations
- **Expression composition** - `expr!` macro can now compose any `BoolExpr` value, not just string literals

### Performance

- **Lazy BDD caching** - Each `BoolExpr` lazily caches its BDD representation using `OnceLock`
  - First call to `to_bdd()` computes and caches the BDD at expression level
  - Subsequent calls return the cached BDD (O(1) access)
  - During composition, subexpression BDD caches are automatically leveraged
  - Prevents redundant BDD construction when the same subexpression appears multiple times
  - Especially beneficial during complex expression composition and transformation
  - **Important:** Minimization creates a new `BoolExpr` with empty expression-level cache
  - Global BDD manager caches (ITE cache, unique table) persist while any Bdd exists
  - Prefer minimizing late (after composition) to maximize expression-level cache hits
- **Hash consing** - Global node sharing across all BDDs reduces memory usage
- **Operation memoization** - ITE results cached and shared across all BDD operations

### Migration Guide

**API Ownership Changes:**

```rust
// v3.0.0 - mutating minimize
let mut cover = Cover::new(CoverType::F);
cover.add_cube(...)?;
cover.minimize()?; // mutates in place

// v3.1.0 - returns new instance
let mut cover = Cover::new(CoverType::F);
cover.add_cube(...)?;
let minimized = cover.minimize()?; // returns new instance
```

**Expression References:**

```rust
// v3.0.0 - takes ownership
let expr = BoolExpr::parse("a * b")?;
cover.add_expr(expr)?; // expr moved

// v3.1.0 - takes reference
let expr = BoolExpr::parse("a * b")?;
cover.add_expr(&expr)?; // expr can be reused
```

**Using BDDs:**

```rust
use espresso_logic::{BoolExpr, Bdd};

let expr = BoolExpr::parse("a * b + a * b * c")?;
let bdd = expr.to_bdd(); // Cached conversion
println!("BDD has {} nodes", bdd.node_count());

// BDDs support efficient operations
let bdd_a = BoolExpr::variable("a").to_bdd();
let combined = bdd.and(&bdd_a);
```

**Alternative Parser Syntax:**

```rust
// Both notations work identically
let expr1 = BoolExpr::parse("a * b + c")?;  // Mathematical notation
let expr2 = BoolExpr::parse("a & b | c")?;  // Logical notation
let expr3 = BoolExpr::parse("a * b | c")?;  // Mixed notation
```

**Expression Composition:**

```rust
// Compose parsed, minimized, or constructed expressions
let func1 = BoolExpr::parse("a * b")?;
let func2 = BoolExpr::parse("c + d")?;
let minimized = func1.minimize()?;

// Seamlessly compose with expr! macro
let combined = expr!(minimized * func2 + "e");
```

### Statistics

- **Test coverage:** 373 automated tests (51 unit/integration + 322 doc tests + ~276 regression tests), all passing
- **Modular organization:** 4 major modules refactored into focused submodules
- **Repository cleanup:** Moved 127 PLA test files from `examples/` to `pla/` directory

## [3.0.0] - 2025-11-09

### Breaking Changes

**Unified Cover API:**
- **`CoverBuilder` removed** - Replaced with dynamic `Cover` type that automatically grows dimensions
- **`PLAType` renamed to `CoverType`** - More intuitive naming for cover types (OnSet, OnSetDontCare, etc.)
- **`ExprCover` removed** - Functionality merged into unified `Cover` type
- **`PLACover` removed** - Dynamic PLA functionality merged into unified `Cover` type
- **New expression methods:** `Cover::add_expr()` and `Cover::to_expr()` replace `ExprCover`
- **Iterator return types changed** - Replaced `Box<dyn Iterator>` with concrete iterator types (`CubesIter`, `ToExprs`)
- **Trait methods use GATs** - `Minimizable` and `PLASerialisable` traits now use Generic Associated Types

**Error Handling:**
- **Structured error hierarchy** - Replaced generic `EspressoError` with operation-specific error types:
  - `MinimizationError`, `AddExprError`, `ToExprError`, `ParseBoolExprError`, `PLAReadError`, `PLAWriteError`
- **Source-level errors** - `InstanceError`, `CubeError`, `ExpressionParseError`, `CoverError`, `PLAError`
- **Programmatic error handling** - All errors implement `Error` trait with proper error chains

**Dependencies:**
- **`clap` now optional** - Use `cli` feature flag to build the binary: `cargo install espresso-logic --features cli`
- **`tempfile` moved to dev-dependencies** - Not part of public API

### Added

**Procedural Macro Support:**
- **`expr!` macro** - Three convenient styles for boolean expressions:
  - String literals: `expr!("a" * "b" + "c")`
  - Variables: `expr!(a * b + c)`
  - Mixed: `expr!(a * "temp" + b)`
- **New workspace crate:** `espresso-logic-macros` for procedural macro implementation

**Enhanced Boolean Expression API:**
- **`BoolExpr::equivalent_to()`** - Test logical equivalence between expressions
- **`BoolExpr::to_dnf()`** - Public API for Disjunctive Normal Form conversion
- **Improved Display trait** - Minimal parentheses output for better readability

**Dynamic Cover API:**
- **`Cover::with_labels()`** - Pre-define variable names for inputs/outputs
- **Automatic dimension growth** - Dimensions expand as cubes are added
- **Label accessors:** `input_labels()`, `output_labels()`
- **Lazy label generation** - Labels only created when needed
- **Unlabeled cover support** - Covers can be minimized without ever creating labels

**Thread-Safe Direct Espresso API (Previously Private):**
- **Exposed low-level API** - Previously private `espresso` module now public for advanced users
- **New `src/espresso.rs` module** - Direct API using thread-local storage
- **`Espresso` singleton** - Automatic cleanup via `Rc<EspressoInner>`
- **`EspressoCover` type** - Safe cover management with memory guarantees
- **`EspressoConfig`** - Comprehensive configuration for minimization algorithms
- **Fine-grained control** - Direct access for performance-critical applications

**Reader/Writer APIs:**
- **`Cover::write_pla<W: Write>()`** - Efficient writer-based PLA serialization
- **`Cover::from_pla_reader<R: BufRead>()`** - Reader-based PLA parsing
- **Composable I/O** - Works with compression, network streams, etc.
- **Zero-copy file operations** - Direct buffered I/O without intermediate strings

**Comprehensive Testing:**
- **~283 regression tests** - Full C implementation parity
- **Memory safety tests** - Comprehensive leak detection and validation
- **Thread safety tests** - Parallel execution validation
- **Benchmark suite** - Criterion-based performance benchmarks with balanced sampling
- **Leak detection examples:** `leak_check.rs`, `intentional_leak.rs`

**New Examples:**
- `examples/expr_macro_demo.rs` - Showcase `expr!` macro styles
- `examples/test_new_api.rs` - Demonstrate unified API patterns
- `examples/variable_labels.rs` - Working with labeled variables
- `examples/espresso_direct_api.rs` - Direct Espresso API usage
- `examples/writer_api.rs` - Writer-based PLA serialization
- `examples/reader_api.rs` - Reader-based PLA parsing

**Documentation (Comprehensive Revision):**
- **`docs/EXAMPLES.md`** - Comprehensive usage examples (new)
- **`docs/INSTALLATION.md`** - Detailed setup instructions (new)
- **`docs/PLA_FORMAT.md`** - PLA file format specification (new)
- **`docs/MEMORY_SAFETY.md`** - Memory safety guarantees (new)
- **`docs/LEAK_TESTING.md`** - Leak testing procedures (new)
- **`TESTING.md`** - Comprehensive testing documentation (new)
- **`docs/API.md`** - Completely rewritten with high-level/low-level API guidance
- **`docs/BOOLEAN_EXPRESSIONS.md`** - Expanded with expr! macro documentation
- **`docs/CLI.md`** - Updated with feature flag information
- **Enhanced API documentation** - All code examples now complete and runnable with proper error handling
- **Doc module** - Comprehensive guides exposed on docs.rs
- **README.md** - Streamlined and updated for v3.0 API

**Build & Tooling:**
- **C11 thread-local detection** - Enhanced build.rs validation
- **Leak checking scripts** - macOS and Linux memory leak detection
- **Balanced benchmark sampling** - 10 files per size category for efficient testing

### Changed

**Performance Improvements:**
- **O(1) label lookups** - Replaced Vec-based linear search with HashMap (was O(n))
- **Lazy label generation** - Labels only created when needed
- **Smart conflict resolution** - Sequential label backfilling (e.g., x0, x1, x3 → uses x2)
- **Batch dimension resizing** - `Cover::add_expr()` optimized for bulk operations
- **Fail-fast validation** - Early output conflict detection

**API Improvements:**
- **Better error messages** - Context-rich error types throughout
- **Intuitive method names** - `add_expr()`, `to_expr()`, clearer semantics
- **Independent label management** - Input/output labels managed separately
- **Mixed labeled/unlabeled support** - Proper backfilling when transitioning

**Code Quality:**
- **Removed `unsafe.rs`** - Replaced with memory-safe abstractions
- **All clippy warnings fixed** - Modern Rust idioms throughout
- **Proper error chains** - All errors implement `Error` trait with `source()`
- **Automatic cleanup** - Removed manual `drop()` calls, rely on RAII

**PLA Format:**
- **Header ordering** - Matches C implementation (.i, .o, .ilb, .ob)
- **Multi-line parsing** - Proper character accumulation and dimension truncation
- **Unlabeled PLA support** - Files without .ilb/.ob create unlabeled covers
- **Conditional label output** - Labels only written if they exist

**Test Infrastructure:**
- **Expanded regression suite** - ~283 tests covering all formats and examples
- **Timeout protection** - 30s main suite, 10s quick tests
- **Skip tracking** - Identifies tests that timeout in C implementation
- **Merged test scripts** - Consolidated comprehensive_regression.sh into regression_test.sh

### Fixed

- **C implementation parity** - All tests that complete in C now produce identical output
- **Cube filtering** - Removed manual filtering; Espresso algorithm returns correct cubes
- **Boolean expression evaluation** - Fixed documentation examples to show correct logic
- **Thread-local storage** - Proper C11 `_Thread_local` detection and usage
- **Memory leaks** - Comprehensive leak prevention with automatic cleanup

### Removed

- **`docs/PROCESS_ISOLATION.md`** - Obsolete implementation documentation
- **`src/unsafe.rs`** - Replaced with safe abstractions
- **`.github/FUNDING.yml`** - Removed funding configuration
- **`.github/README.md`** - Consolidated into main README
- **Manual cleanup methods** - `Espresso::cleanup_if_unused()` removed (automatic via RAII)

### Migration Guide

**From v2.x CoverBuilder to v3.0 Cover:**

```rust
// v2.x
let mut builder = CoverBuilder::new(2, 1, PLAType::F);
builder.add_cube(&[Ternary::One, Ternary::Zero], &[Ternary::One]);
let cover = builder.build();

// v3.0
let mut cover = Cover::new(CoverType::F);
cover.add_cube(&[Some(true), Some(false)], &[Some(true)])?;
// Dimensions grow automatically!
```

**From v2.x ExprCover to v3.0 Cover:**

```rust
// v2.x
let mut expr_cover = ExprCover::new();
expr_cover.add_expr(&expr)?;
let minimized = expr_cover.minimize()?;

// v3.0
let mut cover = Cover::new(CoverType::F);
cover.add_expr(&expr)?;
let minimized = cover.minimize()?;
```

**Using the new expr! macro:**

```rust
// v3.0 - Three convenient styles
use espresso_logic::expr;

let e1 = expr!("a" * "b" + "c");           // String literals
let e2 = expr!(a * b + c);                  // Variables
let e3 = expr!(a * "temp" + b);            // Mixed
```

**Error handling:**

```rust
// v2.x
match result {
    Err(e) => eprintln!("Error: {}", e),  // String error
    Ok(v) => v,
}

// v3.0
match result {
    Err(MinimizationError::Instance(e)) => { /* handle instance error */ }
    Err(MinimizationError::Cube(e)) => { /* handle cube error */ }
    Err(MinimizationError::Io(e)) => { /* handle I/O error */ }
    Ok(v) => v,
}
```

**Installing the CLI:**

```bash
# v2.x
cargo install espresso-logic

# v3.0
cargo install espresso-logic --features cli
```

### Statistics

- **42 files changed:** 5,340 insertions, 2,440 deletions
- **Net addition:** ~2,900 lines
- **Test coverage:** ~283 regression tests, 235+ unit tests
- **Documentation:** 5 new comprehensive guides

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
- **Note (updated v3.1):** Expressions are now converted to DNF via BDD (Binary Decision Diagrams) for efficiency, avoiding exponential complexity of direct DNF conversion
- Variables are stored in alphabetical order (BTreeSet) for consistency
- DNF cubes are directly compatible with Espresso's cover format
- Expression parsing is type-safe and returns helpful error messages
- All expression operations preserve structural sharing via Arc

### Performance
- Expression parsing: microseconds for typical expressions
- **Note (updated v3.1):** DNF conversion via BDD: polynomial time for most practical expressions (was direct conversion in v2.6)
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
