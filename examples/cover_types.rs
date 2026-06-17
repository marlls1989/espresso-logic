//! Example: Using different cover types with Cover

use espresso_logic::{Cover, CoverType, Cube, CubeType, Minimizable};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cover Type Examples ===\n");

    // FD type (default) - ON-set + Don't-care
    println!("1. FD Type (ON-set + Don't-care):");
    let mut fd_cover = Cover::<(), ()>::anonymous(CoverType::FD);
    fd_cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    )); // 01 -> 1 (F cube)
    fd_cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::D,
    )); // 10 -> - (D cube)
    println!("   Before minimize: {} cubes", fd_cover.num_cubes());
    fd_cover = fd_cover.minimize()?;
    println!("   After minimize:  {} cubes\n", fd_cover.num_cubes());

    // F type - ON-set only
    println!("2. F Type (ON-set only):");
    let mut f_cover = Cover::<(), ()>::anonymous(CoverType::F);
    f_cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    )); // 01 -> 1 (F cube)
    f_cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    )); // 10 -> 1 (F cube)
        // (An F-type cover carries only F cubes; D/R cubes would be excluded from its count.)
    println!("   Before minimize: {} cubes", f_cover.num_cubes());
    f_cover = f_cover.minimize()?;
    println!("   After minimize:  {} cubes\n", f_cover.num_cubes());

    // FR type - ON-set + OFF-set
    println!("3. FR Type (ON-set + OFF-set):");
    let mut fr_cover = Cover::<(), ()>::anonymous(CoverType::FR);
    fr_cover.push(Cube::anonymous(
        &[Some(false), Some(false)],
        &[true],
        CubeType::R,
    )); // 00 -> 0 (R cube)
    fr_cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    )); // 01 -> 1 (F cube)
    fr_cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    )); // 10 -> 1 (F cube)
    fr_cover.push(Cube::anonymous(
        &[Some(true), Some(true)],
        &[true],
        CubeType::R,
    )); // 11 -> 0 (R cube)
    println!("   Before minimize: {} cubes", fr_cover.num_cubes());
    fr_cover = fr_cover.minimize()?;
    println!("   After minimize:  {} cubes\n", fr_cover.num_cubes());

    // FDR type - ON-set + Don't-care + OFF-set
    println!("4. FDR Type (ON-set + Don't-care + OFF-set):");
    let mut fdr_cover = Cover::<(), ()>::anonymous(CoverType::FDR);
    fdr_cover.push(Cube::anonymous(
        &[Some(false), Some(false)],
        &[true],
        CubeType::R,
    )); // 00 -> 0 (R cube)
    fdr_cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    )); // 01 -> 1 (F cube)
    fdr_cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::D,
    )); // 10 -> - (D cube)
    fdr_cover.push(Cube::anonymous(
        &[Some(true), Some(true)],
        &[true],
        CubeType::R,
    )); // 11 -> 0 (R cube)
    println!("   Before minimize: {} cubes", fdr_cover.num_cubes());
    fdr_cover = fdr_cover.minimize()?;
    println!("   After minimize:  {} cubes\n", fdr_cover.num_cubes());

    Ok(())
}
