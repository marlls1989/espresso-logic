use espresso_logic::{expr, BoolExpr, Minimizable};

fn main() -> std::io::Result<()> {
    // Parse complex expressions from user input or config files
    let user_func = BoolExpr::parse("(a + b) * (c + d)")?;
    let config_expr = BoolExpr::parse("x * y + z")?;

    // Mix parsed expressions with string literals using expr!
    let system_output = expr!(
        !"reset" * ("enable" * user_func + !"enable" * config_expr) + "reset" * "default_state"
    );

    println!("System output: {}", system_output);

    // Another example: compose minimized sub-expressions
    let complex = BoolExpr::parse("p * q + p * r")?;
    let minimized = complex.minimize()?;
    let final_expr = expr!(minimized * "s" + !"t");

    println!("Final: {}", final_expr);

    Ok(())
}
