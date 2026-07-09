//! C-element: two equivalent syntactic formulations of `next_q`, compared as **syntactic `BoolExpr`**
//! (naive sum-of-products, no canonicalisation) against the **canonical `Bdd`**.
//!
//! Run with: `cargo run --example c_element_example`

use espresso_logic::{bdd_builder, expr, BoolExpr, Cover, CoverType, ExprNode, Minimizable};
use std::collections::BTreeMap;
use std::sync::Arc;

/// One product term: each variable mapped to the polarity it must take.
type Cube = BTreeMap<Arc<str>, bool>;

/// AND two DNF cube sets (cross product), dropping contradictory merges.
fn cross(left: &[Cube], right: &[Cube]) -> Vec<Cube> {
    let mut result = Vec::new();
    for l in left {
        for r in right {
            if let Some(merged) = merge_cubes(l, r) {
                result.push(merged);
            }
        }
    }
    result
}

/// OR two DNF cube sets (concatenation — no deduplication, so the naive blow-up is preserved).
fn union(mut left: Vec<Cube>, right: Vec<Cube>) -> Vec<Cube> {
    left.extend(right);
    left
}

/// Merge (AND) two cubes, or `None` if they disagree on some variable's polarity.
fn merge_cubes(left: &Cube, right: &Cube) -> Option<Cube> {
    let mut result = left.clone();
    for (var, &polarity) in right {
        if let Some(&existing) = result.get(var) {
            if existing != polarity {
                return None; // contradiction — the product is empty
            }
        } else {
            result.insert(Arc::clone(var), polarity);
        }
    }
    Some(result)
}

/// Naive sum-of-products cube count of a syntactic [`BoolExpr`] — the structure exactly as written, with
/// no canonicalisation or sharing.
///
/// Two structural passes, both via the public fold API: first every `^` is rewritten into `&`/`|`/`!`
/// (a naive DNF has no XOR), then a top-down [`BoolExpr::fold_with_context`] pushes negation toward the
/// leaves (De Morgan) and crosses/unions the per-node cube sets. Computing only the positive DNF (never a
/// node's unused negation) keeps it tractable even when the result is huge. No deduplication happens, so
/// this is the blow-up a naive expression-to-cover conversion pays; the canonical [`Bdd`] count is the
/// contrast.
fn naive_cube_count(expr: &BoolExpr) -> usize {
    // Pass 1: expand XOR so the DNF pass only meets AND / OR / NOT.
    let expanded: BoolExpr = expr.fold(|node| -> BoolExpr {
        match node {
            ExprNode::Variable(name) => BoolExpr::var(name),
            ExprNode::Constant(value) => BoolExpr::constant(value),
            ExprNode::Not(inner) => !inner,
            ExprNode::And(left, right) => left & right,
            ExprNode::Or(left, right) => left | right,
            ExprNode::Xor(left, right) => (left.clone() & !right.clone()) | (!left & right),
        }
    });

    // Pass 2: top-down naive DNF. `negate` is the parity pushed down from the root; AND and OR swap
    // under negation (De Morgan), and a leaf emits a single literal of the right polarity.
    let dnf = expanded.fold_with_context(
        false,
        |node, &negate| match node {
            ExprNode::Not(()) => (!negate, !negate),
            _ => (negate, negate),
        },
        |node, negate| match node {
            ExprNode::Variable(name) => vec![BTreeMap::from([(Arc::<str>::from(name), !negate)])],
            ExprNode::Constant(value) => {
                if value ^ negate {
                    vec![Cube::new()]
                } else {
                    Vec::new()
                }
            }
            ExprNode::Not(child) => child, // negation already pushed into `child` via the context
            ExprNode::And(left, right) => {
                if negate {
                    union(left, right)
                } else {
                    cross(&left, &right)
                }
            }
            ExprNode::Or(left, right) => {
                if negate {
                    cross(&left, &right)
                } else {
                    union(left, right)
                }
            }
            ExprNode::Xor(_, _) => unreachable!("XOR expanded away before the DNF pass"),
        },
    );
    dnf.len()
}

