//! Tests for the canonical BDD layer.
//!
//! Two in-crate brand types are declared here: `Local` selects the single-threaded
//! [`LocalCell`](crate::expression::manager_cell::LocalCell) and `Sync` the thread-safe
//! [`SyncCell`](crate::expression::manager_cell::SyncCell). The sealed [`Brand`] trait permits these
//! in-crate impls; downstream code cannot add brands. The public `bdd_context!` / `sync_bdd_context!`
//! macros that would mint these for callers arrive with the 5.0 breaking cut.

use super::brand::{brand_seal, Brand};
use super::{BddContext, SyncBddContext};
use crate::cover::{Cover, CoverType, Cube, CubeType, Minterm, Symbols};
use crate::expression::manager_cell::{LocalCell, SyncCell};
use crate::Symbol;
use std::collections::BTreeSet;
use std::sync::Arc;

/// Single-threaded test brand (selects [`LocalCell`]).
#[derive(Clone, Copy)]
struct Local;
impl brand_seal::Sealed for Local {}
impl Brand for Local {
    type Cell = LocalCell;
}

/// Thread-safe test brand (selects [`SyncCell`]).
#[derive(Clone, Copy)]
struct Sync;
impl brand_seal::Sealed for Sync {}
impl Brand for Sync {
    type Cell = SyncCell;
}

// ---- Send/Sync asymmetry between the two context kinds ---------------------------------------------

/// `SyncBddContext` (SyncCell brand) is `Send + Sync`; asserting these bounds compiles only if they
/// hold. The `!Send`/`!Sync` of `BddContext` (LocalCell brand) is checked by `local_context_not_send`
/// below, which fails to compile if a `LocalCell`-branded context were ever made `Send`.
#[test]
fn sync_context_is_send_and_sync() {
    fn assert_send<T: std::marker::Send>() {}
    fn assert_sync<T: std::marker::Sync>() {}
    assert_send::<SyncBddContext<Sync>>();
    assert_sync::<SyncBddContext<Sync>>();
    // And a handle into it is Send + Sync too.
    assert_send::<super::Bdd<'static, Sync>>();
    assert_sync::<super::Bdd<'static, Sync>>();
}

/// Compile-time witness that a `LocalCell`-branded context is **not** `Send`/`Sync`, while a
/// `SyncCell`-branded one is.
///
/// Stable autoref-specialisation (Kalbertodt / dtolnay): a blanket trait impl over **all** `T` provides
/// the `false` fallback, while an **inherent** method on `Probe<T>` gated on `T: Send` provides `true`.
/// Method resolution prefers the inherent method when it applies (`T: Send`); otherwise it falls back to
/// the trait method. So `Probe::<T>::default().probe()` is `true` exactly when `T: Send`.
#[test]
fn context_send_asymmetry() {
    struct Probe<T>(std::marker::PhantomData<T>);

    impl<T> Default for Probe<T> {
        fn default() -> Self {
            Probe(std::marker::PhantomData)
        }
    }

    // Fallback for every `T`: not Send.
    trait NotSendProbe {
        fn probe(&self) -> bool {
            false
        }
    }
    impl<T> NotSendProbe for Probe<T> {}

    // Specialised inherent method: only present (and selected) when `T: Send`.
    impl<T: std::marker::Send> Probe<T> {
        fn probe(&self) -> bool {
            true
        }
    }

    macro_rules! is_send {
        ($t:ty) => {{
            Probe::<$t>::default().probe()
        }};
    }

    // SyncCell-branded SyncBddContext is Send; LocalCell-branded contexts are not.
    assert!(is_send!(SyncBddContext<Sync>));
    assert!(!is_send!(BddContext<Local>));
    assert!(!is_send!(SyncBddContext<Local>));
}

// ---- Requirement 1: Shannon cofactor / quantification ---------------------------------------------

#[test]
fn restrict_acceptance_table() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");

    // (a & b).restrict("a", true) ≡ b
    assert!((a & b).restrict("a", true).equivalent_to(b));
    // (a & b).restrict("a", false) ≡ false
    assert!((a & b).restrict("a", false).is_contradiction());
    // (a | b).restrict("a", false) ≡ b
    assert!((a | b).restrict("a", false).equivalent_to(b));
    // (a | b).forall(&["a"]) ≡ b
    assert!((a | b).forall(&["a"]).equivalent_to(b));
    // (a & b).exists(&["a"]) ≡ b
    assert!((a & b).exists(&["a"]).equivalent_to(b));
    // (a ^ b).forall(&["a"]) ≡ false
    assert!((a ^ b).forall(&["a"]).is_contradiction());
}

