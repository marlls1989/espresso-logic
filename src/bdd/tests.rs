//! Tests for the canonical BDD layer.
//!
//! Two in-crate brand types are declared here, `BrandA` and `BrandB`. A brand marks one namespace for
//! uniqueness only; it selects no storage backend, so each pairs freely with either
//! [`LocalCell`](crate::expression::manager_cell::LocalCell) or
//! [`SyncCell`](crate::expression::manager_cell::SyncCell). The sealed [`Brand`] trait permits these
//! in-crate impls; downstream code mints brands through the public `bdd_builder!` / `sync_bdd_builder!`
//! macros.

use super::brand::{brand_seal, Brand};
use super::BddBuilder;
use crate::cover::{Cover, CoverType, Cube, CubeType, Minterm, Symbols};
use crate::expression::manager_cell::{LocalCell, SyncCell};
use crate::Symbol;
use std::collections::BTreeSet;
use std::sync::Arc;

/// First test brand.
#[derive(Clone, Copy)]
struct BrandA;
impl brand_seal::Sealed for BrandA {}
impl Brand for BrandA {}

/// Second test brand.
#[derive(Clone, Copy)]
struct BrandB;
impl brand_seal::Sealed for BrandB {}
impl Brand for BrandB {}

// ---- Send/Sync follows the storage cell, not the brand --------------------------------------------

/// A [`SyncCell`]-backed builder is `Send + Sync`; asserting these bounds compiles only if they hold. The
/// `!Send`/`!Sync` of a [`LocalCell`]-backed builder is checked by `context_send_asymmetry` below.
#[test]
fn sync_context_is_send_and_sync() {
    fn assert_send<T: std::marker::Send>() {}
    fn assert_sync<T: std::marker::Sync>() {}
    assert_send::<BddBuilder<BrandB, SyncCell>>();
    assert_sync::<BddBuilder<BrandB, SyncCell>>();
    // And a handle into it is Send + Sync too.
    assert_send::<super::Bdd<BrandB, SyncCell>>();
    assert_sync::<super::Bdd<BrandB, SyncCell>>();
}

/// Compile-time witness that thread-safety follows the storage cell, not the brand: a
/// [`LocalCell`]-backed builder is `!Send` and a [`SyncCell`]-backed one is `Send`, whatever brand each
/// carries.
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

    // The storage cell determines Send; the brand is irrelevant.
    assert!(is_send!(BddBuilder<BrandB, SyncCell>));
    assert!(!is_send!(BddBuilder<BrandA, LocalCell>));
    // Same brand, opposite cells: the cell alone flips Send.
    assert!(is_send!(BddBuilder<BrandA, SyncCell>));
    assert!(!is_send!(BddBuilder<BrandB, LocalCell>));
}

/// Build a `Minterm<Symbol>` fixing each `(name, value)` pair (every field concrete). Variables not
/// listed are simply absent from the minterm, i.e. left free for [`Bdd::evaluate`].
fn assign(pairs: &[(&str, bool)]) -> Minterm<Symbol> {
    let syms = Symbols::new(pairs.iter().map(|(n, _)| Symbol::from(*n)).collect());
    Minterm::from_symbols(syms, pairs.iter().map(|(_, v)| Some(*v)))
}

// ---- Requirement 1: Shannon cofactor / quantification ---------------------------------------------

