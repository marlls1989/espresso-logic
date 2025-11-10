use espresso_logic::{expr, BoolExpr, Minimizable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== expr! Procedural Macro Demo ===\n");

    println!("--- Style 1: Using existing BoolExpr variables ---");
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let d = BoolExpr::variable("d");

    println!("Simple operations:");
    println!("   expr!(a * b) = {}", expr!(a * b));
    println!("   expr!(a + b) = {}", expr!(a + b));
    println!("   expr!(!a) = {}", expr!(!a));
    println!();

    println!("Complex patterns:");
    println!("   expr!(a * b + c) = {}", expr!(a * b + c));
    println!(
        "   expr!(a * b + !a * !b) = {} (XOR)",
        expr!(a * b + !a * !b)
    );
    println!("   expr!((a + b) * c) = {}", expr!((a + b) * c));
    println!("   expr!((a + b) * (c + d)) = {}", expr!((a + b) * (c + d)));
    println!();

    println!("--- Style 2: Using string literals (no variable declaration!) ---");
    println!("   expr!(\"x\" * \"y\") = {}", expr!("x" * "y"));
    println!("   expr!(\"x\" + \"y\") = {}", expr!("x" + "y"));
    println!("   expr!(!(\"x\" * \"y\")) = {}", expr!(!("x" * "y")));
    println!(
        "   expr!(\"x\" * \"y\" + !\"x\" * !\"y\") = {} (XOR)",
        expr!("x" * "y" + !"x" * !"y")
    );
    println!();

    println!("--- Style 3: Mixed (variables + string literals) ---");
    let result = expr!(a * "mixed" + b);
    println!("   expr!(a * \"mixed\" + b) = {}", result);
    println!();

    println!("--- Sub-expressions (composable) ---");
    let sub1 = expr!(a * b);
    let sub2 = expr!(c + d);
    println!("   sub1 = expr!(a * b) = {}", sub1);
    println!("   sub2 = expr!(c + d) = {}", sub2);
    println!("   expr!(sub1 + sub2) = {}", expr!(sub1 + sub2));
    println!();

    println!("--- Minimization ---");
    let redundant = expr!("p" * "q" + "p" * "q" * "r");
    println!("   Before: {}", redundant);
    let minimized = redundant.minimize()?;
    println!("   After:  {}", minimized);
    println!();

    println!("✓ All three styles work perfectly with clean, readable syntax!");

    Ok(())
}