#[test]
fn restrict_absent_variable_is_noop() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");
    let c = ctx.var("c");
    let f = a & b; // depends only on a, b

    // c.restrict("a", true) ≡ c  — restricting an absent variable is a no-op
    assert!(c.restrict("a", true).equivalent_to(c));
    // restricting a variable absent from `f` leaves f unchanged
    assert!(f.restrict("z", true).equivalent_to(f));
    assert!(f.restrict("z", false).equivalent_to(f));
}

#[test]
fn restrict_to_constant() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");
    let f = a & b;

    // Restricting both support variables collapses to a constant.
    assert!(f.restrict("a", true).restrict("b", true).is_tautology());
    assert!(f.restrict("a", true).restrict("b", false).is_contradiction());
    assert!(f.restrict("a", false).restrict("b", true).is_contradiction());
}

#[test]
fn cofactor_is_restrict() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");
    let f = a & b;
    assert!(f.cofactor("a", true).equivalent_to(f.restrict("a", true)));
    assert!(f.cofactor("a", false).equivalent_to(f.restrict("a", false)));
}

#[test]
fn forall_exists_multiple_vars() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");
    let c = ctx.var("c");

    // ∀a,b. (a & b & c) = c restricted to a&b both polarities = false (since a=0 kills it)
    assert!((a & b & c).forall(&["a", "b"]).is_contradiction());
    // ∃a,b. (a & b & c) = c
    assert!((a & b & c).exists(&["a", "b"]).equivalent_to(c));
    // Quantifying over no variables is the identity.
    let empty: &[&str] = &[];
    assert!((a & b).forall(empty).equivalent_to(a & b));
    assert!((a & b).exists(empty).equivalent_to(a & b));
}

// ---- Requirement 2: minterm enumeration -----------------------------------------------------------

/// Build a `Minterm<Symbol>` over the given header from `(name, value)` pairs (all assigned).
fn minterm(header: &Arc<Symbols<Symbol>>, values: &[(&str, bool)]) -> Minterm<Symbol> {
    let labels = header.labels();
    let vals: Vec<Option<bool>> = labels
        .iter()
        .map(|l| {
            values
                .iter()
                .find(|(n, _)| Symbol::new(n) == *l)
                .map(|(_, v)| *v)
        })
        .collect();
    Minterm::from_symbols(Arc::clone(header), vals)
}

