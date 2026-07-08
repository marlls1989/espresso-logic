//! Tests for the canonical BDD layer.
//!
//! Two in-crate brand types are declared here, `BrandA` and `BrandB`. A brand marks one namespace for
//! uniqueness only; it selects no storage backend, so each pairs freely with either
//! [`LocalCell`](crate::bdd::manager_cell::LocalCell) or
//! [`SyncCell`](crate::bdd::manager_cell::SyncCell). The sealed [`Brand`] trait permits these
//! in-crate impls; downstream code mints brands through the public `bdd_builder!` / `sync_bdd_builder!`
//! macros.

use super::brand::{brand_seal, Brand};
use super::BddBuilder;
use crate::bdd::manager_cell::{LocalCell, SyncCell};
use crate::cover::{Cover, CoverType, Cube, CubeType, InputField, Minterm, Symbols};
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

/// `S` is a phantom marker realised only at output boundaries, so it carries no bearing on Send/Sync: a
/// [`SyncCell`]-backed builder/handle stays `Send + Sync` under a non-`Symbol` stored label type too.
#[test]
fn sync_context_is_send_and_sync_under_non_symbol_label() {
    fn assert_send<T: std::marker::Send>() {}
    fn assert_sync<T: std::marker::Sync>() {}
    assert_send::<BddBuilder<BrandB, SyncCell, String>>();
    assert_sync::<BddBuilder<BrandB, SyncCell, String>>();
    assert_send::<super::Bdd<BrandB, SyncCell, String>>();
    assert_sync::<super::Bdd<BrandB, SyncCell, String>>();
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
    let syms = Symbols::new(pairs.iter().map(|(n, _)| Symbol::from(*n)).collect()).unwrap();
    Minterm::from_symbols(syms, pairs.iter().map(|(_, v)| Some(*v)))
}

/// Build a `Minterm<Symbol>` from `(name, value)` pairs where `value` may be `None` — a variable
/// present in the header but left free — rather than always fixed like [`assign`].
fn assign_partial(pairs: &[(&str, Option<bool>)]) -> Minterm<Symbol> {
    let syms = Symbols::new(pairs.iter().map(|(n, _)| Symbol::from(*n)).collect()).unwrap();
    Minterm::from_symbols(syms, pairs.iter().map(|(_, v)| *v))
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
fn evaluate_fast_collapsing_partial() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = (a.clone() & b.clone()) | (!a.clone() & b.clone());

    // f is semantically just `b`; fixing b alone determines the result even though a is left free
    // (absent from the assignment).
    assert_eq!(f.evaluate_fast(&assign(&[("b", true)])), Some(true));
    assert_eq!(f.evaluate_fast(&assign(&[("b", false)])), Some(false));
}

#[test]
fn evaluate_fast_empty_assignment() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a & b;
    let empty = assign(&[]);

    // A non-constant function under an empty assignment is undetermined.
    assert_eq!(f.evaluate_fast(&empty), None);

    // A constant ignores the (empty) assignment and is always determined.
    assert_eq!(builder.constant(true).evaluate_fast(&empty), Some(true));
    assert_eq!(builder.constant(false).evaluate_fast(&empty), Some(false));
}

#[test]
fn evaluate_treats_empty_field_as_free() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let x = builder.var("x");
    let f = a.clone() & x.clone();

    let mut m = Minterm::<Symbol>::with_labels(&[("a", Some(true)), ("x", Some(true))]).unwrap();
    assert_eq!(f.evaluate_fast(&m), Some(true));
    assert_eq!(f.evaluate(&m), Ok(true));

    // Blanking x to the empty literal (`?`) folds to don't-care via the value view (`iter`), leaving
    // x free — the same as if x were absent from the assignment.
    m.set_field_of("x", InputField::Empty).unwrap();
    assert_eq!(f.evaluate_fast(&m), None);
    let residual = f.evaluate(&m).unwrap_err();
    assert!(residual.equivalent_to(&x));
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

