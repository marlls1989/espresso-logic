//! Example demonstrating variable label support in PLA files
//!
//! This example shows how to:
//! - Load PLA files with .ilb (input labels) and .ob (output labels) directives
//! - Access variable labels through the Cover API
//! - Serialize covers with labels back to PLA format
//! - Use Cover with automatically named variables

use espresso_logic::{BoolExpr, Cover, CoverType, PLAReader, PLAWriter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Variable Labels Example ===\n");

    // Example 1: PLA with labels
    println!("Example 1: Loading PLA with input and output labels");
    println!("---------------------------------------------------");

    let pla_with_labels = r#"
.i 3
.ilb a b c
.o 2
.ob out1 out2
.p 3
001 10
010 01
100 11
.e
"#;

    let cover = Cover::from_pla_string(pla_with_labels)?;

    println!(
        "Loaded PLA with {} inputs and {} outputs",
        cover.num_inputs(),
        cover.num_outputs()
    );

    let input_labels = cover.input_labels();
    if !input_labels.is_empty() {
        println!("Input labels: {:?}", input_labels);
        assert_eq!(input_labels[0].as_ref(), "a");
        assert_eq!(input_labels[1].as_ref(), "b");
        assert_eq!(input_labels[2].as_ref(), "c");
    }

    let output_labels = cover.output_labels();
    if !output_labels.is_empty() {
        println!("Output labels: {:?}", output_labels);
        assert_eq!(output_labels[0].as_ref(), "out1");
        assert_eq!(output_labels[1].as_ref(), "out2");
    }

    // Serialize back to PLA format (labels should be preserved)
    let serialized = cover.to_pla_string(cover.cover_type())?;
    println!("\nSerialized PLA (labels preserved):");
    println!("{}", serialized);

    // Example 2: PLA without labels (auto-generated x0, x1, y0, y1 names)
    println!("\nExample 2: PLA without labels (auto-generated names)");
    println!("-----------------------------------------------------");

    let pla_no_labels = r#"
.i 2
.o 1
.p 2
01 1
10 1
.e
"#;

    let cover_anon = Cover::from_pla_string(pla_no_labels)?;
    println!(
        "Loaded PLA with {} inputs and {} outputs",
        cover_anon.num_inputs(),
        cover_anon.num_outputs()
    );

    let input_labels = cover_anon.input_labels();
    println!("Input labels (auto-generated): {:?}", input_labels);
    // Should be x0, x1

    let output_labels = cover_anon.output_labels();
    println!("Output labels (auto-generated): {:?}", output_labels);
    // Should be y0

    // Example 3: Cover with variable names from boolean expressions
    println!("\nExample 3: Cover with named variables from expressions");
    println!("-------------------------------------------------------");

    let x = BoolExpr::variable("x");
    let y = BoolExpr::variable("y");
    let z = BoolExpr::variable("z");

    // Create expression: (x AND y) OR (y AND z)
    let expr = x.and(&y).or(&y.and(&z));
    let mut expr_cover = Cover::new(CoverType::F);
    expr_cover.add_expr(expr, "output").unwrap();

    println!("Created Cover from boolean expression");
    println!("Number of inputs: {}", expr_cover.num_inputs());

    let labels = expr_cover.input_labels();
    println!("Variable names (sorted): {:?}", labels);
    // Variables are stored in sorted order
    assert_eq!(labels[0].as_ref(), "x");
    assert_eq!(labels[1].as_ref(), "y");
    assert_eq!(labels[2].as_ref(), "z");

    let output_labels = expr_cover.output_labels();
    println!("Output labels: {:?}", output_labels);
    assert_eq!(output_labels[0].as_ref(), "output");

    // We can also serialize to PLA format
    let expr_pla = expr_cover.to_pla_string(expr_cover.cover_type())?;
    println!("\nCover serialized to PLA (with input labels):");
    println!("{}", expr_pla);

    // Example 4: Round-trip test (load -> serialize -> load)
    println!("\nExample 4: Round-trip test");
    println!("--------------------------");

    let original_pla = r#"
.i 2
.ilb enable data
.o 1
.ob output
.p 1
11 1
.e
"#;

    let cover1 = Cover::from_pla_string(original_pla)?;
    let serialized1 = cover1.to_pla_string(cover1.cover_type())?;
    let cover2 = Cover::from_pla_string(&serialized1)?;

    println!("Original input labels: {:?}", cover1.input_labels());
    println!("Round-trip input labels: {:?}", cover2.input_labels());
    assert_eq!(cover1.input_labels(), cover2.input_labels());

    println!("Original output labels: {:?}", cover1.output_labels());
    println!("Round-trip output labels: {:?}", cover2.output_labels());
    assert_eq!(cover1.output_labels(), cover2.output_labels());

    println!("\n✓ Labels preserved through serialization round-trip!");

    println!("\n=== All examples completed successfully! ===");
    Ok(())
}