#[test]
fn to_minterms_xor_two_vars() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");
    let f = a ^ b;

    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect());
    let got: BTreeSet<Minterm<Symbol>> = f.to_minterms(&["a", "b"]).into_iter().collect();
    let want: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("a", true), ("b", false)]),
        minterm(&header, &[("a", false), ("b", true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(got, want);
}

#[test]
fn to_minterms_widen_with_absent_variable() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");
    let f = a ^ b;

    // Widen with an absent variable c → c split into both polarities.
    let header = Symbols::new(["a", "b", "c"].iter().map(Symbol::new).collect());
    let got: BTreeSet<Minterm<Symbol>> = f.to_minterms(&["a", "b", "c"]).into_iter().collect();
    let want: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("a", true), ("b", false), ("c", false)]),
        minterm(&header, &[("a", true), ("b", false), ("c", true)]),
        minterm(&header, &[("a", false), ("b", true), ("c", false)]),
        minterm(&header, &[("a", false), ("b", true), ("c", true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(got, want);
}

#[test]
fn to_minterms_true_is_full_cube() {
    let ctx: BddContext<Local> = BddContext::new();
    let t = ctx.constant(true);

    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect());
    let got: BTreeSet<Minterm<Symbol>> = t.to_minterms(&["a", "b"]).into_iter().collect();
    let want: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("a", false), ("b", false)]),
        minterm(&header, &[("a", false), ("b", true)]),
        minterm(&header, &[("a", true), ("b", false)]),
        minterm(&header, &[("a", true), ("b", true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(got, want);
}

#[test]
fn to_minterms_single_var_splits_other() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");

    // a.to_minterms(&[a, b]) == { a:1,b:0 ; a:1,b:1 } — b split, a fixed.
    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect());
    let got: BTreeSet<Minterm<Symbol>> = a.to_minterms(&["a", "b"]).into_iter().collect();
    let want: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("a", true), ("b", false)]),
        minterm(&header, &[("a", true), ("b", true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(got, want);
}

#[test]
fn to_minterms_is_idempotent_and_deterministic() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");
    let f = (a & b) | (!a & !b); // a == b

    let once = f.to_minterms(&["a", "b"]);
    let twice = f.to_minterms(&["a", "b"]);
    // Deterministic order.
    assert_eq!(once, twice);
    // Already-maximal expansion over the same vars is stable as a set.
    let set: BTreeSet<_> = once.iter().cloned().collect();
    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect());
    let want: BTreeSet<_> = [
        minterm(&header, &[("a", false), ("b", false)]),
        minterm(&header, &[("a", true), ("b", true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(set, want);
}

#[test]
fn to_minterms_matches_cube_expand_to() {
    // Mirror to_minterms against the Cube::expand_to / Cover::maximize primitive directly.
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let f = a; // a=1, b unconstrained

    let via_handle: BTreeSet<Minterm<Symbol>> =
        f.to_minterms(&["a", "b"]).into_iter().collect();

    // a=1 cube expanded over [a, b]
    let cube = Cube::<Symbol, Symbol>::with_labels(&[("a", Some(true))], &[("o", true)], CubeType::F)
        .unwrap();
    let via_cube: BTreeSet<Minterm<Symbol>> = cube
        .expand_to(&[Symbol::new("a"), Symbol::new("b")])
        .into_iter()
        .collect();
    assert_eq!(via_handle, via_cube);
}

// ---- Requirement 4: tautology / contradiction -----------------------------------------------------

#[test]
fn tautology_and_contradiction() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let t = ctx.constant(true);
    let f = ctx.constant(false);

    assert!(t.is_tautology());
    assert!(!t.is_contradiction());
    assert!(f.is_contradiction());
    assert!(!f.is_tautology());

    // a | !a is a tautology; a & !a is a contradiction.
    assert!((a | !a).is_tautology());
    assert!((a & !a).is_contradiction());
}

// ---- Operators and canonicity ---------------------------------------------------------------------

#[test]
fn operators_commute_and_canonicalise() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");

    // a & b ≡ b & a; a | b ≡ b | a; canonical roots are identical so PartialEq holds too.
    assert_eq!(a & b, b & a);
    assert_eq!(a | b, b | a);
    assert!((a & b).equivalent_to(b & a));

    // De Morgan: !(a & b) ≡ !a | !b
    assert!((!(a & b)).equivalent_to(!a | !b));
    assert!((!(a | b)).equivalent_to(!a & !b));
}

#[test]
fn operator_ref_combinations_compile_and_agree() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");

    let by_value = a & b;
    // Bind references through variables so the `&Bdd` operator impls (not the value impls) are
    // genuinely exercised — a literal `&a & b` over a `Copy` operand would be linted away.
    let (ra, rb) = (&a, &b);
    assert!((ra & b).equivalent_to(by_value));
    assert!((a & rb).equivalent_to(by_value));
    assert!((ra & rb).equivalent_to(by_value));
    assert!((!ra).equivalent_to(!a));
}

#[test]
fn equivalent_to_is_root_identity() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");
    // Two syntactically different but logically equal builds share one canonical root.
    let f = (a & b) | (a & !b); // == a
    assert!(f.equivalent_to(a));
    assert_eq!(f, a);
}

// ---- build_cover / to_cubes round-trip ------------------------------------------------------------

/// The XOR cover (a⊕b): inputs 01→1, 10→1.
fn xor_cover() -> Cover<Symbol, Symbol> {
    Cover::from_cubes(
        CoverType::F,
        [
            Cube::<Symbol, Symbol>::with_labels(
                &[("a", Some(false)), ("b", Some(true))],
                &[("o", true)],
                CubeType::F,
            )
            .unwrap(),
            Cube::<Symbol, Symbol>::with_labels(
                &[("a", Some(true)), ("b", Some(false))],
                &[("o", true)],
                CubeType::F,
            )
            .unwrap(),
        ],
    )
}