// `forall`/`exists` moved from `&[S]` to `impl IntoIterator<Item = S>`; the `vars` binding below is a
// `&[&str]` (not an inline `["a"]` array literal) to prove that pre-existing, borrowed-slice callers
// still compile unmodified against the widened bound — the same guarantee the
// `docs/EXAMPLES.md`/`docs/BOOLEAN_EXPRESSIONS.md` doctests make.
#[test]
fn restrict_acceptance_table() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");

    // (a & b).restrict("a", true) ≡ b
    assert!((a.clone() & b.clone())
        .restrict("a", true)
        .equivalent_to(&b));
    // (a & b).restrict("a", false) ≡ false
    assert!((a.clone() & b.clone())
        .restrict("a", false)
        .is_contradiction());
    // (a | b).restrict("a", false) ≡ b
    assert!((a.clone() | b.clone())
        .restrict("a", false)
        .equivalent_to(&b));
    let vars: &[&str] = &["a"];
    // (a | b).forall(&["a"]) ≡ b
    assert!((a.clone() | b.clone()).forall(vars).equivalent_to(&b));
    // (a & b).exists(&["a"]) ≡ b
    assert!((a.clone() & b.clone()).exists(vars).equivalent_to(&b));
    // (a ^ b).forall(&["a"]) ≡ false
    assert!((a ^ b).forall(vars).is_contradiction());
}

#[test]
fn evaluate_matches_truth_table() {
    use crate::expr;

    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    // f = a & b | !c.
    let expr = expr!("a" & "b" | !"c");
    let f = builder.build(&expr);

    for mask in 0..8u32 {
        let a = mask & 1 == 1;
        let b = (mask >> 1) & 1 == 1;
        let c = (mask >> 2) & 1 == 1;
        let expected = (a && b) || !c;
        // A complete assignment over the support is determined, so evaluation yields `Ok`.
        assert_eq!(
            f.evaluate(&assign(&[("a", a), ("b", b), ("c", c)])),
            Ok(expected)
        );
    }
}

#[test]
fn fold_collects_support_variables() {
    use super::BddNode;
    use std::collections::BTreeSet;

    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let f = (builder.var("a") & builder.var("b")) | builder.var("c");

    // Fold the decision diagram into the set of tested variables (union is sharing-safe). The result
    // must match the handle's own support.
    let vars: BTreeSet<String> = f.fold(|node: BddNode<BTreeSet<String>>| match node {
        BddNode::Terminal(_) => BTreeSet::new(),
        BddNode::Decision {
            variable,
            low,
            high,
        } => {
            let mut set = low;
            set.extend(high);
            set.insert(variable.to_string());
            set
        }
    });
    let expected: BTreeSet<String> = f.variables().map(|s| s.to_string()).collect();
    assert_eq!(vars, expected);
}

#[test]
fn fold_with_context_evaluates_via_path_descent() {
    use super::BddNode;
    use std::collections::HashMap;

    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let f = (builder.var("a") & builder.var("b")) | !builder.var("c");

    // Re-implement evaluation with the top-down builder carrying the assignment: descend selects the
    // branch for each variable, combine reads it back. Must agree with Bdd::evaluate.
    for mask in 0..8u32 {
        let a = mask & 1 == 1;
        let b = (mask >> 1) & 1 == 1;
        let c = (mask >> 2) & 1 == 1;
        let assignment: HashMap<Symbol, bool> = [("a", a), ("b", b), ("c", c)]
            .into_iter()
            .map(|(name, value)| (Symbol::from(name), value))
            .collect();

        let via_fold = f.fold_with_context(
            (),
            |_node, ()| ((), ()),
            |node, ()| match node {
                BddNode::Terminal(value) => value,
                BddNode::Decision {
                    variable,
                    low,
                    high,
                } => {
                    let set = assignment
                        .get(&Symbol::from(variable))
                        .copied()
                        .unwrap_or(false);
                    if set {
                        high
                    } else {
                        low
                    }
                }
            },
        );
        // A complete assignment is determined, so unwrap the `Ok`.
        assert_eq!(
            via_fold,
            f.evaluate(&assign(&[("a", a), ("b", b), ("c", c)]))
                .unwrap()
        );
    }
}

#[test]
fn fold_closure_may_reenter_builder() {
    use super::BddNode;

    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let f = builder.var("a") & builder.var("b");

    // The read guard is released before the fold closure runs, so the closure may re-enter the builder
    // (which locks the same cell) without double-borrowing the LocalCell's RefCell.
    let count = f.fold(|node: BddNode<usize>| match node {
        BddNode::Terminal(_) => {
            let _ = builder.var("reentrant");
            1
        }
        BddNode::Decision { low, high, .. } => {
            let _ = builder.constant(true);
            low + high + 1
        }
    });
    assert!(count >= 1);
}

