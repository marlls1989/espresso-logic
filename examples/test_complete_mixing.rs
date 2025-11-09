use espresso_logic::{expr, BoolExpr};

fn main() -> std::io::Result<()> {
    // Parse complex expressions from user input or config files
    let user_func = BoolExpr::parse("(a + b) * (c + d)")?;
    let config_expr = BoolExpr::parse("x * y + z")?;

    // Create control variables
    let enable = BoolExpr::variable("enable");
    let reset = BoolExpr::variable("reset");

    // Mix everything: parsed + variables + string literals
    let system_output =
        expr!(!reset * (enable * user_func + !enable * config_expr) + reset * "default_state");

    println!("System output: {}", system_output);

    // Another example: compose minimized sub-expressions
    let sub1 = BoolExpr::parse("p * q + p * r")?;
    let sub2 = BoolExpr::variable("s");
    let minimized = sub1.minimize()?;
    let final_expr = expr!(minimized * sub2 + !"t");

    println!("Final: {}", final_expr);

    Ok(())
}