fn main() -> std::io::Result<()> {
    println!("C-Element: Two Equivalent Formulations of next_q");
    println!("=================================================\n");

    // The C-element characteristic functions as syntactic `BoolExpr` values (composed through `expr!`,
    // which grafts the shared sub-expressions in). These are NOT canonicalised: `next_q_v1` and
    // `next_q_v2` are different syntax for the same function.
    let activation = BoolExpr::var("a") & BoolExpr::var("b");
    let deactivation = expr!(!"a" & !"b");
    let hold = expr!(activation ^ !deactivation);
    let next_q_v1 = expr!((activation | "q") & !deactivation);
    let next_q_v2 = expr!(activation | "q" & hold);

    println!("Original Functions:");
    println!("  activation   = {activation}");
    println!("  deactivation = {deactivation}");
    println!("  hold         = {hold} (activation XOR !deactivation)");
    println!("  next_q_v1    = {next_q_v1}");
    println!("  next_q_v2    = {next_q_v2}");
    println!();

    let functions = [
        ("activation", &activation),
        ("deactivation", &deactivation),
        ("hold", &hold),
        ("next_q_v1", &next_q_v1),
        ("next_q_v2", &next_q_v2),
    ];

    println!("==============================================================================");
    println!("BDD CANONICALISATION IMPACT DEMONSTRATION");
    println!("==============================================================================\n");

    // Stage 1: cube count from the syntactic BoolExpr (naive De Morgan / XOR expansion).
    println!("Stage 1: Naive expansion of the syntactic BoolExpr (no canonicalisation)...");
    let mut naive_counts = Vec::new();
    let mut total_naive = 0;
    for (name, expr) in functions {
        let count = naive_cube_count(expr);
        naive_counts.push(count);
        total_naive += count;
        println!("  {name:<15} {count:>10} cubes");
    }
    println!(
        "  {:<15} {:>10} cubes (sum of all outputs)\n",
        "TOTAL", total_naive
    );

    // Stage 2: cube count from the canonical Bdd — build each function into one shared builder. The two
    // `next_q` formulations canonicalise to the *same* handle.
    println!("Stage 2: Canonical BDD form...");
    let builder = bdd_builder!();
    let bdds: Vec<(&str, _)> = functions
        .iter()
        .map(|&(name, expr)| (name, builder.build(expr)))
        .collect();

    let mut bdd_counts = Vec::new();
    let mut total_bdd = 0;
    for (name, bdd) in &bdds {
        let count = bdd.cover().num_cubes();
        bdd_counts.push(count);
        total_bdd += count;
        println!("  {name:<15} {count:>10} cubes");
    }
    println!(
        "  {:<15} {:>10} cubes (sum of all outputs)",
        "TOTAL", total_bdd
    );
    println!(
        "  ({:.1}x reduction from naive)\n",
        total_naive as f64 / total_bdd as f64
    );

    // Build a multi-output cover from the canonical BDDs and let Espresso minimise it.
    let mut cover = Cover::new(CoverType::F);
    for (name, bdd) in &bdds {
        cover.add_bdd(bdd, name)?;
    }

    // Stage 3: Espresso exact minimisation.
    println!("Stage 3: Espresso exact minimisation...");
    let minimized = cover.minimize_exact()?;
    let mut esp_counts = Vec::new();
    for (i, (name, _)) in functions.iter().enumerate() {
        let count = minimized
            .cubes()
            .filter(|c| c.outputs().value_at(i))
            .count();
        esp_counts.push(count);
        println!("  {name:<15} {count:>10} cubes");
    }
    let total_esp = minimized.num_cubes();
    println!(
        "  {:<15} {:>10} cubes (unique cubes in cover)\n",
        "TOTAL", total_esp
    );

    // Comparison table.
    println!("Cube counts at three stages:\n");
    println!(
        "{:<15} {:>10} {:>10} {:>10} {:>15}",
        "Function", "Naive", "BDD", "Espresso", "BDD Reduction"
    );
    println!("{}", "-".repeat(70));
    for (i, (name, _)) in functions.iter().enumerate() {
        println!(
            "{:<15} {:>10} {:>10} {:>10} {:>14.1}x",
            name,
            naive_counts[i],
            bdd_counts[i],
            esp_counts[i],
            naive_counts[i] as f64 / bdd_counts[i] as f64
        );
    }
    println!(
        "\nConclusion: canonicalising the syntactic BoolExpr into a BDD reduces the cube count fed to",
    );
    println!(
        "            Espresso by ~{:.0}x; Espresso then achieves the final optimal form.\n",
        total_naive as f64 / total_bdd as f64
    );

    println!("==============================================================================\n");

    // Equivalence of the two formulations is an O(1) canonical-BDD comparison — they share a builder, so
    // equal functions are the identical handle.
    println!("Equivalence Check:");
    println!("==================");
    let next_q_v1_bdd = &bdds[3].1;
    let next_q_v2_bdd = &bdds[4].1;
    if next_q_v1_bdd.equivalent_to(next_q_v2_bdd) {
        println!("✓ Both formulations are logically equivalent (identical canonical BDD)!");
        println!("  - Version 1: (activation | q) & !deactivation");
        println!("  - Version 2: activation | q & hold");
        println!("  where hold = activation XOR !deactivation");
    } else {
        println!("✗ Formulations differ (unexpected!)");
    }
    println!();

    println!("==============================================================================\n");

    // Display all minimised functions.
    println!("C-Element Minimized Functions:");
    for (name, expr) in minimized.to_exprs() {
        println!("{name:20} = {expr}");
    }

    Ok(())
}