#[test]
fn fold_with_context_closures_may_reenter_builder() {
    use super::BddNode;

    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let f = builder.var("a") | builder.var("b");

    // Both the descend and combine closures re-enter the builder; this must not deadlock or
    // double-borrow now that the guard is released before either runs.
    let leaves = f.fold_with_context(
        (),
        |_node, ()| {
            let _ = builder.var("reentrant_descend");
            ((), ())
        },
        |node, ()| match node {
            BddNode::Terminal(_) => {
                let _ = builder.constant(false);
                1usize
            }
            BddNode::Decision { low, high, .. } => low + high,
        },
    );
    assert!(leaves >= 1);
}

#[test]
fn evaluate_partial_returns_residual() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a & b.clone();

    // Only `a` fixed (true): the function still depends on the free `b`, so evaluation is *partial* —
    // no silent default. The residual is the manual cofactor f|a=1 == b.
    let residual = f.evaluate(&assign(&[("a", true)])).unwrap_err();
    assert!(residual.equivalent_to(&f.restrict("a", true)));
    assert!(residual.equivalent_to(&b));

    // Fixing `a` to false determines the conjunction outright → Ok(false).
    assert_eq!(f.evaluate(&assign(&[("a", false)])), Ok(false));

    // A constant ignores the (empty) assignment and is determined.
    let empty = assign(&[]);
    assert_eq!(builder.constant(true).evaluate(&empty), Ok(true));
    assert_eq!(builder.constant(false).evaluate(&empty), Ok(false));
}

#[test]
fn evaluate_complete_minterm_matches_truth_table() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let f = builder.var("a") & builder.var("b");

    // A complete minterm over the support is always determined → Ok matching the truth table.
    for &av in &[false, true] {
        for &bv in &[false, true] {
            assert_eq!(f.evaluate(&assign(&[("a", av), ("b", bv)])), Ok(av && bv));
        }
    }
}

#[test]
fn restrict_absent_variable_is_noop() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = a & b; // depends only on a, b

    // c.restrict("a", true) ≡ c  — restricting an absent variable is a no-op
    assert!(c.restrict("a", true).equivalent_to(&c));
    // restricting a variable absent from `f` leaves f unchanged
    assert!(f.restrict("z", true).equivalent_to(&f));
    assert!(f.restrict("z", false).equivalent_to(&f));
}

#[test]
fn restrict_to_constant() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a & b;

    // Restricting both support variables collapses to a constant.
    assert!(f.restrict("a", true).restrict("b", true).is_tautology());
    assert!(f
        .restrict("a", true)
        .restrict("b", false)
        .is_contradiction());
    assert!(f
        .restrict("a", false)
        .restrict("b", true)
        .is_contradiction());
}

#[test]
fn cofactor_is_restrict() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a & b;
    assert!(f.cofactor("a", true).equivalent_to(&f.restrict("a", true)));
    assert!(f
        .cofactor("a", false)
        .equivalent_to(&f.restrict("a", false)));
}

// See the comment on `restrict_acceptance_table` above: `vars` is a `&[&str]` binding, kept
// deliberately unchanged (not inlined) to prove the widened `impl IntoIterator` bound still accepts a
// borrowed slice.
#[test]
fn forall_exists_multiple_vars() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");

    let vars: &[&str] = &["a", "b"];
    // ∀a,b. (a & b & c) = c restricted to a&b both polarities = false (since a=0 kills it)
    assert!((a.clone() & b.clone() & c.clone())
        .forall(vars)
        .is_contradiction());
    // ∃a,b. (a & b & c) = c
    assert!((a.clone() & b.clone() & c.clone())
        .exists(vars)
        .equivalent_to(&c));
    // Quantifying over no variables is the identity.
    let empty: &[&str] = &[];
    assert!((a.clone() & b.clone())
        .forall(empty)
        .equivalent_to(&(a.clone() & b.clone())));
    assert!((a.clone() & b.clone())
        .exists(empty)
        .equivalent_to(&(a & b)));
}

