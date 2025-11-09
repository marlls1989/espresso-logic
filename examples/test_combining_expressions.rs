use espresso_logic::{expr, BoolExpr};

fn main() -> std::io::Result<()> {
    // Create expressions using different methods
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let func_a = BoolExpr::parse("input1 * input2")?;
    let func_b = BoolExpr::parse("input3 + input4")?;

    // Combine them all with expr! - clean and readable
    let xor = expr!(a * !b + !a * b);
    let combined = expr!(func_a + func_b);

    // Mix created and parsed expressions
    let selector = BoolExpr::variable("mode");
    let output = expr!(selector * func_a + !selector * func_b);

    println!("XOR: {}", xor);
    println!("Combined: {}", combined);
    println!("Output: {}", output);

    Ok(())
}
