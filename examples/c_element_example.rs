use espresso_logic::{expr, BoolExpr, Cover, CoverType, ExprNode, Minimizable};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Compute XOR of two boolean expressions
fn xor(a: &BoolExpr, b: &BoolExpr) -> BoolExpr {
    expr!(a * !b + !a * b)
}

// ============================================================================
// Naive De Morgan Cube Counting (top-down negation pushing)
// ============================================================================
// This demonstrates what happens when converting to DNF using naive De Morgan
// expansion without BDD optimization.

/// Compute naive DNF cube count using De Morgan's laws (top-down approach)
fn naive_cube_count(expr: &BoolExpr) -> usize {
    naive_to_dnf(expr).len()
}

/// Convert expression to DNF with naive De Morgan expansion (top-down)
fn naive_to_dnf(expr: &BoolExpr) -> Vec<BTreeMap<Arc<str>, bool>> {
    expr.fold_with_context(false, |node, negate, recurse_left, recurse_right| {
        match node {
            ExprNode::Constant(val) => {
                let result_val = if negate { !val } else { val };
                if result_val {
                    vec![BTreeMap::new()] // TRUE = one empty cube
                } else {
                    vec![] // FALSE = no cubes
                }
            }
            ExprNode::Variable(name) => {
                let mut cube = BTreeMap::new();
                cube.insert(Arc::from(name), !negate);
                vec![cube]
            }
            ExprNode::Not(()) => {
                // NOT: flip the negation flag for the child
                recurse_left(!negate)
            }
            ExprNode::And((), ()) => {
                if negate {
                    // De Morgan: ~(A * B) = ~A + ~B
                    let left_cubes = recurse_left(true);
                    let right_cubes = recurse_right(true);
                    let mut result = left_cubes;
                    result.extend(right_cubes);
                    result
                } else {
                    // AND: cross product
                    let left_cubes = recurse_left(false);
                    let right_cubes = recurse_right(false);
                    let mut result = Vec::new();
                    for left_cube in &left_cubes {
                        for right_cube in &right_cubes {
                            if let Some(merged) = merge_cubes(left_cube, right_cube) {
                                result.push(merged);
                            }
                        }
                    }
                    result
                }
            }
            ExprNode::Or((), ()) => {
                if negate {
                    // De Morgan: ~(A + B) = ~A * ~B
                    let left_cubes = recurse_left(true);
                    let right_cubes = recurse_right(true);
                    let mut result = Vec::new();
                    for left_cube in &left_cubes {
                        for right_cube in &right_cubes {
                            if let Some(merged) = merge_cubes(left_cube, right_cube) {
                                result.push(merged);
                            }
                        }
                    }
                    result
                } else {
                    // OR: union
                    let left_cubes = recurse_left(false);
                    let right_cubes = recurse_right(false);
                    let mut result = left_cubes;
                    result.extend(right_cubes);
                    result
                }
            }
        }
    })
}

/// Merge two cubes (AND them together)
fn merge_cubes(
    left: &BTreeMap<Arc<str>, bool>,
    right: &BTreeMap<Arc<str>, bool>,
) -> Option<BTreeMap<Arc<str>, bool>> {
    let mut result = left.clone();
    for (var, &polarity) in right {
        if let Some(&existing) = result.get(var) {
            if existing != polarity {
                return None; // Contradiction
            }
        } else {
            result.insert(Arc::clone(var), polarity);
        }
    }
    Some(result)
}