#[test]
fn forall_exists_accept_owned_iterator_and_adaptor() {
    // `forall`/`exists` take `impl IntoIterator<Item: AsRef<str>>`, not just borrowed slices: an owned
    // `Vec<String>` and an arbitrary adaptor chain both work.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");

    // Owned iterator: a `Vec<String>` passed by value.
    let owned: Vec<String> = vec![String::from("a"), String::from("b")];
    assert!((a.clone() & b.clone() & c.clone())
        .forall(owned)
        .is_contradiction());

    // Adaptor chain: filter an iterator of names down to the ones actually being quantified.
    let names = ["a", "b", "z"];
    let adaptor = names.iter().filter(|&&n| n != "z");
    assert!((a.clone() & b.clone() & c.clone())
        .exists(adaptor)
        .equivalent_to(&c));
}

#[test]
fn forall_over_deep_chain_no_overflow() {
    // `forall`/`cofactor`/`exists` funnel through the now-iterative `restrict`; quantifying over the
    // *bottom* variable of a deep AND chain makes restrict walk the whole chain without overflowing
    // the call stack.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let n = 2000usize;
    let names: Vec<String> = (0..n).map(|i| format!("v{i}")).collect();
    let mut f = builder.var(&names[0]);
    for name in &names[1..] {
        f = f & builder.var(name);
    }
    // The conjunction cannot hold for both polarities of any variable, so ∀(bottom var) is false.
    assert!(f
        .forall(std::slice::from_ref(&names[n - 1]))
        .is_contradiction());
    // Restricting the bottom variable to true drops just it, leaving a non-constant conjunction.
    assert!(!f.restrict(&names[n - 1], true).is_contradiction());
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
fn maximize_xor_two_vars() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a ^ b;

    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect());
    let got: BTreeSet<Minterm<Symbol>> = f
        .maximize(&["a", "b"])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    let want: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("a", true), ("b", false)]),
        minterm(&header, &[("a", false), ("b", true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(got, want);
}

#[test]
fn maximize_widen_with_absent_variable() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a ^ b;

    // Widen with an absent variable c → c split into both polarities.
    let header = Symbols::new(["a", "b", "c"].iter().map(Symbol::new).collect());
    let got: BTreeSet<Minterm<Symbol>> = f
        .maximize(&["a", "b", "c"])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
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
fn maximize_subset_header_dedups() {
    // Regression: a header that omits a support variable projects distinct cubes onto the same
    // minterm, so `maximize` must deduplicate. f = (a & b) | (!a & c); to_cubes = {a1 b1}, {a0 c1}.
    // Over [b, c] both expansions include b1c1, which must appear exactly once.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = (a.clone() & b.clone()) | (!a & c.clone());

    let got: Vec<Minterm<Symbol>> = f
        .maximize(&["b", "c"])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    // No duplicates: the raw sequence and the deduplicated set have the same length.
    let set: BTreeSet<Minterm<Symbol>> = got.iter().cloned().collect();
    assert_eq!(got.len(), set.len(), "maximize must not repeat minterms");

    let header = Symbols::new(["b", "c"].iter().map(Symbol::new).collect());
    let want: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("b", true), ("c", false)]),
        minterm(&header, &[("b", true), ("c", true)]),
        minterm(&header, &[("b", false), ("c", true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(set, want);
}

#[test]
fn maximize_true_is_full_cube() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let t = builder.constant(true);

    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect());
    let got: BTreeSet<Minterm<Symbol>> = t
        .maximize(&["a", "b"])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
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
fn maximize_single_var_splits_other() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");

    // a.maximize(&[a, b]) == { a:1,b:0 ; a:1,b:1 } — b split, a fixed.
    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect());
    let got: BTreeSet<Minterm<Symbol>> = a
        .maximize(&["a", "b"])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    let want: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("a", true), ("b", false)]),
        minterm(&header, &[("a", true), ("b", true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(got, want);
}

#[test]
fn maximize_is_idempotent_and_deterministic() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = (a.clone() & b.clone()) | (!a & !b); // a == b

    let once: Vec<_> = f
        .maximize(&["a", "b"])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    let twice: Vec<_> = f
        .maximize(&["a", "b"])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    // Deterministic order (same function + vars → same traversal → same sequence).
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
fn maximize_matches_cube_expand_to() {
    // Mirror maximize against the Cube::expand_to / Cover::maximize primitive directly.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let f = a; // a=1, b unconstrained

    let via_handle: BTreeSet<Minterm<Symbol>> = f
        .maximize(&["a", "b"])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();

    // a=1 cube expanded over [a, b]
    let cube =
        Cube::<Symbol, Symbol>::with_labels(&[("a", Some(true))], &[("o", true)], CubeType::F)
            .unwrap();
    let via_cube: BTreeSet<Minterm<Symbol>> = cube
        .expand_to(&[Symbol::new("a"), Symbol::new("b")])
        .collect();
    assert_eq!(via_handle, via_cube);
}

// ---- Requirement 4: tautology / contradiction -----------------------------------------------------

#[test]
fn tautology_and_contradiction() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let t = builder.constant(true);
    let f = builder.constant(false);

    assert!(t.is_tautology());
    assert!(!t.is_contradiction());
    assert!(f.is_contradiction());
    assert!(!f.is_tautology());

    // a | !a is a tautology; a & !a is a contradiction.
    assert!((a.clone() | !a.clone()).is_tautology());
    assert!((a.clone() & !a).is_contradiction());
}

// ---- Operators and canonicity ---------------------------------------------------------------------

#[test]
fn operators_commute_and_canonicalise() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");

    // a & b ≡ b & a; a | b ≡ b | a; canonical roots are identical so PartialEq holds too.
    assert_eq!(a.clone() & b.clone(), b.clone() & a.clone());
    assert_eq!(a.clone() | b.clone(), b.clone() | a.clone());
    assert!((a.clone() & b.clone()).equivalent_to(&(b.clone() & a.clone())));

    // De Morgan: !(a & b) ≡ !a | !b
    assert!((!(a.clone() & b.clone())).equivalent_to(&(!a.clone() | !b.clone())));
    assert!((!(a.clone() | b.clone())).equivalent_to(&(!a & !b)));
}