#[test]
fn restrict_many_agrees_with_restrict_chain_exhaustive() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let d = builder.var("d");
    let f = (a.clone() & b.clone()) | (!a.clone() & c.clone() & d.clone());

    let names = ["a", "b", "c", "d"];
    let states = [None, Some(true), Some(false)];
    // All 3^4 = 81 partial assignments over a, b, c, d (each variable unset / true / false).
    for &s0 in &states {
        for &s1 in &states {
            for &s2 in &states {
                for &s3 in &states {
                    let pairs: Vec<(&str, Option<bool>)> =
                        names.into_iter().zip([s0, s1, s2, s3]).collect();

                    // Oracle: chain single restricts over the fixed variables, in any order — restrict
                    // is commutative across distinct variables.
                    let mut expected = f.clone();
                    for (name, value) in pairs.iter().copied() {
                        if let Some(v) = value {
                            expected = expected.restrict(name, v);
                        }
                    }

                    let fixed: Vec<(&str, bool)> = pairs
                        .iter()
                        .copied()
                        .filter_map(|(name, value)| value.map(|v| (name, v)))
                        .collect();
                    assert_eq!(f.restrict_many(fixed), expected);

                    let m = assign_partial(&pairs);
                    // The minterm-keyed entry point must agree with the same oracle.
                    assert_eq!(f.restrict_to(&m), expected);
                    let oracle = if expected.is_tautology() {
                        Some(true)
                    } else if expected.is_contradiction() {
                        Some(false)
                    } else {
                        None
                    };
                    assert_eq!(f.evaluate_fast(&m), oracle);

                    match f.evaluate(&m) {
                        Ok(b) => assert_eq!(Some(b), oracle),
                        Err(residual) => {
                            assert_eq!(oracle, None);
                            assert_eq!(residual, expected);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn restrict_many_empty_and_absent() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a.clone() & b.clone();

    // An empty assignment is a no-op.
    let empty: Vec<(&str, bool)> = Vec::new();
    assert_eq!(f.restrict_many(empty), f);

    // An assignment naming only absent variables is also a no-op.
    assert_eq!(f.restrict_many([("zzz", true), ("yyy", false)]), f);

    // A repeated name takes its last entry.
    assert_eq!(
        f.restrict_many([("a", true), ("a", false)]),
        f.restrict("a", false)
    );
}

#[test]
fn restrict_to_normalises_minterm_by_name() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = (a.clone() & b.clone()) | (a.clone() ^ c.clone());

    // A minterm carrying an unknown name ("zzz"), a don't-care ("b"), and its fixed vars in an order
    // different from the manager's VarId order: alignment is by name (unknown dropped, don't-care free).
    let m = Minterm::<Symbol>::with_labels(&[
        ("c", Some(false)),
        ("zzz", Some(true)),
        ("a", Some(true)),
        ("b", None),
    ])
    .unwrap();
    assert!(f
        .restrict_to(&m)
        .equivalent_to(&f.restrict_many([("a", true), ("c", false)])));

    // An all-don't-care / all-unknown minterm is a whole no-op.
    let free = Minterm::<Symbol>::with_labels(&[("b", None), ("zzz", Some(true))]).unwrap();
    assert!(f.restrict_to(&free).equivalent_to(&f));
}

#[test]
fn restrict_many_on_sync_cell_agrees() {
    let local: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let local_combo: BTreeSet<Minterm<Symbol>> = {
        let a = local.var("a");
        let b = local.var("b");
        let c = local.var("c");
        let d = local.var("d");
        let f = (a.clone() & b.clone()) | (!a.clone() & c.clone() & d.clone());
        f.restrict_many([("a", true), ("c", false)])
            .maximize()
            .cubes()
            .map(|cube| cube.inputs().clone())
            .collect()
    };
    let sync: BddBuilder<BrandB, SyncCell> = BddBuilder::new();
    let sync_combo: BTreeSet<Minterm<Symbol>> = {
        let a = sync.var("a");
        let b = sync.var("b");
        let c = sync.var("c");
        let d = sync.var("d");
        let f = (a.clone() & b.clone()) | (!a.clone() & c.clone() & d.clone());
        f.restrict_many([("a", true), ("c", false)])
            .maximize()
            .cubes()
            .map(|cube| cube.inputs().clone())
            .collect()
    };
    assert_eq!(local_combo, sync_combo);
}

/// Regression cover for the `restrict_many` re-entrancy bug: the pre-fix engine consumed the caller's
/// iterator *while* holding the manager guard, so a lazy adaptor that touched the manager mid-iteration
/// re-borrowed it — a `RefCell` "already borrowed" panic on [`LocalCell`] and a deadlock on
/// [`SyncCell`]. The fix drains the iterator before taking the guard. Parameterised over the cell so both
/// backends are exercised.
fn lazy_reentrant_restrict_many_body<B: Brand, C: crate::bdd::ManagerCell>(
    builder: BddBuilder<B, C>,
) {
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = (a.clone() & b.clone()) | (!a.clone() & c.clone());

    // The `.map` closure interns a fresh variable on the first pull, so the manager is borrowed *while*
    // `restrict_many` is consuming the iterator. Pre-fix this reborrowed the held guard.
    let restricted = f.restrict_many([("a", true), ("c", false)].into_iter().map(
        |(name, value)| {
            let _ = builder.var("scratch");
            (name, value)
        },
    ));

    // Reaching here at all proves no panic / deadlock; the result matches the eager slice call.
    assert!(restricted.equivalent_to(&f.restrict_many([("a", true), ("c", false)])));
}

#[test]
fn restrict_many_lazy_reentrant_iterator_local_cell() {
    lazy_reentrant_restrict_many_body(BddBuilder::<BrandA, LocalCell>::new());
}

#[test]
fn restrict_many_lazy_reentrant_iterator_sync_cell() {
    lazy_reentrant_restrict_many_body(BddBuilder::<BrandB, SyncCell>::new());
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

    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect()).unwrap();
    let got: BTreeSet<Minterm<Symbol>> = f.maximize().cubes().map(|c| c.inputs().clone()).collect();
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
    let header = Symbols::new(["a", "b", "c"].iter().map(Symbol::new).collect()).unwrap();
    let got: BTreeSet<Minterm<Symbol>> = f
        .cover_over(["a", "b", "c"])
        .maximize()
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
fn cover_over_subset_projects_universally() {
    // Projecting away a support variable is universal, not existential: f = (a & b) | (!a & c);
    // projecting onto {b, c} (eliminating a) yields ∀a.f = f(a=1) & f(a=0) = b & c — a single
    // minterm, not the union of the two on-set expansions.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = (a.clone() & b.clone()) | (!a & c.clone());

    let got: BTreeSet<Minterm<Symbol>> = f
        .cover_over(["b", "c"])
        .maximize()
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();

    let header = Symbols::new(["b", "c"].iter().map(Symbol::new).collect()).unwrap();
    let want: BTreeSet<Minterm<Symbol>> = [minterm(&header, &[("b", true), ("c", true)])]
        .into_iter()
        .collect();
    assert_eq!(got, want);
}

/// `cover_over`'s `vars` names a variable *set*: a repeated name is deduplicated, so the projection
/// is unaffected.
#[test]
fn cover_over_deduplicates_repeated_variable_name() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a ^ b;

    let with_dup = f.cover_over(["a", "b", "a"]);
    let without_dup = f.cover_over(["a", "b"]);
    assert_eq!(with_dup, without_dup);
}

#[test]
fn maximize_true_is_full_cube() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let t = builder.constant(true);

    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect()).unwrap();
    let got: BTreeSet<Minterm<Symbol>> = t
        .cover_over(["a", "b"])
        .maximize()
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

    // a.cover_over(&[a, b]).maximize() == { a:1,b:0 ; a:1,b:1 } — b split, a fixed.
    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect()).unwrap();
    let got: BTreeSet<Minterm<Symbol>> = a
        .cover_over(["a", "b"])
        .maximize()
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

    let once: Vec<_> = f.maximize().cubes().map(|c| c.inputs().clone()).collect();
    let twice: Vec<_> = f.maximize().cubes().map(|c| c.inputs().clone()).collect();
    // Deterministic order (same function + vars → same traversal → same sequence).
    assert_eq!(once, twice);
    // Already-maximal expansion over the same vars is stable as a set.
    let set: BTreeSet<_> = once.iter().cloned().collect();
    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect()).unwrap();
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
        .cover_over(["a", "b"])
        .maximize()
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

// ---- build_cover / cover round-trip -----------------------------------------------------------------

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
    let from_handle: BTreeSet<Minterm<Symbol>> =
        f.maximize().cubes().map(|c| c.inputs().clone()).collect();
    let from_cover: BTreeSet<Minterm<Symbol>> = cover
        .maximize()
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    assert_eq!(from_handle, from_cover);

    // Rebuilding from cover() reproduces the same canonical handle.
    let rebuilt = builder.build_cover(&f.cover());
    assert!(rebuilt.equivalent_to(&f));
}

