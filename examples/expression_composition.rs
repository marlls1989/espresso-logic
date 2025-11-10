use espresso_logic::{expr, BoolExpr, Minimizable};

fn main() -> std::io::Result<()> {
    println!("=== Expression Composition Example ===\n");

    // Parse expressions from user input (simulated)
    let user_func1 = BoolExpr::parse("a * b + c")?;
    let user_func2 = BoolExpr::parse("d + e * f")?;
    let user_func3 = BoolExpr::parse("g * h")?;

    println!("User function 1: {}", user_func1);
    println!("User function 2: {}", user_func2);
    println!("User function 3: {}", user_func3);
    println!();

    // Compose them using expr! macro - clean and readable
    let combined = expr!(user_func1 * user_func2 + !user_func3);
    println!("Combined: {}", combined);
    println!();

    // Build more complex compositions
    let condition = BoolExpr::parse("enable")?;
    let output = expr!(condition * user_func1 + !condition * user_func2);
    println!("Conditional (enable ? func1 : func2): {}", output);
    println!();

    // Compose minimized sub-expressions
    let minimized1 = user_func1.minimize()?;
    let minimized2 = user_func2.minimize()?;
    let final_expr = expr!(minimized1 + minimized2);
    println!("Composed minimized expressions: {}", final_expr);

    Ok(())
}