#[test]
fn operator_ref_combinations_compile_and_agree() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");

    let by_value = a.clone() & b.clone();
    // Bind references through variables so the `&Bdd` operator impls (not the value impls) are
    // genuinely exercised.
    let (ra, rb) = (&a, &b);
    assert!((ra & b.clone()).equivalent_to(&by_value));
    assert!((a.clone() & rb).equivalent_to(&by_value));
    assert!((ra & rb).equivalent_to(&by_value));
    assert!((!ra).equivalent_to(&!a));
}

#[test]
fn complement_not_and_operator_agree() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");

    // `complement` and `not` are public aliases of each other and of the unary `!` operator; all three
    // negate the function, and negating twice returns the original.
    let by_complement = a.complement();
    assert!(by_complement.equivalent_to(&a.not()));
    assert!(by_complement.equivalent_to(&!&a));
    assert!(a.complement().complement().equivalent_to(&a));
}

#[test]
fn hash_agrees_with_canonical_equality() {
    use std::collections::HashSet;

    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");

    // Syntactically different but logically equal builds are `==` (canonical roots), so they must hash
    // equal too and collapse to one entry in a `HashSet`.
    let by_value = a.clone() & b.clone();
    let by_commuted_value = b.clone() & a.clone();
    assert_eq!(by_value, by_commuted_value);

    // `Bdd`'s `Hash`/`Eq` are pointer-identity (the manager cell's address) plus the canonical root id —
    // never a walk of the manager's interior state — so it is a sound `HashSet` key despite `ManagerCell`
    // structurally containing a `RefCell`/`RwLock`: nothing hashed here can change after insertion in a
    // way that would move a key to a different bucket. This is the standard `Rc`/`Arc`-pointer-identity
    // hashing pattern, which `clippy::mutable_key_type` cannot see through.
    #[allow(clippy::mutable_key_type)]
    let mut set = HashSet::new();
    assert!(set.insert(by_value.clone()));
    assert!(!set.insert(by_commuted_value));
    assert_eq!(set.len(), 1);

    // A distinct function hashes (and lands) separately.
    assert!(set.insert(a.clone()));
    assert_eq!(set.len(), 2);
    assert!(set.contains(&by_value));
    assert!(set.contains(&a));
}

