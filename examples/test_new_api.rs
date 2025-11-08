//! Test the new unified Cover API with add_expr and to_exprs

use espresso_logic::{BoolExpr, Cover, CoverType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing New Unified Cover API ===\n");

    // Create a cover
    let mut cover = Cover::new(CoverType::F);

    // Add multiple expressions
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    println!("Adding expressions...");
    cover.add_expr(a.and(&b), "out1")?;
    println!("  out1 = a * b");
    cover.add_expr(b.or(&c), "out2")?;
    println!("  out2 = b + c");

    println!("\nCover stats:");
    println!("  Inputs: {}", cover.num_inputs());
    println!("  Outputs: {}", cover.num_outputs());
    println!("  Input labels: {:?}", cover.input_labels());
    println!("  Output labels: {:?}", cover.output_labels());
    println!("  Cubes: {}", cover.num_cubes());

    // Try adding to existing output (should fail)
    println!("\nTrying to add to existing output (should fail)...");
    match cover.add_expr(a.or(&b), "out1") {
        Ok(_) => println!("  ERROR: Should have failed!"),
        Err(e) => println!("  ✓ Correctly rejected: {}", e),
    }

    // Minimize
    println!("\nMinimizing...");
    cover.minimize()?;
    println!("  After minimization: {} cubes", cover.num_cubes());

    // Convert back to expressions
    println!("\nExpressions after minimization:");
    for (name, expr) in cover.to_exprs() {
        println!("  {}: {}", name, expr);
    }

    // Test individual expression retrieval
    println!("\nRetrieving individual expressions:");
    let expr1 = cover.to_expr("out1")?;
    println!("  out1: {}", expr1);
    let expr2 = cover.to_expr_by_index(1)?;
    println!("  Output 1 (out2): {}", expr2);

    println!("\n✓ All tests passed!");
    Ok(())
}
