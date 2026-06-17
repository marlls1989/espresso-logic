use espresso_logic::{Cover, CoverType, Cube, CubeType, Minimizable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Test case from test_cover_many_cubes
    let mut cover = Cover::<(), ()>::anonymous(CoverType::F);

    println!("Adding 4 cubes:");
    cover.push(Cube::anonymous(
        &[Some(false), Some(false), Some(true)],
        &[true],
        CubeType::F,
    )); // 001 -> 1
    println!("  001 -> 1");
    cover.push(Cube::anonymous(
        &[Some(false), Some(true), Some(true)],
        &[true],
        CubeType::F,
    )); // 011 -> 1
    println!("  011 -> 1");
    cover.push(Cube::anonymous(
        &[Some(true), Some(false), Some(true)],
        &[true],
        CubeType::F,
    )); // 101 -> 1
    println!("  101 -> 1");
    cover.push(Cube::anonymous(
        &[Some(true), Some(true), Some(true)],
        &[true],
        CubeType::F,
    )); // 111 -> 1
    println!("  111 -> 1");

    println!("\nBefore minimize: {} cubes", cover.num_cubes());

    // Minimize
    cover = cover.minimize()?;

    println!("After minimize: {} cubes", cover.num_cubes());

    // Print the minimized cubes
    println!("\nMinimized cubes:");
    for (i, cube) in cover.cubes().enumerate() {
        let inputs: Vec<Option<bool>> = cube.inputs().iter().collect();
        let outputs: Vec<Option<bool>> = cube.outputs().iter().collect();
        print!("  Cube {}: ", i);
        for inp in &inputs {
            match inp {
                Some(true) => print!("1"),
                Some(false) => print!("0"),
                None => print!("-"),
            }
        }
        print!(" -> ");
        for out in &outputs {
            match out {
                Some(true) => print!("1"),
                Some(false) => print!("0"),
                None => print!("-"),
            }
        }
        println!();
    }

    if cover.num_cubes() == 1 {
        println!("\n✓ SUCCESS: Minimized to 1 cube as expected!");
    } else {
        println!("\n✗ FAIL: Expected 1 cube, got {}", cover.num_cubes());
    }

    Ok(())
}