#[test]
fn contradiction_lowers_without_panicking() {
    use crate::BoolExpr;

    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let f = a.clone() & !a; // a & !a — the constant false
    assert!(f.is_contradiction());

    // cover() keeps the arity-1 anonymous output header, so the cover is one output with zero cubes
    // (rather than a re-derived zero-output header that would break to_expr_by_index(0)).
    let cover = f.cover();
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.num_cubes(), 0);

    // to_expr therefore lowers a contradiction to the constant false ("0") with no panic.
    assert_eq!(f.to_expr(), BoolExpr::constant(false));
    assert_eq!(f.to_expr().to_string(), "0");

    // The From<Bdd>/From<BoolExpr>/minimize paths that funnel through cover() also stay sound.
    let from_bdd: Cover<Symbol, crate::Anonymous> = (&f).into();
    assert_eq!(from_bdd.num_outputs(), 1);
    assert!(f.minimize().is_ok());
}

#[test]
fn cover_is_anonymous_single_output_onset() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a & b;
    let cover = f.cover();
    assert_eq!(cover.num_outputs(), 1);
    // Every cube is an ON-set (F) cube.
    assert!(cover.cubes().all(|c| c.cube_type() == CubeType::F));
}

// ---- FR (on+off-set) extraction -------------------------------------------------------------------

/// Collect the input minterms of `cover`'s cubes whose `cube_type()` equals `want`.
fn inputs_of_type(
    cover: &Cover<Symbol, crate::Anonymous>,
    want: CubeType,
) -> BTreeSet<Minterm<Symbol>> {
    cover
        .cubes()
        .filter(|c| c.cube_type() == want)
        .map(|c| c.inputs().clone())
        .collect()
}

#[test]
fn cover_fr_carries_both_sets() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a ^ b;

    let cover = f.cover_fr();
    assert_eq!(cover.cover_type(), CoverType::FR);

    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect()).unwrap();
    // The XOR BDD is a full depth-2 tree, so every raw path is already a full minterm.
    let on = inputs_of_type(&cover, CubeType::F);
    let off = inputs_of_type(&cover, CubeType::R);
    assert_eq!(
        on,
        [
            minterm(&header, &[("a", true), ("b", false)]),
            minterm(&header, &[("a", false), ("b", true)]),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        off,
        [
            minterm(&header, &[("a", false), ("b", false)]),
            minterm(&header, &[("a", true), ("b", true)]),
        ]
        .into_iter()
        .collect()
    );

    // Non-empty, disjoint, and jointly exhaustive over the four minterms (after widening).
    assert!(!on.is_empty() && !off.is_empty());
    assert!(on.is_disjoint(&off));
    let maxed = f.maximize_fr();
    let all: BTreeSet<Minterm<Symbol>> = maxed.cubes().map(|c| c.inputs().clone()).collect();
    let want_all: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("a", false), ("b", false)]),
        minterm(&header, &[("a", false), ("b", true)]),
        minterm(&header, &[("a", true), ("b", false)]),
        minterm(&header, &[("a", true), ("b", true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(all, want_all);
}

#[test]
fn maximize_fr_partitions_minterms() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a ^ b;

    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect()).unwrap();
    let maxed = f.maximize_fr();

    let on = inputs_of_type(&maxed, CubeType::F);
    let off = inputs_of_type(&maxed, CubeType::R);
    let want_on: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("a", true), ("b", false)]),
        minterm(&header, &[("a", false), ("b", true)]),
    ]
    .into_iter()
    .collect();
    let want_off: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("a", false), ("b", false)]),
        minterm(&header, &[("a", true), ("b", true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(on, want_on);
    assert_eq!(off, want_off);

    // The two sides together are exactly the four minterms.
    let union: BTreeSet<Minterm<Symbol>> = on.union(&off).cloned().collect();
    assert_eq!(union.len(), 4);
}

#[test]
fn minimize_fr_returns_fr_cover() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a ^ b;

    let m = f.minimize_fr().expect("XOR minimises without error");
    assert_eq!(m.cover_type(), CoverType::FR);

    // The minimised ON-set, widened to full minterms, reproduces the plain ON-set maximisation.
    let on = inputs_of_type(&m.maximize(), CubeType::F);
    let want: BTreeSet<Minterm<Symbol>> =
        f.maximize().cubes().map(|c| c.inputs().clone()).collect();
    assert_eq!(on, want);
}

#[test]
fn contradiction_and_tautology_fr() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");

    // Contradiction: no ON-set path, so zero F cubes but a non-empty R region.
    let contradiction = a.clone() & !a.clone();
    let cc = contradiction.cover_fr();
    assert_eq!(cc.cover_type(), CoverType::FR);
    assert_eq!(cc.num_outputs(), 1);
    assert_eq!(
        cc.cubes().filter(|c| c.cube_type() == CubeType::F).count(),
        0
    );
    assert!(cc.cubes().any(|c| c.cube_type() == CubeType::R));

    // Tautology: no OFF-set path, so zero R cubes but a non-empty F region.
    let tautology = a.clone() | !a;
    let tc = tautology.cover_fr();
    assert_eq!(tc.cover_type(), CoverType::FR);
    assert_eq!(tc.num_outputs(), 1);
    assert_eq!(
        tc.cubes().filter(|c| c.cube_type() == CubeType::R).count(),
        0
    );
    assert!(tc.cubes().any(|c| c.cube_type() == CubeType::F));
}

#[test]
fn cover_unchanged_after_refactor() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a & b;

    let cover = f.cover();
    assert_eq!(cover.cover_type(), CoverType::F);
    assert_eq!(cover.num_outputs(), 1);
    assert!(cover.cubes().all(|c| c.cube_type() == CubeType::F));
}

#[test]
fn both_context_kinds_agree_fr() {
    // Single-threaded.
    let local: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let la = local.var("a");
    let lb = local.var("b");
    let lf = la ^ lb;
    let local_maxed = lf.maximize_fr();
    let local_on = inputs_of_type(&local_maxed, CubeType::F);
    let local_off = inputs_of_type(&local_maxed, CubeType::R);

    // Thread-safe.
    let sync: BddBuilder<BrandB, SyncCell> = BddBuilder::new();
    let sa = sync.var("a");
    let sb = sync.var("b");
    let sf = sa ^ sb;
    let sync_maxed = sf.maximize_fr();
    let sync_on = inputs_of_type(&sync_maxed, CubeType::F);
    let sync_off = inputs_of_type(&sync_maxed, CubeType::R);

    assert_eq!(local_on, sync_on);
    assert_eq!(local_off, sync_off);
}

