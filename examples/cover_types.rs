//! Example: Using different cover types with CoverBuilder

use espresso_logic::{Cover, CoverBuilder, FType, FRType, FDRType};

fn main() -> std::io::Result<()> {
    println!("=== Cover Type Examples ===\n");

    // FD type (default) - ON-set + Don't-care
    println!("1. FD Type (ON-set + Don't-care) - default:");
    let mut fd_cover = CoverBuilder::<2, 1>::new();
    fd_cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);   // 01 -> 1 (F cube)
    fd_cover.add_cube(&[Some(true), Some(false)], &[None]);         // 10 -> - (D cube)
    println!("   Before minimize: {} cubes", fd_cover.num_cubes());
    fd_cover.minimize()?;
    println!("   After minimize:  {} cubes\n", fd_cover.num_cubes());

    // F type - ON-set only
    println!("2. F Type (ON-set only):");
    let mut f_cover = CoverBuilder::<2, 1, FType>::new();
    f_cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);    // 01 -> 1 (F cube)
    f_cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);    // 10 -> 1 (F cube)
    f_cover.add_cube(&[Some(true), Some(true)], &[None]);           // 11 -> - (ignored, F-type doesn't support D)
    println!("   Before minimize: {} cubes", f_cover.num_cubes());
    f_cover.minimize()?;
    println!("   After minimize:  {} cubes\n", f_cover.num_cubes());

    // FR type - ON-set + OFF-set
    println!("3. FR Type (ON-set + OFF-set):");
    let mut fr_cover = CoverBuilder::<2, 1, FRType>::new();
    fr_cover.add_cube(&[Some(false), Some(false)], &[Some(false)]); // 00 -> 0 (R cube)
    fr_cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);   // 01 -> 1 (F cube)
    fr_cover.add_cube(&[Some(true), Some(false)], &[Some(true)]);   // 10 -> 1 (F cube)
    fr_cover.add_cube(&[Some(true), Some(true)], &[Some(false)]);   // 11 -> 0 (R cube)
    println!("   Before minimize: {} cubes", fr_cover.num_cubes());
    fr_cover.minimize()?;
    println!("   After minimize:  {} cubes\n", fr_cover.num_cubes());

    // FDR type - ON-set + Don't-care + OFF-set
    println!("4. FDR Type (ON-set + Don't-care + OFF-set):");
    let mut fdr_cover = CoverBuilder::<2, 1, FDRType>::new();
    fdr_cover.add_cube(&[Some(false), Some(false)], &[Some(false)]); // 00 -> 0 (R cube)
    fdr_cover.add_cube(&[Some(false), Some(true)], &[Some(true)]);   // 01 -> 1 (F cube)
    fdr_cover.add_cube(&[Some(true), Some(false)], &[None]);         // 10 -> - (D cube)
    fdr_cover.add_cube(&[Some(true), Some(true)], &[Some(false)]);   // 11 -> 0 (R cube)
    println!("   Before minimize: {} cubes", fdr_cover.num_cubes());
    fdr_cover.minimize()?;
    println!("   After minimize:  {} cubes\n", fdr_cover.num_cubes());

    Ok(())
}