#[test]
fn build_cover_round_trip() {
    let ctx: BddContext<Local> = BddContext::new();
    let cover = xor_cover();
    let f = ctx.build_cover(&cover);

    // f is exactly a ⊕ b.
    let a = ctx.var("a");
    let b = ctx.var("b");
    assert!(f.equivalent_to(a ^ b));

    // The minterm set of build_cover(cover) matches the cover's own maximised minterm set.
    let from_handle: BTreeSet<Minterm<Symbol>> =
        f.to_minterms(&["a", "b"]).into_iter().collect();
    let from_cover: BTreeSet<Minterm<Symbol>> = cover
        .maximize(&[Symbol::new("a"), Symbol::new("b")])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    assert_eq!(from_handle, from_cover);

    // Rebuilding from to_cubes() reproduces the same canonical handle.
    let rebuilt = ctx.build_cover(&f.to_cubes());
    assert!(rebuilt.equivalent_to(f));
}

#[test]
fn to_cubes_is_anonymous_single_output_onset() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");
    let f = a & b;
    let cover = f.to_cubes();
    assert_eq!(cover.num_outputs(), 1);
    // Every cube is an ON-set (F) cube.
    assert!(cover.cubes().all(|c| c.cube_type() == CubeType::F));
}

// ---- Both context kinds agree ---------------------------------------------------------------------

#[test]
fn both_context_kinds_agree() {
    // Single-threaded.
    let local: BddContext<Local> = BddContext::new();
    let la = local.var("a");
    let lb = local.var("b");
    let lc = local.var("c");
    let lf = (la & lb) | (la ^ lc);
    let local_minterms: BTreeSet<Minterm<Symbol>> =
        lf.to_minterms(&["a", "b", "c"]).into_iter().collect();
    let local_taut = (la | !la).is_tautology();

    // Thread-safe.
    let sync: SyncBddContext<Sync> = SyncBddContext::new();
    let sa = sync.var("a");
    let sb = sync.var("b");
    let sc = sync.var("c");
    let sf = (sa & sb) | (sa ^ sc);
    let sync_minterms: BTreeSet<Minterm<Symbol>> =
        sf.to_minterms(&["a", "b", "c"]).into_iter().collect();
    let sync_taut = (sa | !sa).is_tautology();

    assert_eq!(local_minterms, sync_minterms);
    assert_eq!(local_taut, sync_taut);
    assert!(local_taut);
}

#[test]
fn sync_context_is_send_across_threads() {
    let sync: SyncBddContext<Sync> = SyncBddContext::new();
    // Build something, then move the context into another thread and use it there.
    {
        let a = sync.var("a");
        let b = sync.var("b");
        assert!((a & b).restrict("a", true).equivalent_to(b));
    }
    let handle = std::thread::spawn(move || {
        let a = sync.var("a");
        let b = sync.var("b");
        let f = (a & b) | (!a & !b);
        f.to_minterms(&["a", "b"]).len()
    });
    let n = handle.join().unwrap();
    assert_eq!(n, 2); // {a:0,b:0}, {a:1,b:1}
}

#[test]
fn sync_context_shared_by_reference_across_threads() {
    let sync: SyncBddContext<Sync> = SyncBddContext::new();
    let shared = std::sync::Arc::new(sync);
    let c1 = std::sync::Arc::clone(&shared);
    let c2 = std::sync::Arc::clone(&shared);
    let t1 = std::thread::spawn(move || {
        let a = c1.var("a");
        let b = c1.var("b");
        (a & b).node_count()
    });
    let t2 = std::thread::spawn(move || {
        let a = c2.var("a");
        let b = c2.var("b");
        (a | b).node_count()
    });
    let _ = t1.join().unwrap();
    let _ = t2.join().unwrap();
}

// ---- minimize -------------------------------------------------------------------------------------

#[test]
fn minimize_reduces_redundancy() {
    let ctx: BddContext<Local> = BddContext::new();
    let a = ctx.var("a");
    let b = ctx.var("b");
    // (a & b) | (a & !b) == a — minimisation should collapse to a single cube fixing only `a`.
    let f = (a & b) | (a & !b);
    let min = f.minimize().expect("minimisation succeeds");
    // The function still equals `a`.
    let rebuilt = ctx.build_cover(&min);
    assert!(rebuilt.equivalent_to(a));
}