#[test]
fn minimize_fr_matches_plain_minimize_on_majority() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let maj = (a.clone() & b.clone()) | (b.clone() & c.clone()) | (a.clone() & c.clone());

    let header = Symbols::new(["a", "b", "c"].iter().map(Symbol::new).collect()).unwrap();
    let m = maj.minimize_fr().expect("majority minimises without error");
    assert_eq!(m.cover_type(), CoverType::FR);

    let maxed = m.maximize();
    let on = inputs_of_type(&maxed, CubeType::F);
    let off = inputs_of_type(&maxed, CubeType::R);

    // ON-set: the four minterms with at least two of a, b, c set.
    let want_on: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("a", false), ("b", true), ("c", true)]),
        minterm(&header, &[("a", true), ("b", false), ("c", true)]),
        minterm(&header, &[("a", true), ("b", true), ("c", false)]),
        minterm(&header, &[("a", true), ("b", true), ("c", true)]),
    ]
    .into_iter()
    .collect();
    // OFF-set: the complementary four minterms.
    let want_off: BTreeSet<Minterm<Symbol>> = [
        minterm(&header, &[("a", false), ("b", false), ("c", false)]),
        minterm(&header, &[("a", false), ("b", false), ("c", true)]),
        minterm(&header, &[("a", false), ("b", true), ("c", false)]),
        minterm(&header, &[("a", true), ("b", false), ("c", false)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(on, want_on);
    assert_eq!(off, want_off);

    // Supplying the exact off-set does not change the minimised ON-set: it matches plain `minimize`.
    let plain_on: BTreeSet<Minterm<Symbol>> = maj
        .minimize()
        .expect("majority minimises without error")
        .maximize()
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    assert_eq!(on, plain_on);
}

#[test]
fn minimize_fr_on_constants() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");

    // Contradiction: empty ON-set, full OFF-set — minimises without error, no F cubes.
    let contradiction = a.clone() & !a.clone();
    let cm = contradiction
        .minimize_fr()
        .expect("contradiction minimises without error");
    assert_eq!(cm.cover_type(), CoverType::FR);
    assert_eq!(cm.num_outputs(), 1);
    assert_eq!(
        cm.cubes().filter(|c| c.cube_type() == CubeType::F).count(),
        0
    );

    // Tautology: full ON-set, empty OFF-set — minimises without error, no R cubes.
    let tautology = a.clone() | !a;
    let tm = tautology
        .minimize_fr()
        .expect("tautology minimises without error");
    assert_eq!(tm.cover_type(), CoverType::FR);
    assert_eq!(tm.num_outputs(), 1);
    assert_eq!(
        tm.cubes().filter(|c| c.cube_type() == CubeType::R).count(),
        0
    );
}

#[test]
fn cover_over_fr_subset_opens_undef_gap() {
    // Projecting onto a strict subset of the support is universal, not existential: f = a & b;
    // projecting onto {a} (eliminating b) gives on = ∀b.(a & b) = ∅ and off = ∀b.!(a & b) = !a =
    // {a:0}. The assignment a=1 is genuinely undecided (f depends on b there) and lands in neither
    // side — a real don't-care gap, not an overlap.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a & b;

    let header = Symbols::new(["a"].iter().map(Symbol::new).collect()).unwrap();
    let cover = f.cover_over_fr(["a"]);
    let m = cover.maximize();
    let on = inputs_of_type(&m, CubeType::F);
    let off = inputs_of_type(&m, CubeType::R);
    assert!(on.is_empty());
    assert_eq!(
        off,
        [minterm(&header, &[("a", false)])].into_iter().collect()
    );
    let a_true = minterm(&header, &[("a", true)]);
    assert!(!on.contains(&a_true) && !off.contains(&a_true));
    assert!(on.is_disjoint(&off));
}

// ---- over_vars / primes (universal projection, all-primes) ----------------------------------------

#[test]
fn cover_over_fr_c_element_gap() {
    // Muller C-element: q_next = (a & b) | (q & a) | (q & b). Projecting away q onto {a, b} keeps
    // the consensus prime a & b, so on = {a:1,b:1} and off = {a:0,b:0}; the disagreeing assignments
    // a≠b are genuinely undecided and land in neither side.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let q = builder.var("q");
    let q_next = (a.clone() & b.clone()) | (q.clone() & a.clone()) | (q & b.clone());

    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect()).unwrap();
    let m = q_next.cover_over_fr(["a", "b"]).maximize();
    assert_eq!(m.cover_type(), CoverType::FR);

    let on = inputs_of_type(&m, CubeType::F);
    let off = inputs_of_type(&m, CubeType::R);
    assert_eq!(
        on,
        [minterm(&header, &[("a", true), ("b", true)])]
            .into_iter()
            .collect()
    );
    assert_eq!(
        off,
        [minterm(&header, &[("a", false), ("b", false)])]
            .into_iter()
            .collect()
    );
    let a1b0 = minterm(&header, &[("a", true), ("b", false)]);
    let a0b1 = minterm(&header, &[("a", false), ("b", true)]);
    assert!(!on.contains(&a1b0) && !off.contains(&a1b0));
    assert!(!on.contains(&a0b1) && !off.contains(&a0b1));
}

#[test]
fn cover_over_keeps_partial_support_prime() {
    // f = a | (b & c). ∀c.f = a, so projecting onto {a, b} (eliminating c) widens the surviving
    // prime `a` over b: {a:1,b:0}, {a:1,b:1}.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = a.clone() | (b.clone() & c);

    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect()).unwrap();
    let got: BTreeSet<Minterm<Symbol>> = f
        .cover_over(["a", "b"])
        .maximize()
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
fn cover_over_survives_irredundant_prime_trap() {
    // f = (a & x) | (b & !x) | (a & b). The irredundant cover espresso's minimiser would return is
    // {a & x, b & !x} — the consensus prime a & b is redundant there and gets dropped. But
    // `primes_consensus` (which `cover_over`/`over_vars` filter on) returns the COMPLETE prime set,
    // so a & b survives and is exactly what ∀x.f projects to onto {a, b}.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let x = builder.var("x");
    let f = (a.clone() & x.clone()) | (b.clone() & !x) | (a & b);

    let header = Symbols::new(["a", "b"].iter().map(Symbol::new).collect()).unwrap();
    let got: BTreeSet<Minterm<Symbol>> = f
        .cover_over(["a", "b"])
        .maximize()
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    let want: BTreeSet<Minterm<Symbol>> = [minterm(&header, &[("a", true), ("b", true)])]
        .into_iter()
        .collect();
    assert_eq!(got, want);
}

#[test]
fn cover_over_agrees_with_bdd_forall() {
    // Oracle check: universal projection via `cover_over` must agree with explicit BDD-level
    // quantification (`forall`) followed by the same widening.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let x = builder.var("x");
    let f = (a.clone() & x.clone()) | (b.clone() & !x.clone()) | (a & b);

    let via_project: BTreeSet<Minterm<Symbol>> = f
        .cover_over(["a", "b"])
        .maximize()
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    let via_forall: BTreeSet<Minterm<Symbol>> = f
        .forall(["x"])
        .cover_over(["a", "b"])
        .maximize()
        .cubes()
        .map(|c| c.inputs().clone())
        .collect();
    assert_eq!(via_project, via_forall);
}

