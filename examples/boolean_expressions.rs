//! Example: Boolean expression minimization
//!
//! This example demonstrates how to use boolean expressions with the expr! macro,
//! method-based API, and parsing to create and minimize boolean functions.

use espresso_logic::{expr, BoolExpr, Cover, ExprCover};

fn main() -> std::io::Result<()> {
    println!("=== Boolean Expression Examples ===\n");

    // Example 1: Programmatic construction with expr! macro
    println!("1. Programmatic Construction (using expr! macro):");
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let _c = BoolExpr::variable("c");

    // XOR function: a*~b + ~a*b - clean syntax!
    let xor = expr!(a * !b + !a * b);
    println!("   XOR = a*~b + ~a*b");
    println!("   Variables: {:?}", xor.collect_variables());
    let xor_cover = ExprCover::from_expr(xor);
    println!(
        "   Inputs: {}, Outputs: {}",
        xor_cover.num_inputs(),
        xor_cover.num_outputs()
    );
    println!();

    // Example 2: Parsing from string
    println!("2. Parsing from String:");
    let parsed_expr = BoolExpr::parse("(a + b) * (c + d)").unwrap();
    println!("   Expression: (a + b) * (c + d)");
    println!("   Variables: {:?}", parsed_expr.collect_variables());
    let parsed_cover = ExprCover::from_expr(parsed_expr);
    println!(
        "   Inputs: {}, Outputs: {}",
        parsed_cover.num_inputs(),
        parsed_cover.num_outputs()
    );
    println!();

    // Example 3: Complex expression with negation
    println!("3. Complex Expression with Negation:");
    let complex = BoolExpr::parse("~(a * b) + (c * ~d)").unwrap();
    println!("   Expression: ~(a * b) + (c * ~d)");
    println!("   Variables: {:?}", complex.collect_variables());
    println!();

    // Example 4: Minimization
    println!("4. Minimization Example:");
    println!("   Original: a*b + a*b*c (redundant term)");

    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");
    let redundant = expr!(a * b + a * b * c);

    println!("   Before minimization:");
    println!("      Variables: {:?}", redundant.collect_variables());

    let mut redundant_cover = ExprCover::from_expr(redundant);
    redundant_cover.minimize()?;
    let minimized = redundant_cover.to_expr();

    println!("   After minimization:");
    println!("      Expression: {}", minimized);
    println!();

    // Example 5: XNOR function
    println!("5. XNOR Function (equivalence):");
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let xnor = expr!(a * b + !a * !b);

    println!("   XNOR = a*b + ~a*~b");
    println!("   Before minimize: {}", xnor);

    let mut xnor_cover = ExprCover::from_expr(xnor);
    xnor_cover.minimize()?;
    let minimized_xnor = xnor_cover.to_expr();

    println!("   After minimize:  {}", minimized_xnor);
    println!();

    // Example 6: Three-variable function
    println!("6. Three-Variable Majority Function:");
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let c = BoolExpr::variable("c");

    // Majority function: true if at least 2 of 3 inputs are true
    // For more complex expressions, the method API is clearer
    let majority = a.and(&b).or(&b.and(&c)).or(&a.and(&c));
    let mut majority_cover = ExprCover::from_expr(majority);

    println!("   Majority = a*b + b*c + a*c");
    println!("   Before minimize: {} cubes", majority_cover.num_cubes());

    majority_cover.minimize()?;

    println!("   After minimize:  {} cubes", majority_cover.num_cubes());
    println!();

    // Example 7: Converting to PLA format
    println!("7. PLA Format Export:");
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");
    let simple = a.and(&b);
    let simple_cover = ExprCover::from_expr(simple);

    let pla_string = simple_cover.to_pla_string(espresso_logic::PLAType::F)?;
    println!("   Expression: a * b");
    println!("   PLA format:");
    for line in pla_string.lines() {
        println!("      {}", line);
    }
    println!();

    // Example 8: Parsing with constants
    println!("8. Expressions with Constants:");
    let expr_with_const = BoolExpr::parse("a * 1 + 0 * b").unwrap();
    println!("   Expression: a * 1 + 0 * b");
    println!("   Variables: {:?}", expr_with_const.collect_variables());
    println!();

    // Example 9: De Morgan's laws in action
    println!("9. De Morgan's Laws:");
    let a = BoolExpr::variable("a");
    let b = BoolExpr::variable("b");

    let demorgan1 = expr!(!(a * b));
    let cover1 = ExprCover::from_expr(demorgan1);
    println!("   ~(a * b) has {} variables", cover1.num_inputs());

    let demorgan2 = expr!(!(a + b));
    let cover2 = ExprCover::from_expr(demorgan2);
    println!("   ~(a + b) has {} variables", cover2.num_inputs());
    println!();

    // Example 10: Comparison - same logical function, different expressions
    println!("10. Equivalent Expressions:");
    let expr1 = BoolExpr::parse("a * b + a * c").unwrap();
    let expr2 = BoolExpr::parse("a * (b + c)").unwrap();

    let mut cover1 = ExprCover::from_expr(expr1);
    let mut cover2 = ExprCover::from_expr(expr2);

    println!("    Expression 1: a * b + a * c");
    println!("    Expression 2: a * (b + c)");
    println!("    Both have {} variables", cover1.num_inputs());

    cover1.minimize()?;
    cover2.minimize()?;

    println!("    After minimization, they should be equivalent");
    println!();

    // Example 11: Iterating over cubes
    println!("11. Cube Iteration:");
    let expr = BoolExpr::parse("a * b + ~a * c").unwrap();
    let cover = ExprCover::from_expr(expr);
    println!("    Expression: a * b + ~a * c");
    println!("    Cubes:");

    for (i, (inputs, outputs)) in cover.cubes_iter().enumerate() {
        print!("      Cube {}: inputs=[", i + 1);
        for input in &inputs {
            match input {
                Some(true) => print!("1"),
                Some(false) => print!("0"),
                None => print!("-"),
            }
        }
        print!("] outputs=[");
        for output in &outputs {
            match output {
                Some(true) => print!("1"),
                Some(false) => print!("0"),
                None => print!("-"),
            }
        }
        println!("]");
    }
    println!();

    println!("=== Examples Complete ===");
    Ok(())
}