#[test]
fn equivalent_to_is_root_identity() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    // Two syntactically different but logically equal builds share one canonical root.
    let f = (a.clone() & b.clone()) | (a.clone() & !b); // == a
    assert!(f.equivalent_to(&a));
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
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let cover = xor_cover();
    let f = builder.build_cover(&cover);

    // f is exactly a ⊕ b.
    let a = builder.var("a");
    let b = builder.var("b");
    assert!(f.equivalent_to(&(a ^ b)));

    // The minterm set of build_cover(cover) matches the cover's own maximised minterm set.
    let from_handle: BTreeSet<Minterm<Symbol>> = f
        .maximize(&["a", "b"])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    let from_cover: BTreeSet<Minterm<Symbol>> = cover
        .maximize(&[Symbol::new("a"), Symbol::new("b")])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    assert_eq!(from_handle, from_cover);

    // Rebuilding from to_cubes() reproduces the same canonical handle.
    let rebuilt = builder.build_cover(&f.to_cubes());
    assert!(rebuilt.equivalent_to(&f));
}

#[test]
fn contradiction_lowers_without_panicking() {
    use crate::BoolExpr;

    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let f = a.clone() & !a; // a & !a — the constant false
    assert!(f.is_contradiction());

    // to_cubes keeps the arity-1 anonymous output header, so the cover is one output with zero cubes
    // (rather than a re-derived zero-output header that would break to_expr_by_index(0)).
    let cover = f.to_cubes();
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.num_cubes(), 0);

    // to_expr therefore lowers a contradiction to the constant false ("0") with no panic.
    assert_eq!(f.to_expr(), BoolExpr::constant(false));
    assert_eq!(f.to_expr().to_string(), "0");

    // The From<Bdd>/From<BoolExpr>/minimize paths that funnel through to_cubes also stay sound.
    let from_bdd: Cover<Symbol, crate::Anonymous> = (&f).into();
    assert_eq!(from_bdd.num_outputs(), 1);
    assert!(f.minimize().is_ok());
}

#[test]
fn to_cubes_is_anonymous_single_output_onset() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a & b;
    let cover = f.to_cubes();
    assert_eq!(cover.num_outputs(), 1);
    // Every cube is an ON-set (F) cube.
    assert!(cover.cubes().all(|c| c.cube_type() == CubeType::F));
}

// ---- Both builder kinds agree ---------------------------------------------------------------------

#[test]
fn both_context_kinds_agree() {
    // Single-threaded.
    let local: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let la = local.var("a");
    let lb = local.var("b");
    let lc = local.var("c");
    let lf = (la.clone() & lb) | (la.clone() ^ lc);
    let local_minterms: BTreeSet<Minterm<Symbol>> = lf
        .maximize(&["a", "b", "c"])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    let local_taut = (la.clone() | !la).is_tautology();

    // Thread-safe.
    let sync: BddBuilder<BrandB, SyncCell> = BddBuilder::new();
    let sa = sync.var("a");
    let sb = sync.var("b");
    let sc = sync.var("c");
    let sf = (sa.clone() & sb) | (sa.clone() ^ sc);
    let sync_minterms: BTreeSet<Minterm<Symbol>> = sf
        .maximize(&["a", "b", "c"])
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    let sync_taut = (sa.clone() | !sa).is_tautology();

    assert_eq!(local_minterms, sync_minterms);
    assert_eq!(local_taut, sync_taut);
    assert!(local_taut);
}