#[test]
fn bdd_primes_contains_consensus_prime() {
    // f = (a & x) | (b & !x) | (a & b). `primes()` returns the COMPLETE prime set, including the
    // consensus prime a & b (x don't-care); `minimize()` returns an irredundant cover, which drops
    // it since {a & x, b & !x} already covers the on-set.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let x = builder.var("x");
    let f = (a.clone() & x.clone()) | (b.clone() & !x) | (a & b);

    let primes = f.primes();
    let is_consensus_prime = |cube: &Cube<Symbol, crate::Anonymous>| {
        cube.inputs().value_of("a") == Some(true)
            && cube.inputs().value_of("b") == Some(true)
            && cube.inputs().value_of("x").is_none()
    };
    assert!(primes.cubes().any(is_consensus_prime));

    let minimized = f.minimize().expect("f minimises without error");
    assert!(!minimized.cubes().any(is_consensus_prime));
}

#[test]
#[allow(deprecated)]
fn to_cubes_is_deprecated_alias_of_cover() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a & b;
    assert_eq!(f.to_cubes(), f.cover());
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
    let local_minterms: BTreeSet<Minterm<Symbol>> =
        lf.maximize().cubes().map(|c| c.inputs().clone()).collect();
    let local_taut = (la.clone() | !la).is_tautology();

    // Thread-safe.
    let sync: BddBuilder<BrandB, SyncCell> = BddBuilder::new();
    let sa = sync.var("a");
    let sb = sync.var("b");
    let sc = sync.var("c");
    let sf = (sa.clone() & sb) | (sa.clone() ^ sc);
    let sync_minterms: BTreeSet<Minterm<Symbol>> =
        sf.maximize().cubes().map(|c| c.inputs().clone()).collect();
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
        f.maximize().cubes().count()
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

// ---- Composition (compose / compose_map) ----------------------------------------------------------

/// The `f = (a & b) | (!a & c)` fixture and its five-candidate `g` battery — `d ^ a`, `!b`, the two
/// constants, and `c | (a & e)` — shared by the compose/compose_map oracle tests below.
fn compose_battery(
    builder: &BddBuilder<BrandA, LocalCell>,
) -> (
    super::Bdd<BrandA, LocalCell>,
    Vec<super::Bdd<BrandA, LocalCell>>,
) {
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let d = builder.var("d");
    let e = builder.var("e");
    let f = (a.clone() & b.clone()) | (!a.clone() & c.clone());
    let g_candidates = vec![
        d ^ a.clone(),
        !b,
        builder.constant(true),
        builder.constant(false),
        c | (a & e),
    ];
    (f, g_candidates)
}

#[test]
fn compose_identity_projection() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = (a.clone() & b.clone()) | !a.clone();

    // Substituting a variable with itself is the identity: the same canonical root.
    assert_eq!(f.compose("a", &a), f);
}

#[test]
fn compose_absent_var_is_noop() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let d = builder.var("d"); // present in the manager, but outside f's support
    let f = a.clone() & b.clone();

    // A name never created in this manager is absent from every function — a no-op.
    assert_eq!(f.compose("zzz", &d), f);
    // A name that exists in the manager's ordering but is not in f's support is likewise a no-op.
    assert_eq!(f.compose("d", &d), f);
}

#[test]
fn compose_matches_naive_ite_oracle() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let (f, g_candidates) = compose_battery(&builder);

    // Oracle: compose(var, g) must equal g.ite(f|var=1, f|var=0) — canonicity makes this a root
    // equality, not merely an equivalence.
    for v in ["a", "b", "c"] {
        for g in &g_candidates {
            let composed = f.compose(v, g);
            let oracle = g.ite(&f.restrict(v, true), &f.restrict(v, false));
            assert_eq!(composed, oracle);
        }
    }

    // One exhaustive semantic check over the union support {a, b, c, d, e}: compose(f, v, g)(σ) must
    // equal f(σ[v := g(σ)]) for every assignment.
    let v = "a";
    let g = &g_candidates[4]; // c | (a & e)
    let composed = f.compose(v, g);
    let names = ["a", "b", "c", "d", "e"];
    for mask in 0..(1u32 << names.len()) {
        let vals: Vec<(&str, bool)> = names
            .iter()
            .enumerate()
            .map(|(i, &n)| (n, (mask >> i) & 1 == 1))
            .collect();
        let sigma = assign(&vals);
        let g_val = g.evaluate(&sigma).unwrap();

        let mut shifted = vals.clone();
        shifted.iter_mut().find(|(n, _)| *n == v).unwrap().1 = g_val;

        assert_eq!(
            composed.evaluate(&sigma).unwrap(),
            f.evaluate(&assign(&shifted)).unwrap()
        );
    }
}

#[test]
fn compose_g_above_substituted_var() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    // Mint order a, b, c: a sits above the substituted variable c in the diagram.
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = b.clone() & c.clone();

    // g = a is entirely above var in the order — the substitution must still splice correctly.
    assert_eq!(f.compose("c", &a), a.clone() & b.clone());

    // g spans both sides of var: a is above c, d (minted after) is below it.
    let d = builder.var("d");
    let g = a.clone() ^ d;
    let composed = f.compose("c", &g);
    let oracle = g.ite(&f.restrict("c", true), &f.restrict("c", false));
    assert_eq!(composed, oracle);
}

#[test]
fn compose_g_may_test_var_itself() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a.clone() & b.clone();

    // g = !b tests the very variable being substituted; the b inside g stays free rather than being
    // grounded by the substitution point.
    let g = !b.clone();
    assert_eq!(f.compose("b", &g), a & !b);
}

#[test]
fn compose_map_is_simultaneous() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f = a.clone() & !b.clone();

    let swapped = f.compose_map([("a", &b), ("b", &a)]);
    assert_eq!(swapped, b.clone() & !a.clone());

    // A sequential chain substitutes a := b first, then b := a on the already-substituted result — the
    // second substitution catches the b just introduced, collapsing to a contradiction.
    let sequential = f.compose("a", &b).compose("b", &a);
    assert!(sequential.is_contradiction());
    assert_ne!(swapped, sequential);
}

#[test]
fn compose_map_hoisted_substitution_canonical() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    // Mint order a, b, c: a sits above the substituted variable c.
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = b.clone() & c.clone();

    let via_map = f.compose_map([("c", &a)]);
    assert_eq!(via_map, a.clone() & b.clone());
    // Agrees with single-variable `compose` — the unmapped `b` takes the fallback (unsubstituted) path.
    assert_eq!(via_map, f.compose("c", &a));
}

