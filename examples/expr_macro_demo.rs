//! Building `BoolExpr`s with `BoolExpr::build` and the `expr!` macro.
//!
//! Run with: `cargo run --example expr_macro_demo`

use espresso_logic::{bdd_builder, expr, BoolExpr};

fn main() {
    // `BoolExpr::build` hands a closure an auxiliary builder. Its handles are `Copy` and implement the
    // operators, so a large expression is composed without `&` or `.clone()` and allocates a single
    // token stream at the end.
    let parity: BoolExpr = BoolExpr::build(|b| {
        let a = b.var("a");
        let c = b.var("c");
        (a ^ b.var("b")) ^ c
    });
    println!("built:   {parity}");

    // `expr!` is the same composition in infix syntax. Identifiers splice existing expressions in;
    // string literals are fresh variables; `0`/`1` are constants.
    let a: BoolExpr = BoolExpr::var("a");
    let b = BoolExpr::var("b");
    let selected = expr!(a & !b | "c" & 1);
    println!("expr!:   {selected}");

    // A `BoolExpr` is syntactic; build it into a `Bdd` to minimise or compare it.
    let builder = bdd_builder!();
    let minimised = builder.build(&selected).minimize().unwrap();
    println!("minimised cover:\n{minimised}");
}
