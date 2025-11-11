use espresso_logic::{expr, BoolExpr, Cover, CoverType, ExprNode, Minimizable};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Compute XOR of two boolean expressions using expr! macro
fn xor(a: &BoolExpr, b: &BoolExpr) -> BoolExpr {
    expr!(a * !b + !a * b)
}

// ============================================================================
// Naive De Morgan Cube Counting (top-down negation pushing)
// ============================================================================
// This demonstrates what happens when converting to DNF using naive De Morgan
// expansion without BDD optimization. This causes exponential blowup with
// negations of complex expressions.
//
// De Morgan's laws work TOP-DOWN: we push negations down through the tree.

/// Compute naive DNF cube count using De Morgan's laws (top-down approach)
fn naive_cube_count(expr: &BoolExpr) -> usize {
    naive_to_dnf(expr).len()
}

/// Convert expression to DNF with naive De Morgan expansion (top-down)
/// Uses fold_with_context to pass negation flag top-down through the tree
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
                cube.insert(Arc::from(name), !negate); // Flip polarity if negated
                vec![cube]
            }
            ExprNode::Not(()) => {
                // NOT: flip the negation flag for the child (De Morgan top-down!)
                recurse_left(!negate)
            }
            ExprNode::And((), ()) => {
                if negate {
                    // De Morgan: ~(A * B) = ~A + ~B (OR of negated children)
                    let left_cubes = recurse_left(true); // Negate left
                    let right_cubes = recurse_right(true); // Negate right
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
                    // De Morgan: ~(A + B) = ~A * ~B (AND of negated children)
                    let left_cubes = recurse_left(true); // Negate left
                    let right_cubes = recurse_right(true); // Negate right
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
/// Returns None if they contradict (e.g., x AND ~x)
fn merge_cubes(
    left: &BTreeMap<Arc<str>, bool>,
    right: &BTreeMap<Arc<str>, bool>,
) -> Option<BTreeMap<Arc<str>, bool>> {
    let mut result = left.clone();
    for (var, &polarity) in right {
        if let Some(&existing) = result.get(var) {
            if existing != polarity {
                // Contradiction: x AND ~x = FALSE
                return None;
            }
        } else {
            result.insert(Arc::clone(var), polarity);
        }
    }
    Some(result)
}

fn main() -> std::io::Result<()> {
    // 5-input threshold gate with feedback q
    // Activation: at least 4 inputs high (4 or 5)
    // Deactivation: at most 1 input high (0 or 1)
    // Hold: 2 or 3 inputs high

    // Define all combinations for activation (at least 4 high)
    let activation = expr!(
        // All 5 high
        "a" * "b" * "c" * "d" * "e" +
        // Any 4 high (5 choose 4 = 5 combinations)
        "a" * "b" * "c" * "d" * !"e" +
        "a" * "b" * "c" * !"d" * "e" +
        "a" * "b" * !"c" * "d" * "e" +
        "a" * !"b" * "c" * "d" * "e" +
        !"a" * "b" * "c" * "d" * "e"
    );

    // Define all combinations for deactivation (at most 1 high)
    let deactivation = expr!(
        // All 5 low
        !"a" * !"b" * !"c" * !"d" * !"e" +
        // Any 1 high (5 combinations)
        "a" * !"b" * !"c" * !"d" * !"e" +
        !"a" * "b" * !"c" * !"d" * !"e" +
        !"a" * !"b" * "c" * !"d" * !"e" +
        !"a" * !"b" * !"c" * "d" * !"e" +
        !"a" * !"b" * !"c" * !"d" * "e"
    );

    // Hold region is XOR of activation and negation of deactivation
    let hold = xor(&activation, &deactivation.not());

    // Two equivalent formulations of next_q:
    // Version 1: Using deactivation directly - (activation + q) * !deactivation
    let next_q_v1 = expr!((activation + "q") * !deactivation);

    // Version 2: Using hold - activation + q * hold (where hold = activation XOR !deactivation)
    let next_q_v2 = expr!(activation + "q" * hold);

    // ========================================================================
    // DEMONSTRATE BDD PRE-MINIMIZATION IMPACT
    // ========================================================================

    println!("==============================================================================");
    println!("BDD PRE-MINIMIZATION IMPACT DEMONSTRATION");
    println!("==============================================================================\n");

    let output_names = [
        "activation",
        "deactivation",
        "hold",
        "next_q_v1",
        "next_q_v2",
    ];
    let expressions = [&activation, &deactivation, &hold, &next_q_v1, &next_q_v2];

    // Stage 1: Count cubes with naive De Morgan expansion (before any optimization)
    println!("Stage 1: Naive De Morgan expansion (exponential blowup)...");
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

    // Stage 2: Create cover with BDD-based conversion (automatic pre-minimization)
    println!("Stage 2: Creating cover with BDD conversion (canonical form)...");
    let mut cover = Cover::new(CoverType::F);
    cover.add_expr(&activation, "activation")?;
    cover.add_expr(&deactivation, "deactivation")?;
    cover.add_expr(&hold, "hold")?;
    cover.add_expr(&next_q_v1, "next_q_v1")?;
    cover.add_expr(&next_q_v2, "next_q_v2")?;

    // Count cubes per output after BDD conversion
    let mut bdd_counts = Vec::new();
    for (i, name) in output_names.iter().enumerate() {
        let count = cover.cubes().filter(|c| c.outputs()[i]).count();
        bdd_counts.push(count);
        println!("  {:<15} {:>10} cubes", name, count);
    }
    // Total unique cubes (not sum, since cubes can be shared across outputs)
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
    // Total unique cubes after Espresso
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

    for i in 0..5 {
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
    println!(
        "• BDD reduces 'hold' from {} to {} cubes ({}x reduction!)",
        naive_counts[2],
        bdd_counts[2],
        naive_counts[2] / bdd_counts[2]
    );
    println!(
        "• BDD reduces 'next_q_v1' from {} to {} cubes ({}x reduction!)",
        naive_counts[3],
        bdd_counts[3],
        naive_counts[3] / bdd_counts[3]
    );
    println!(
        "• BDD reduces 'next_q_v2' from {} to {} cubes ({}x reduction!)",
        naive_counts[4],
        bdd_counts[4],
        naive_counts[4] / bdd_counts[4]
    );
    println!(
        "• Espresso further optimizes 'hold': {} → {} cubes ({:.0}% reduction)",
        bdd_counts[2],
        esp_counts[2],
        (1.0 - esp_counts[2] as f64 / bdd_counts[2] as f64) * 100.0
    );
    println!(
        "• Espresso further optimizes 'next_q_v1': {} → {} cubes ({:.0}% reduction)",
        bdd_counts[3],
        esp_counts[3],
        (1.0 - esp_counts[3] as f64 / bdd_counts[3] as f64) * 100.0
    );
    println!(
        "• Espresso further optimizes 'next_q_v2': {} → {} cubes ({:.0}% reduction)",
        bdd_counts[4],
        esp_counts[4],
        (1.0 - esp_counts[4] as f64 / bdd_counts[4] as f64) * 100.0
    );
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
        println!("  - Version 2: activation + q * hold");
        println!("  where hold = activation XOR !deactivation");
    } else {
        println!("✗ Formulations differ (unexpected!)");
    }
    println!("\n==============================================================================\n");

    // Display results
    println!("5-Input Threshold Gate Minimized Functions:");
    for (name, expr) in minimized.to_exprs() {
        println!("{:15} = {}", name, expr);
    }

    // ========================================================================
    // ACTUAL MEASURED RESULTS - Demonstrating BDD's Critical Role
    // ========================================================================
    //
    // This example measures cube counts at three optimization stages to quantify
    // the impact of BDD pre-minimization on Espresso's input size.
    //
    // STAGE 1 - Naive De Morgan Expansion (No BDD):
    // ------------------------------------------------
    // Using top-down De Morgan's laws without BDD optimization causes exponential
    // blowup when negations are applied to complex expressions:
    //
    //   activation:    6 cubes (simple OR of products - already in DNF)
    //   deactivation:  6 cubes (simple OR of products - already in DNF)
    //   hold:          375,840 cubes! (XOR + negation → MASSIVE explosion!)
    //   next_q:        7,006 cubes (negation of 6-term OR → huge cross-product)
    //   ─────────────────────────────────────────────────────────────
    //   TOTAL:         382,858 cubes (sum across all outputs)
    //
    // The 'hold' expression is particularly dramatic: xor(activation, !deactivation)
    // expands to (A*!B + !A*B), where !B requires negating a 6-term OR. De Morgan's
    // law transforms !(t1+t2+t3+t4+t5+t6) into !t1*!t2*!t3*!t4*!t5*!t6, creating
    // massive cross-product expansion when combined with A.
    //
    // STAGE 2 - BDD-Based DNF (Canonical Form):
    // ------------------------------------------
    // Converting through BDD provides canonical representation and eliminates
    // redundancies automatically:
    //
    //   activation:    5 cubes (BDD eliminated 1 redundant term, 1.2x reduction)
    //   deactivation:  5 cubes (BDD eliminated 1 redundant term, 1.2x reduction)
    //   hold:          14 cubes (BDD achieves 26,845x reduction! 375,840→14)
    //   next_q:        19 cubes (BDD achieves 369x reduction! 7,006→19)
    //   ─────────────────────────────────────────────────────────────
    //   TOTAL:         43 unique cubes in cover (8,904x reduction!)
    //
    // Note: The total is unique cubes, not the sum of per-output counts, because
    // cubes can have multiple output bits set (shared across functions).
    //
    // STAGE 3 - Espresso Minimization (Final Optimal Form):
    // ------------------------------------------------------
    // Espresso further optimizes using advanced heuristics:
    //
    //   activation:    5 cubes (already minimal - no change)
    //   deactivation:  5 cubes (already minimal - no change)
    //   hold:          10 cubes (Espresso achieves 29% further reduction, 14→10)
    //   next_q:        15 cubes (Espresso achieves 21% further reduction, 19→15)
    //   ─────────────────────────────────────────────────────────────
    //   TOTAL:         30 unique cubes in cover (30% further reduction)
    //
    // KEY INSIGHTS:
    // =============
    // 1. **BDD is ESSENTIAL** for expressions with negations:
    //    - Without BDD: 382,858 cubes → Espresso would be intractable
    //    - With BDD: 43 cubes → Espresso runs efficiently
    //    - Reduction factor: 8,904x (reduces input by 99.99%!)
    //
    // 2. **Espresso is STILL NEEDED** after BDD:
    //    - BDD provides canonical form, but not necessarily minimal form
    //    - Espresso achieves additional 30% reduction (43→30 cubes)
    //    - Critical for final optimization of hold (14→10) and next_q (19→15)
    //
    // 3. **The Pipeline is Complementary**:
    //    - BDD excels at handling negations and creating canonical form
    //    - Espresso excels at finding minimal cover through heuristic search
    //    - Together they achieve both tractability AND optimality
    //
    // FINAL MINIMIZED EXPRESSIONS (30 unique cubes total):
    // =====================================================
    // activation   (5 cubes):  a*b*c*e + a*b*d*e + a*c*d*e + b*c*d*e + a*b*c*d
    //
    // deactivation (5 cubes):  ~b*~c*~d*~e + ~a*~c*~d*~e + ~a*~b*~d*~e +
    //                          ~a*~b*~c*~e + ~a*~b*~c*~d
    //
    // hold         (10 cubes): ~a*~b*c*e + ~a*~c*d*e + ~a*c*d*~e + ~a*b*~d*e +
    //                          a*~b*~c*d + a*~b*c*~e + a*~b*~d*e + b*~c*d*~e +
    //                          a*b*~c*~d + b*c*~d*~e
    //
    // next_q       (15 cubes): a*d*q + a*e*q + a*c*q + a*b*q + b*d*q + b*e*q +
    //                          b*c*q + c*d*q + c*e*q + d*e*q + a*b*c*e + a*b*d*e +
    //                          a*c*d*e + b*c*d*e + a*b*c*d

    Ok(())
}