#[test]
fn compose_map_empty_and_absent() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = a.clone() & b.clone();

    // An empty map is a no-op.
    let empty: Vec<(&str, &super::Bdd<BrandA, LocalCell>)> = Vec::new();
    assert_eq!(f.compose_map(empty), f);

    // A map naming only absent variables is also a no-op.
    assert_eq!(f.compose_map([("zzz", &c), ("yyy", &c)]), f);

    // A repeated name takes its last entry.
    assert_eq!(f.compose_map([("a", &b), ("a", &c)]), f.compose("a", &c));
}

#[test]
fn compose_singleton_map_agrees_with_compose() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let (f, g_candidates) = compose_battery(&builder);

    for v in ["a", "b", "c"] {
        for g in &g_candidates {
            assert_eq!(f.compose_map([(v, g)]), f.compose(v, g));
        }
    }
}

#[test]
fn compose_deep_chain_no_overflow() {
    // Mirrors `forall_over_deep_chain_no_overflow`: composing the *bottom* variable of a deep AND chain
    // must walk the whole chain without overflowing the call stack.
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let n = 2000usize;
    let names: Vec<String> = (0..n).map(|i| format!("v{i}")).collect();
    let mut f = builder.var(&names[0]);
    for name in &names[1..] {
        f = f & builder.var(name);
    }
    let bottom = names[n - 1].as_str();
    let ff = builder.constant(false);

    // Substituting the bottom variable with the constant false collapses the whole conjunction.
    assert!(f.compose(bottom, &ff).is_contradiction());
    assert!(f.compose_map([(bottom, &ff)]).is_contradiction());
}

#[test]
fn compose_twice_yields_same_root() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = a.clone() & b.clone();

    // The same substitution computed twice reaches the same canonical root.
    let once = f.compose("a", &c);
    let twice = f.compose("a", &c);
    assert_eq!(once, twice);
}

#[test]
fn scoped_compose_agrees_with_owned() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();

    // f = a & b, then substitute b := c, composed entirely inside one scope.
    let scoped = builder.scope(|s| s.var("a").and(s.var("b")).compose("b", s.var("c")));
    let a = builder.var("a");
    let c = builder.var("c");
    assert!(scoped.equivalent_to(&(a.clone() & c.clone())));

    // The `compose_map` simultaneous swap of `compose_map_is_simultaneous`, composed inside a scope.
    let swapped_scoped = builder.scope(|s| {
        let sa = s.var("a");
        let sb = s.var("b");
        (sa & !sb).compose_map([("a", sb), ("b", sa)])
    });
    let b = builder.var("b");
    assert!(swapped_scoped.equivalent_to(&(b & !a)));
}

#[test]
fn batch_compose_map_matches_single() {
    use crate::bdd::Composer;
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f1 = &a & &b;
    let f2 = &a ^ &c;

    // One substitution across two functions in a single shared-cache pass.
    let batched: Vec<_> = vec![f1.clone(), f2.clone()]
        .compose_map([("a", c.clone())])
        .collect();

    assert!(batched[0].equivalent_to(&f1.compose_map([("a", &c)])));
    assert!(batched[1].equivalent_to(&f2.compose_map([("a", &c)])));
}

#[test]
fn batch_compose_preserves_order() {
    use crate::bdd::Composer;
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f1 = &a & &b;
    let f2 = &a | &b;
    let f3 = !&a;

    let out: Vec<_> = vec![f1.clone(), f2.clone(), f3.clone()]
        .compose("a", c.clone())
        .collect();
    assert_eq!(out.len(), 3);
    assert!(out[0].equivalent_to(&f1.compose("a", &c)));
    assert!(out[1].equivalent_to(&f2.compose("a", &c)));
    assert!(out[2].equivalent_to(&f3.compose("a", &c)));
}

#[test]
fn batch_compose_single_var_matches_compose_map() {
    use crate::bdd::Composer;
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f1 = &a & &b;
    let f2 = &a ^ &c;

    // `.compose("a", g)` is exactly `.compose_map([("a", g)])` over the same stream.
    let via_compose: Vec<_> = vec![f1.clone(), f2.clone()]
        .compose("a", c.clone())
        .collect();
    let via_map: Vec<_> = vec![f1.clone(), f2.clone()]
        .compose_map([("a", c.clone())])
        .collect();
    assert_eq!(via_compose[0], via_map[0]);
    assert_eq!(via_compose[1], via_map[1]);
}

#[test]
fn batch_compose_identity_leaves_functions_unchanged() {
    use crate::bdd::Composer;
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f1 = &a & &b;
    let f2 = &a | &c;

    // A name no function tests resolves to an empty substitution: each function is returned as-is.
    let unknown: Vec<_> = vec![f1.clone(), f2.clone()]
        .compose_map([("zzz", c.clone())])
        .collect();
    assert!(unknown[0].equivalent_to(&f1));
    assert!(unknown[1].equivalent_to(&f2));

    // A genuinely empty substitution (no manager to seed from up front) is likewise identity.
    let empty: Vec<_> = vec![f1.clone(), f2.clone()]
        .compose_map(Vec::<(&str, crate::bdd::Bdd<BrandA, LocalCell>)>::new())
        .collect();
    assert!(empty[0].equivalent_to(&f1));
    assert!(empty[1].equivalent_to(&f2));
}

#[test]
fn batch_compose_repeat_reuses_nodes() {
    use crate::bdd::{Composer, ManagerCell};
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = &a & &b;

    // Warm up so every node the composition needs is already interned.
    let _ = vec![f.clone()].compose("b", c.clone()).collect::<Vec<_>>();
    let nodes_before = builder.cell().read().nodes.len();

    // Composing the same function twice in one batch: the second pull hits the shared memo and the
    // first re-derives already-interned nodes — no new nodes either way.
    let out: Vec<_> = vec![f.clone(), f.clone()].compose("b", c.clone()).collect();
    assert_eq!(out[0], out[1]);
    assert_eq!(builder.cell().read().nodes.len(), nodes_before);
}

#[test]
fn batch_compose_scoped_matches_owned() {
    use crate::bdd::Composer;
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f1 = &a & &b;
    let f2 = &a ^ &c;
    let e1 = f1.compose_map([("a", &c)]);
    let e2 = f2.compose_map([("a", &c)]);

    // The scoped batch borrows the scope lifetime, so collect and compare inside the closure.
    let _ = builder.scope(|s| {
        let sf1 = s.lift(&f1);
        let sf2 = s.lift(&f2);
        let sc = s.lift(&c);
        let out: Vec<_> = vec![sf1, sf2].compose_map([("a", sc)]).collect();
        // Canonical roots on the shared manager: equal iff the functions are equivalent.
        assert_eq!(out[0].root(), e1.root());
        assert_eq!(out[1].root(), e2.root());
        sf1
    });
}

