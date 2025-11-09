use espresso_logic::{expr, BoolExpr};

fn main() -> std::io::Result<()> {
    // Parse complex expressions from user input or config files
    let f1 = BoolExpr::parse("(a + b) * (c + d)")?;
    let f2 = BoolExpr::parse("x * y + z")?;

    // Mix parsed expressions with string literals using expr!
    let out = expr!(!"rst" * ("en" * f1 + !"en" * f2) + "rst" * "def");

    println!("Output: {}", out);

    // Another example: compose minimized sub-expressions
    let expr = BoolExpr::parse("p * q + p * r")?;
    let min = expr.minimize()?;
    let final_expr = expr!(min * "s" + !"t");

    println!("Final: {}", final_expr);

    Ok(())
}