fn main() -> std::io::Result<()> {
    println!("C-Element: Two Equivalent Formulations of next_q");
    println!("=================================================\n");

    // Define the C-element characteristic functions
    let activation = expr!("a" * "b");
    let hold = expr!("a" + "b");
    let deactivation = expr!(!"a" * !"b");

    // Compute hold_xor = activation XOR !deactivation
    let hold_xor = xor(&activation, &deactivation.not());

    // Two equivalent formulations of next_q:
    // Version 1: Using deactivation directly
    let next_q_v1 = expr!((activation + "q") * !deactivation);

    // Version 2: Using hold_xor (where hold_xor = activation XOR !deactivation)
    let next_q_v2 = expr!(activation + "q" * hold_xor);

    println!("Original Functions:");
    println!("  activation   = {}", activation);
    println!("  deactivation = {}", deactivation);
    println!("  hold         = {}", hold);
    println!(
        "  hold_xor     = {} (activation XOR !deactivation)",
        hold_xor
    );
    println!();

    println!("Two Formulations of next_q (before minimization):");
    println!("  next_q_v1 = (activation + q) * !deactivation");
    println!("            = {}", next_q_v1);
    println!();
    println!("  next_q_v2 = activation + q * hold_xor");
    println!("            = {}", next_q_v2);
    println!();

    // ========================================================================
    // DEMONSTRATE BDD PRE-MINIMIZATION IMPACT
    // ========================================================================

    println!("==============================================================================");
    println!("BDD PRE-MINIMIZATION IMPACT DEMONSTRATION");
    println!("==============================================================================\n");

    let output_names = [
        "activation",
        "hold",
        "deactivation",
        "hold_xor",
        "next_q_v1",
        "next_q_v2",
    ];
    let expressions = [
        &activation,
        &hold,
        &deactivation,
        &hold_xor,
        &next_q_v1,
        &next_q_v2,
    ];

    // Stage 1: Count cubes with naive De Morgan expansion
    println!("Stage 1: Naive De Morgan expansion...");
    let mut naive_counts = Vec::new();
    let mut total_naive = 0;
    for (i, expr) in expressions.iter().enumerate() {
        let count = naive_cube_count(expr);
        naive_counts.push(count);
        total_naive += count;
        println!("  {:<15} {:>10} cubes", output_names[i], count);
    }
    println!(
        "  {:<15} {:>10} cubes (sum of all outputs)\n",
        "TOTAL", total_naive
    );

    // Stage 2: Create cover with BDD-based conversion
    println!("Stage 2: Creating cover with BDD conversion (canonical form)...");
    let mut cover = Cover::new(CoverType::F);
    cover.add_expr(&activation, "activation")?;
    cover.add_expr(&hold, "hold")?;
    cover.add_expr(&deactivation, "deactivation")?;
    cover.add_expr(&hold_xor, "hold_xor")?;
    cover.add_expr(&next_q_v1, "next_q_v1")?;
    cover.add_expr(&next_q_v2, "next_q_v2")?;

    // Count cubes per output after BDD conversion
    let mut bdd_counts = Vec::new();
    for (i, name) in output_names.iter().enumerate() {
        let count = cover.cubes().filter(|c| c.outputs()[i]).count();
        bdd_counts.push(count);
        println!("  {:<15} {:>10} cubes", name, count);
    }
    let total_bdd = cover.cubes().count();
    println!(
        "  {:<15} {:>10} cubes (unique cubes in cover)",
        "TOTAL", total_bdd
    );
    println!(
        "  ({:.1}x reduction from naive)\n",
        total_naive as f64 / total_bdd as f64
    );

    // Stage 3: Minimize with Espresso
    println!("Stage 3: Running Espresso minimization...");
    let minimized = cover.minimize()?;

    // Count cubes per output after Espresso minimization
    let mut esp_counts = Vec::new();
    for (i, name) in output_names.iter().enumerate() {
        let count = minimized.cubes().filter(|c| c.outputs()[i]).count();
        esp_counts.push(count);
        println!("  {:<15} {:>10} cubes", name, count);
    }
    let total_esp = minimized.cubes().count();
    println!(
        "  {:<15} {:>10} cubes (unique cubes in cover)",
        "TOTAL", total_esp
    );
    println!(
        "  ({:.0}% further reduction from BDD)\n",
        (1.0 - total_esp as f64 / total_bdd as f64) * 100.0
    );

    // Display comparison table
    println!("Cube counts at three optimization stages:\n");
    println!(
        "{:<15} {:>10} {:>10} {:>10} {:>15}",
        "Function", "Naive", "BDD", "Espresso", "BDD Reduction"
    );
    println!("{}", "-".repeat(70));

    for i in 0..6 {
        println!(
            "{:<15} {:>10} {:>10} {:>10} {:>14.1}x",
            output_names[i],
            naive_counts[i],
            bdd_counts[i],
            esp_counts[i],
            naive_counts[i] as f64 / bdd_counts[i] as f64
        );
    }

    println!("\nKey Insights:");
    for i in 0..6 {
        if naive_counts[i] > bdd_counts[i] * 2 {
            println!(
                "• BDD reduces '{}' from {} to {} cubes ({}x reduction!)",
                output_names[i],
                naive_counts[i],
                bdd_counts[i],
                naive_counts[i] / bdd_counts[i]
            );
        }
    }
    println!(
        "\nConclusion: BDD pre-minimization reduces input to Espresso by ~{}x,",
        total_naive / total_bdd
    );
    println!("           then Espresso achieves final optimal form.\n");

    println!("==============================================================================\n");

    // Verify equivalence of the two next_q formulations
    println!("Equivalence Check:");
    println!("==================");
    let min_next_q_v1 = minimized.to_expr("next_q_v1")?;
    let min_next_q_v2 = minimized.to_expr("next_q_v2")?;

    println!("next_q_v1 (minimized) = {}", min_next_q_v1);
    println!("next_q_v2 (minimized) = {}", min_next_q_v2);
    println!();

    if min_next_q_v1.equivalent_to(&min_next_q_v2) {
        println!("✓ Both formulations are logically equivalent!");
        println!("  - Version 1: (activation + q) * !deactivation");
        println!("  - Version 2: activation + q * hold_xor");
        println!("  where hold_xor = activation XOR !deactivation");
    } else {
        println!("✗ Formulations differ (unexpected!)");
    }
    println!();

    println!("==============================================================================\n");

    // Display all minimized functions
    println!("C-Element Minimized Functions:");
    for (name, expr) in minimized.to_exprs() {
        println!("{:20} = {}", name, expr);
    }

    Ok(())
}