#[test]
#[should_panic(expected = "different manager")]
fn batch_compose_cross_manager_panics() {
    use crate::bdd::Composer;
    let one: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let two: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let f_one = one.var("a") & one.var("b");
    let g_one = one.var("c");
    let f_two = two.var("a");

    // The substitution and the first function share `one`'s manager; the second function is from
    // `two`. Mixing managers in one batch is a bug and must panic.
    let _: Vec<_> = vec![f_one, f_two].compose("a", g_one).collect();
}

// The batch path resolves *every* substitute against the one manager held from the first entry, so a
// foreign substitute in a multi-entry `compose_map` reads roots in the wrong NodeId space. This is a
// distinct gap from `batch_compose_cross_manager_panics` (a foreign *stream* function through single-var
// `.compose`): here the stream and first substitute are consistent, and it is the second *substitute* that
// clashes. The batch check fires with its own message, worded differently from `assert_same_manager`.
#[test]
#[should_panic(expected = "batch compose: a function came from a different manager")]
fn batch_compose_map_foreign_substitute_panics() {
    use crate::bdd::Composer;
    let make = || crate::bdd_builder!();
    let one = make();
    let two = make();
    // One call site, so `one` and `two` share a brand type but own different managers: this type-checks.
    let f = one.var("a") & one.var("b");
    let g1 = one.var("c"); // seeds the held manager
    let g2 = two.var("c"); // foreign substitute in the same map
    let _: Vec<_> = vec![f].compose_map([("a", g1), ("b", g2)]).collect();
}

#[test]
fn batch_compose_on_sync_cell_matches_single() {
    use crate::bdd::Composer;
    let sync: BddBuilder<BrandB, SyncCell> = BddBuilder::new();
    let a = sync.var("a");
    let b = sync.var("b");
    let c = sync.var("c");
    let f1 = &a & &b;
    let f2 = &a ^ &c;

    // The batch path over a SyncCell-backed builder (the other batch tests are LocalCell-only), matched
    // against the equivalent single-function compose.
    let batched: Vec<_> = vec![f1.clone(), f2.clone()]
        .compose_map([("a", c.clone())])
        .collect();
    assert!(batched[0].equivalent_to(&f1.compose_map([("a", &c)])));
    assert!(batched[1].equivalent_to(&f2.compose_map([("a", &c)])));

    // And the single-variable `compose` shorthand.
    let via_compose: Vec<_> = vec![f1.clone(), f2.clone()]
        .compose("a", c.clone())
        .collect();
    assert!(via_compose[0].equivalent_to(&f1.compose("a", &c)));
    assert!(via_compose[1].equivalent_to(&f2.compose("a", &c)));
}

#[test]
fn batch_compose_empty_stream_yields_empty() {
    use crate::bdd::Composer;
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let c = builder.var("c");
    let empty: Vec<crate::bdd::Bdd<BrandA, LocalCell>> = Vec::new();

    // No functions to compose: the iterator is empty, reports length 0 up front (ExactSizeIterator), and
    // stays exhausted across repeated pulls (FusedIterator).
    let mut single = empty.clone().into_iter().compose("a", c.clone());
    assert_eq!(single.len(), 0);
    assert!(single.next().is_none());
    assert!(single.next().is_none());

    // Same over `compose_map`, with a non-empty substitution …
    let mut mapped = empty.clone().into_iter().compose_map([("a", c.clone())]);
    assert_eq!(mapped.len(), 0);
    assert!(mapped.next().is_none());

    // … and with an empty substitution (the identity path never even seeds a manager).
    let no_sub = Vec::<(&str, crate::bdd::Bdd<BrandA, LocalCell>)>::new();
    let mut both_empty = empty.into_iter().compose_map(no_sub);
    assert_eq!(both_empty.len(), 0);
    assert!(both_empty.next().is_none());
}

#[test]
fn batch_compose_empty_substitution_returns_same_handle() {
    use crate::bdd::Composer;
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let f1 = &a & &b;
    let f2 = &a ^ &b;

    // An empty substitution short-circuits (bug ④): each function is yielded unchanged rather than
    // re-walked, so the result is the *same* canonical root, not merely an equivalent rebuild.
    let no_sub = Vec::<(&str, crate::bdd::Bdd<BrandA, LocalCell>)>::new();
    let out: Vec<_> = vec![f1.clone(), f2.clone()].compose_map(no_sub).collect();
    assert_eq!(out[0].root(), f1.root());
    assert_eq!(out[1].root(), f2.root());
}

#[test]
fn compose_on_sync_cell_agrees() {
    // One combo from the §6.3 battery: f = (a & b) | (!a & c), compose("a", d ^ a).
    let local: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let local_combo: BTreeSet<Minterm<Symbol>> = {
        let a = local.var("a");
        let b = local.var("b");
        let c = local.var("c");
        let d = local.var("d");
        let f = (a.clone() & b.clone()) | (!a.clone() & c.clone());
        let g = d ^ a;
        f.compose("a", &g)
            .maximize()
            .cubes()
            .map(|cube| cube.inputs().clone())
            .collect()
    };
    let sync: BddBuilder<BrandB, SyncCell> = BddBuilder::new();
    let sync_combo: BTreeSet<Minterm<Symbol>> = {
        let a = sync.var("a");
        let b = sync.var("b");
        let c = sync.var("c");
        let d = sync.var("d");
        let f = (a.clone() & b.clone()) | (!a.clone() & c.clone());
        let g = d ^ a;
        f.compose("a", &g)
            .maximize()
            .cubes()
            .map(|cube| cube.inputs().clone())
            .collect()
    };
    assert_eq!(local_combo, sync_combo);

    // `compose_map_is_simultaneous`'s swap.
    let local_swapped: BTreeSet<Minterm<Symbol>> = {
        let a = local.var("a");
        let b = local.var("b");
        let f = a.clone() & !b.clone();
        f.compose_map([("a", &b), ("b", &a)])
            .maximize()
            .cubes()
            .map(|cube| cube.inputs().clone())
            .collect()
    };
    let sync_swapped: BTreeSet<Minterm<Symbol>> = {
        let a = sync.var("a");
        let b = sync.var("b");
        let f = a.clone() & !b.clone();
        f.compose_map([("a", &b), ("b", &a)])
            .maximize()
            .cubes()
            .map(|cube| cube.inputs().clone())
            .collect()
    };
    assert_eq!(local_swapped, sync_swapped);

    // `compose_map_hoisted_substitution_canonical`'s hoist: e is minted after a, so a sits above the
    // substituted variable e.
    let local_hoisted: BTreeSet<Minterm<Symbol>> = {
        let a = local.var("a");
        let b = local.var("b");
        let e = local.var("e");
        let f = b.clone() & e.clone();
        f.compose_map([("e", &a)])
            .maximize()
            .cubes()
            .map(|cube| cube.inputs().clone())
            .collect()
    };
    let sync_hoisted: BTreeSet<Minterm<Symbol>> = {
        let a = sync.var("a");
        let b = sync.var("b");
        let e = sync.var("e");
        let f = b.clone() & e.clone();
        f.compose_map([("e", &a)])
            .maximize()
            .cubes()
            .map(|cube| cube.inputs().clone())
            .collect()
    };
    assert_eq!(local_hoisted, sync_hoisted);
}

