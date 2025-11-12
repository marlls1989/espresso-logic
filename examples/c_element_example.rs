use espresso_logic::{expr, BoolExpr, Cover, CoverType, Minimizable};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Compute XOR of two boolean expressions
fn xor(a: &BoolExpr, b: &BoolExpr) -> BoolExpr {
    expr!(a * !b + !a * b)
}

// ============================================================================
// Naive Expression AST (for accurate complexity measurement)
// ============================================================================
// This is a simple AST that captures expression structure as-built, without
// any BDD optimization. It allows us to measure the true exponential blowup
// that occurs with naive De Morgan expansion.

/// Simple expression AST that captures structure without BDD optimization
#[derive(Clone)]
enum NaiveExpr {
    Variable(Arc<str>),
    And(Box<NaiveExpr>, Box<NaiveExpr>),
    Or(Box<NaiveExpr>, Box<NaiveExpr>),
    Not(Box<NaiveExpr>),
}

impl NaiveExpr {
    fn variable(name: &str) -> Self {
        NaiveExpr::Variable(Arc::from(name))
    }

    fn and(self, other: NaiveExpr) -> Self {
        NaiveExpr::And(Box::new(self), Box::new(other))
    }

    fn or(self, other: NaiveExpr) -> Self {
        NaiveExpr::Or(Box::new(self), Box::new(other))
    }

    fn not(self) -> Self {
        NaiveExpr::Not(Box::new(self))
    }
}

/// XOR for naive expressions
fn naive_xor(a: NaiveExpr, b: NaiveExpr) -> NaiveExpr {
    // a XOR b = (a * !b) + (!a * b)
    a.clone().and(b.clone().not()).or(a.not().and(b))
}

// ============================================================================
// Naive De Morgan Cube Counting (top-down negation pushing)
// ============================================================================
// This demonstrates what happens when converting to DNF using naive De Morgan
// expansion without BDD optimization.

/// Compute naive DNF cube count using De Morgan's laws (top-down approach)
fn naive_cube_count(expr: &NaiveExpr) -> usize {
    naive_to_dnf(expr, false).len()
}

/// Convert naive expression to DNF with naive De Morgan expansion (top-down)
/// Directly walks the NaiveExpr tree with pattern matching
fn naive_to_dnf(expr: &NaiveExpr, negate: bool) -> Vec<BTreeMap<Arc<str>, bool>> {
    match expr {
        NaiveExpr::Variable(name) => {
            let mut cube = BTreeMap::new();
            cube.insert(Arc::clone(name), !negate); // Flip polarity if negated
            vec![cube]
        }
        NaiveExpr::Not(inner) => {
            // NOT: flip the negation flag (De Morgan top-down!)
            naive_to_dnf(inner, !negate)
        }
        NaiveExpr::And(left, right) => {
            if negate {
                // De Morgan: ~(A * B) = ~A + ~B (OR of negated children)
                let mut left_cubes = naive_to_dnf(left, true);
                let right_cubes = naive_to_dnf(right, true);
                left_cubes.extend(right_cubes);
                left_cubes
            } else {
                // AND: cross product
                let left_cubes = naive_to_dnf(left, false);
                let right_cubes = naive_to_dnf(right, false);
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
        NaiveExpr::Or(left, right) => {
            if negate {
                // De Morgan: ~(A + B) = ~A * ~B (AND of negated children)
                let left_cubes = naive_to_dnf(left, true);
                let right_cubes = naive_to_dnf(right, true);
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
                let mut left_cubes = naive_to_dnf(left, false);
                let right_cubes = naive_to_dnf(right, false);
                left_cubes.extend(right_cubes);
                left_cubes
            }
        }
    }
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

    // ========================================================================
    // Build NAIVE expressions (capture structure without BDD optimization)
    // ========================================================================
    // These naive expressions preserve the actual expression structure,
    // allowing us to measure the true cost of naive De Morgan expansion.

    let a = NaiveExpr::variable("a");
    let b = NaiveExpr::variable("b");
    let q = NaiveExpr::variable("q");

    // Activation: a * b
    let naive_activation = a.clone().and(b.clone());

    // Deactivation: !a * !b
    let naive_deactivation = a.clone().not().and(b.clone().not());

    // Hold: XOR of activation and negation of deactivation
    // This causes exponential blowup with naive expansion!
    let naive_hold = naive_xor(naive_activation.clone(), naive_deactivation.clone().not());

    // next_q_v1: (activation + q) * !deactivation
    let naive_next_q_v1 = naive_activation
        .clone()
        .or(q.clone())
        .and(naive_deactivation.clone().not());

    // next_q_v2: activation + q * hold
    let naive_next_q_v2 = naive_activation
        .clone()
        .or(q.clone().and(naive_hold.clone()));

    // ========================================================================
    // Build BDD-backed expressions (for actual minimization)
    // ========================================================================

    // Define the C-element characteristic functions
    let activation = expr!("a" * "b");
    let deactivation = expr!(!"a" * !"b");

    // Hold region: XOR of activation and negation of deactivation
    // This is the unique hold region where neither activation nor deactivation occurs
    let hold = xor(&activation, &deactivation.not());

    // Two equivalent formulations of next_q:
    // Version 1: Using deactivation directly
    let next_q_v1 = expr!((activation + "q") * !deactivation);

    // Version 2: Using hold (where hold = activation XOR !deactivation)
    let next_q_v2 = expr!(activation + "q" * hold);

    println!("Original Functions:");
    println!("  activation   = {}", activation);
    println!("  deactivation = {}", deactivation);
    println!("  hold         = {} (activation XOR !deactivation)", hold);
    println!();

    println!("Two Formulations of next_q (before minimization):");
    println!("  next_q_v1 = (activation + q) * !deactivation");
    println!("            = {}", next_q_v1);
    println!();
    println!("  next_q_v2 = activation + q * hold");
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
        "deactivation",
        "hold",
        "next_q_v1",
        "next_q_v2",
    ];
    let naive_expressions = [
        &naive_activation,
        &naive_deactivation,
        &naive_hold,
        &naive_next_q_v1,
        &naive_next_q_v2,
    ];

    // Stage 1: Count cubes with naive De Morgan expansion
    println!("Stage 1: Naive De Morgan expansion...");
    println!("           Walking actual expression structure (not BDD-optimised)");
    let mut naive_counts = Vec::new();
    let mut total_naive = 0;
    for (i, expr) in naive_expressions.iter().enumerate() {
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
    let total_bdd = cover.cubes().count();
    println!(
        "  {:<15} {:>10} cubes (unique cubes in cover)",
        "TOTAL", total_bdd
    );
    println!(
        "  ({:.1}x reduction from naive)\n",
        total_naive as f64 / total_bdd as f64
    );

    // Stage 3: Minimize with Espresso (using exact algorithm)
    println!("Stage 3: Running Espresso exact minimization...");
    let minimized = cover.minimize_exact()?;

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
    for i in 0..5 {
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
        println!("  - Version 2: activation + q * hold");
        println!("  where hold = activation XOR !deactivation");
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