#[test]
fn sync_context_is_send_across_threads() {
    let sync: BddBuilder<BrandB, SyncCell> = BddBuilder::new();
    // Build something, then move the builder into another thread and use it there.
    {
        let a = sync.var("a");
        let b = sync.var("b");
        assert!((a & b.clone()).restrict("a", true).equivalent_to(&b));
    }
    let handle = std::thread::spawn(move || {
        let a = sync.var("a");
        let b = sync.var("b");
        let f = (a.clone() & b.clone()) | (!a & !b);
        // Each cube of the maximal cover is one minterm, so the cube count is the minterm count.
        f.maximize(&["a", "b"]).cubes().count()
    });
    let n = handle.join().unwrap();
    assert_eq!(n, 2); // {a:0,b:0}, {a:1,b:1}
}

#[test]
fn sync_context_shared_by_reference_across_threads() {
    let sync: BddBuilder<BrandB, SyncCell> = BddBuilder::new();
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

// ---- recovering the builder from a handle ---------------------------------------------------------

/// `Bdd::builder` recovers a builder onto the *same* manager, even after the original builder is
/// dropped: handles it mints share the brand and manager, so they combine with the stored handle (no
/// `assert_same_manager` panic) and a rebuilt function is canonically equal to the original.
#[test]
fn builder_recovers_the_same_manager() {
    // Build a handle, then drop the builder that minted it.
    let a = {
        let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
        builder.var("a")
    };

    // Recover a builder onto the same manager and derive further handles.
    let builder = a.builder();
    let b = builder.var("b");

    // Combining the recovered builder's handle with the stored one type-checks and computes.
    let f = &a & &b;
    assert!(f.equivalent_to(&builder.parse("a & b").unwrap()));
    // And the recovered builder builds `a` to the very same canonical handle as the stored one.
    assert!(builder.var("a").equivalent_to(&a));
}

/// The same round trip over the `SyncCell` backend.
#[test]
fn builder_recovers_the_same_manager_sync() {
    let a = {
        let builder: BddBuilder<BrandB, SyncCell> = BddBuilder::new();
        builder.var("a")
    };

    let builder = a.builder();
    let b = builder.var("b");

    assert!((&a & &b).equivalent_to(&builder.parse("a & b").unwrap()));
    assert!(builder.var("a").equivalent_to(&a));
}

// ---- scoped, by-reference builder -----------------------------------------------------------------

/// `BddBuilder::scope` composes `Copy`, by-reference handles and returns the owned root, which equals the
/// same function built with the owned operators.
#[test]
fn scope_composes_without_clone() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    // (a ^ b) & !c, composed from Copy handles — no `.clone()`, an operand named twice for free.
    let f = builder.scope(|s| {
        let a = s.var("a");
        (a ^ s.var("b")) & !s.var("c")
    });
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    assert!(f.equivalent_to(&((a ^ b) & !c)));
}

/// The owned `Bdd` returned by `scope` shares the builder's brand and manager, so it combines with
/// further handles the builder mints.
#[test]
fn scope_returns_interoperable_owned() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let f = builder.scope(|s| s.var("a") & s.var("b"));
    let g = builder.var("c");
    // Combining the scope's result with a fresh owned handle type-checks and computes.
    let h = &f | &g;
    assert!(h.equivalent_to(&builder.parse("(a & b) | c").unwrap()));
}

