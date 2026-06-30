use espresso_logic::{expr, BoolExpr, Cover, Minimizable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Expression Composition Example ===\n");

    // Parse expressions from user input (simulated)
    let user_func1 = BoolExpr::parse("a & b | c")?;
    let user_func2 = BoolExpr::parse("d | e & f")?;
    let user_func3 = BoolExpr::parse("g & h")?;

    println!("User function 1: {}", user_func1);
    println!("User function 2: {}", user_func2);
    println!("User function 3: {}", user_func3);
    println!();

    // Compose them with the `expr!` macro (bare identifiers graft the
    // existing `BoolExpr` values; `&` AND, `|` OR, `!` NOT)
    let combined = expr!(user_func1 & user_func2 | !user_func3);
    println!("Combined: {}", combined);
    println!();

    // Build more complex compositions
    let condition = BoolExpr::parse("enable")?;
    let output = expr!(condition & user_func1 | !condition & user_func2);
    println!("Conditional (enable ? func1 : func2): {}", output);
    println!();

    // Compose minimised sub-expressions. Minimisation lives on `Cover` now, so
    // route each expression through a cover and read back the factored result.
    let minimized1 = Cover::from(&user_func1).minimize()?.to_expr_by_index(0)?;
    let minimized2 = Cover::from(&user_func2).minimize()?.to_expr_by_index(0)?;
    let final_expr = expr!(minimized1 | minimized2);
    println!("Composed minimised expressions: {}", final_expr);

    Ok(())
}
