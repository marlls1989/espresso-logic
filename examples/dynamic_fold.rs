//! Data-driven construction: `BoolExpr::build` (syntactic) vs a BDD-builder fold (canonical).
//!
//! `expr!` is fixed infix syntax, so it cannot fold a *runtime* set of variables. The closure
//! builders can: this example folds a slice of variable names into a conjunction two ways and
//! compares the results.
//!
//! Run with: `cargo run --example dynamic_fold`

use espresso_logic::{bdd_builder, BddBuilder, BoolExpr, LocalCell};

fn main() {
    // The variables are only known at runtime — here a slice, but it could be any iterator.
    let names = ["a", "b", "c", "d"];

    // 1. BoolExpr::build — a single-pass *syntactic* expression. The handles are Copy and compose
    //    with the operators, so the fold reads like ordinary Rust and serialises one token stream.
    let conj_expr: BoolExpr = BoolExpr::build(|b| {
        names
            .iter()
            .map(|n| b.var(*n))
            .reduce(|x, y| x & y)
            .unwrap()
    });
    println!("BoolExpr::build : {conj_expr}"); // a & b & c & d — the syntax, exactly as folded

    // 2. BDD-builder fold — the canonical *function*. Handles are Clone and canonicalise as they
    //    combine, so shared structure is merged and equivalence is O(1).
    let builder: BddBuilder<_, LocalCell> = bdd_builder!();
    let conj_bdd = names
        .iter()
        .map(|n| builder.var(*n))
        .reduce(|x, y| x & y)
        .unwrap();

    // Canonical: folding in the opposite order yields the same function.
    let reversed = names
        .iter()
        .rev()
        .map(|n| builder.var(*n))
        .reduce(|x, y| x & y)
        .unwrap();
    println!(
        "BDD fold       : canonical — a&b&c&d equals d&c&b&a is {}",
        conj_bdd.equivalent_to(&reversed)
    );

    // 3. The same canonical function through a scope. A `ScopedBdd` is Copy, so the fold composes the
    //    handles in place — no refcount bump per `&` — and one owned `Bdd` is materialised at the end.
    let conj_scoped = builder.scope(|s| {
        names
            .iter()
            .map(|n| s.var(*n))
            .reduce(|x, y| x & y)
            .unwrap()
    });
    println!(
        "scope fold     : matches the owned fold: {}",
        conj_scoped.equivalent_to(&conj_bdd)
    );

    // The two layers answer different questions:
    //  - keep `conj_expr` to display, persist, or minimise the expression as written;
    //  - keep `conj_bdd` to ask about the function (equivalence, evaluation, cofactors).
    // A `BoolExpr` can always be built into a builder when the function is needed later:
    let built = builder.build(&conj_expr);
    println!(
        "expr built into the same builder matches the fold: {}",
        built.equivalent_to(&conj_bdd)
    );
}