/// `Scope::lift` splices an existing owned `Bdd` into the scope; the result equals composing that handle
/// directly.
#[test]
fn scope_lift_round_trips() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let lifted = builder.scope(|s| s.lift(&a) & s.var("b"));
    assert!(lifted.equivalent_to(&(a & builder.var("b"))));
}

/// `Scope::build` / `Scope::parse` agree with the owned `BddBuilder::build` / `BddBuilder::parse`.
#[test]
fn scope_build_and_parse_agree_with_owned() {
    use crate::BoolExpr;
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let expr = BoolExpr::parse("(a | b) & !c").unwrap();
    let built = builder.scope(|s| s.build(&expr));
    let parsed = builder.scope(|s| s.parse("(a | b) & !c").unwrap());
    assert!(built.equivalent_to(&builder.build(&expr)));
    assert!(parsed.equivalent_to(&built));
}

/// The same round trip over the `SyncCell` backend.
#[test]
fn scope_composes_on_sync_cell() {
    let builder: BddBuilder<BrandB, SyncCell> = BddBuilder::new();
    let f = builder.scope(|s| (s.var("a") ^ s.var("b")) & !s.var("c"));
    assert!(f.equivalent_to(&builder.parse("(a ^ b) & !c").unwrap()));
}

/// `ScopedBdd`'s `|` operator and `Scope::constant` compose directly inside a closure: `a | false == a`
/// and `a | true` is a tautology.
#[test]
fn scope_or_and_constant_compose() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    // `|` between two scoped handles.
    let or = builder.scope(|s| s.var("a") | s.var("b"));
    assert!(or.equivalent_to(&builder.parse("a | b").unwrap()));
    // `s.constant(false)` is the OR identity; `s.constant(true)` saturates.
    let with_false = builder.scope(|s| s.var("a") | s.constant(false));
    assert!(with_false.equivalent_to(&builder.var("a")));
    let with_true = builder.scope(|s| s.var("a") | s.constant(true));
    assert!(with_true.is_tautology());
}

/// A `ScopedBdd` is `Copy`, so an operand can be named twice with no `.clone()`: `a | !a` is a tautology
/// and `a & !a` a contradiction, both reusing the single handle `a`.
#[test]
fn scope_operand_reused_without_clone() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    assert!(builder
        .scope(|s| {
            let a = s.var("a");
            a | !a
        })
        .is_tautology());
    assert!(builder
        .scope(|s| {
            let a = s.var("a");
            a & !a
        })
        .is_contradiction());
}

// ---- minimize -------------------------------------------------------------------------------------

#[test]
fn minimize_reduces_redundancy() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    // (a & b) | (a & !b) == a — minimisation should collapse to a single cube fixing only `a`.
    let f = (a.clone() & b.clone()) | (a.clone() & !b);
    let min = f.minimize().expect("minimisation succeeds");
    // The function still equals `a`.
    let rebuilt = builder.build_cover(&min);
    assert!(rebuilt.equivalent_to(&a));
}

// ---- Brand clash: the runtime same-manager backstop -----------------------------------------------
//
// `bdd_builder!` mints a brand per call *site*: the brand is a local `struct` defined once where the macro
// expands. Wrapping one invocation in a closure and calling it twice therefore yields two builders with the
// *same* brand type but *different* managers, whose handles type-check together. The always-on same-manager
// assert is the only guard against then computing across the two managers; these tests build exactly that
// clash and prove each assert fires (so removing the assert would let the bug through silently).

#[test]
#[should_panic(expected = "different managers")]
fn owned_operator_across_clashing_brands_panics() {
    let make = || crate::bdd_builder!();
    let one = make();
    let two = make();
    // One call site, so `one` and `two` share a brand type but own different managers: this type-checks,
    // then trips the runtime backstop.
    let _ = one.var("x") & two.var("x");
}

#[test]
#[should_panic(expected = "different manager")]
fn lift_across_clashing_brands_panics() {
    let make = || crate::bdd_builder!();
    let one = make();
    let foreign = make().var("x");
    let _ = one.scope(|s| s.lift(&foreign));
}