#[test]
fn scoped_restrict_agrees_with_owned() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();

    // f = a & b, restricted b := true, entirely inside one scope.
    let scoped = builder.scope(|s| (s.var("a") & s.var("b")).restrict("b", true));
    let a = builder.var("a");
    assert!(scoped.equivalent_to(&a));

    // A scoped restrict of an absent name is a no-op.
    let scoped_noop = builder.scope(|s| s.var("a").restrict("zzz", true));
    assert!(scoped_noop.equivalent_to(&a));
}

#[test]
fn scoped_restrict_many_agrees_with_owned() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = (a.clone() & b.clone()) | (!a.clone() & c.clone());

    let scoped = builder.scope(|s| s.lift(&f).restrict_many([("a", true), ("c", false)]));
    assert!(scoped.equivalent_to(&f.restrict_many([("a", true), ("c", false)])));

    // A repeated name inside the scoped call takes its last entry, same as the owned call.
    let scoped_repeated = builder.scope(|s| s.lift(&f).restrict_many([("a", true), ("a", false)]));
    assert!(scoped_repeated.equivalent_to(&f.restrict_many([("a", true), ("a", false)])));
}

/// The scoped path shares `encoding::restrict_many`, so it has the same re-entrancy exposure: a lazy
/// adaptor minting a fresh scoped variable mid-iteration must not reborrow the manager guard. Companion to
/// `restrict_many_lazy_reentrant_iterator_local_cell` on the owned handle.
#[test]
fn scoped_restrict_many_lazy_reentrant_iterator() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = (a.clone() & b.clone()) | (!a.clone() & c.clone());

    let scoped = builder.scope(|s| {
        s.lift(&f).restrict_many(
            [("a", true), ("c", false)]
                .into_iter()
                .map(|(name, value)| {
                    let _ = s.var("scratch");
                    (name, value)
                }),
        )
    });
    assert!(scoped.equivalent_to(&f.restrict_many([("a", true), ("c", false)])));
}

#[test]
fn scoped_restrict_to_agrees_with_owned() {
    let builder: BddBuilder<BrandA, LocalCell> = BddBuilder::new();
    let a = builder.var("a");
    let b = builder.var("b");
    let c = builder.var("c");
    let f = (a.clone() & b.clone()) | (!a.clone() & c.clone());

    let m = Minterm::<Symbol>::with_labels(&[("a", Some(true)), ("c", Some(false))]).unwrap();
    let scoped = builder.scope(|s| s.lift(&f).restrict_to(&m));
    assert!(scoped.equivalent_to(&f.restrict_to(&m)));
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

#[test]
#[should_panic(expected = "different managers")]
fn compose_across_clashing_brands_panics() {
    let make = || crate::bdd_builder!();
    let one = make();
    let two = make();
    // Same brand clash as the other tests in this section, but through `compose`'s own
    // `assert_same_manager` check on `g`.
    let _ = one.var("x").compose("x", &two.var("y"));
}

// ---- Generic label parameter S (non-Symbol interop) -------------------------------------------------
//
// The manager stays Symbol-keyed; `S` is a phantom marker on `Bdd`/`BddBuilder`/`Scope`, realised only
// at output boundaries via `S::from`. `bdd_builder!()`/`sync_bdd_builder!()` always mint the `Symbol`
// default; `BddBuilder::relabel` is how a non-`Symbol` builder is minted. These tests exercise a
// `String`-labelled builder end to end, mirroring the Symbol-default coverage above.

#[test]
fn relabelled_builder_var_and_parse_agree() {
    let b = crate::bdd_builder!().relabel::<String>();
    let f = b.var("a") & b.var("b");
    let parsed = b.parse("a & b").unwrap();
    assert!(f.equivalent_to(&parsed));
}

#[test]
fn relabelled_builder_variables_yields_stored_type() {
    let b = crate::bdd_builder!().relabel::<String>();
    let f = b.parse("a & b").unwrap();
    let mut vars: Vec<String> = f.variables().collect();
    vars.sort();
    assert_eq!(vars, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn relabelled_builder_cover_is_string_labelled_and_sorted() {
    let b = crate::bdd_builder!().relabel::<String>();
    let f = b.parse("c & a & b").unwrap();
    let cover: Cover<String, crate::Anonymous> = f.cover();
    assert_eq!(
        cover.input_labels(),
        &["a".to_string(), "b".to_string(), "c".to_string()]
    );
}

#[test]
fn relabelled_builder_minimize_returns_string_labelled_cover() {
    let b = crate::bdd_builder!().relabel::<String>();
    // (a & b) | (a & !b) reduces to a.
    let f = b.parse("(a & b) | (a & !b)").unwrap();
    let minimized: Cover<String, crate::Anonymous> = f.minimize().unwrap();
    assert_eq!(minimized.input_labels(), &["a".to_string()]);
}

#[test]
fn relabelled_builder_to_expr_round_trips_through_build() {
    let b = crate::bdd_builder!().relabel::<String>();
    let original = b.parse("a & (b | !c)").unwrap();
    let expr: crate::BoolExpr<String> = original.to_expr();
    // build accepts L = String directly, matching the builder's own stored label type.
    let rebuilt = b.build(&expr);
    assert!(rebuilt.equivalent_to(&original));
}

#[test]
fn relabel_round_trip_preserves_root() {
    let builder = crate::bdd_builder!();
    let f = builder.parse("a & b").unwrap();
    let round_tripped = f.relabel::<String>().relabel::<Symbol>();
    assert_eq!(round_tripped, f);
}

#[test]
fn string_scope_lifts_symbol_handle() {
    let symbol_builder = crate::bdd_builder!();
    let owned = symbol_builder.var("a");
    let string_builder = symbol_builder.relabel::<String>();
    // The scope's own stored label (String) is independent of the lifted handle's (Symbol): `lift` is
    // generic over the source handle's S2.
    let f = string_builder.scope(|s| s.lift(&owned) & s.var("b"));
    let expected = &owned & &symbol_builder.var("b");
    assert!(f.relabel::<Symbol>().equivalent_to(&expected));
}
