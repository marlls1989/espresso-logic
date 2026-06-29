//! Tests for the cover module

use super::pla::{PLAWriter, PlaCover};
use super::*;
use crate::Symbol;
use std::sync::Arc;

#[test]
fn test_cover_creation() {
    let cover: Cover<Symbol, Symbol> = Cover::new(CoverType::F);
    assert_eq!(cover.num_inputs(), 0);
    assert_eq!(cover.num_outputs(), 0);
    assert_eq!(cover.num_cubes(), 0);
}

#[test]
fn test_cover_with_labels() {
    let cover: Cover<Symbol, Symbol> = Cover::with_labels(CoverType::F, &["a", "b", "c"], &["out"]);
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");
    assert_eq!(cover.input_labels()[2].as_ref(), "c");
    assert_eq!(cover.output_labels()[0].as_ref(), "out");
}

#[test]
fn test_push() {
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.num_cubes(), 1);
}

#[test]
fn test_from_cubes_matches_push() {
    // Cubes of differing widths: from_cubes grows to the widest, padding don't-care/unasserted.
    let cubes = [
        Cube::anonymous(&[Some(false), Some(true)], &[true], CubeType::F),
        Cube::anonymous(&[Some(true)], &[true, false], CubeType::F),
    ];
    let built = Cover::from_cubes(CoverType::F, cubes.iter().cloned());

    let mut pushed = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    for cube in cubes.iter().cloned() {
        pushed.push(cube);
    }

    assert_eq!(built.num_inputs(), 2);
    assert_eq!(built.num_outputs(), 2);
    assert_eq!(built.num_inputs(), pushed.num_inputs());
    assert_eq!(built.num_outputs(), pushed.num_outputs());
    // Same cube payloads in the same order.
    assert_eq!(io_rows(&built), io_rows(&pushed));
}

/// One cube's `(inputs, output-membership)` values.
type CubeRow = (Vec<Option<bool>>, Vec<bool>);

/// `(inputs, outputs)` rows of every cube, in order.
fn io_rows<I, O>(c: &Cover<I, O>) -> Vec<CubeRow> {
    c.cubes()
        .map(|cube| {
            (
                cube.inputs().iter().collect(),
                cube.outputs().iter().collect(),
            )
        })
        .collect()
}

#[test]
fn cube_labeled_builds_expected_and_matches_with_labels() {
    let cube = Cube::<Symbol, Symbol>::labeled(
        &[(Symbol::new("a"), Some(true)), (Symbol::new("b"), None)],
        &[(Symbol::new("f"), true)],
        CubeType::F,
    )
    .unwrap();
    assert_eq!(cube.inputs().num_vars(), 2);
    assert_eq!(cube.outputs().num_vars(), 1);
    assert_eq!(cube.inputs().value_of("a"), Some(true));
    assert_eq!(cube.inputs().value_of("b"), None);
    assert_eq!(cube.cube_type(), CubeType::F);

    // The `&str` convenience builds an identical cube.
    let via_str = Cube::<Symbol, Symbol>::with_labels(
        &[("a", Some(true)), ("b", None)],
        &[("f", true)],
        CubeType::F,
    )
    .unwrap();
    assert_eq!(cube, via_str);
}

#[test]
fn cube_labeled_accepts_non_string_labels() {
    // Any `Label` works, not only strings — here `u32` column ids.
    let cube = Cube::<u32, u32>::labeled(
        &[(10, Some(true)), (20, Some(false))],
        &[(0, true)],
        CubeType::F,
    )
    .unwrap();
    assert_eq!(cube.inputs().value_of(&10u32), Some(true));
    assert_eq!(cube.inputs().value_of(&20u32), Some(false));
    assert_eq!(cube.outputs().num_vars(), 1);
}

#[test]
fn cube_rejects_duplicate_labels() {
    use super::DuplicateLabel;
    // Duplicate input name → Err(Input).
    assert_eq!(
        Cube::<Symbol, Symbol>::with_labels(
            &[("a", Some(true)), ("a", Some(false))],
            &[("f", true)],
            CubeType::F,
        ),
        Err(DuplicateLabel::Input { index: 1 })
    );
    // Duplicate output name → Err(Output).
    assert_eq!(
        Cube::<Symbol, Symbol>::with_labels(
            &[("a", Some(true))],
            &[("f", true), ("f", false)],
            CubeType::F,
        ),
        Err(DuplicateLabel::Output { index: 1 })
    );
    // Distinct labels are fine.
    assert!(Cube::<Symbol, Symbol>::with_labels(
        &[("a", Some(true)), ("b", Some(false))],
        &[("f", true)],
        CubeType::F,
    )
    .is_ok());
}

#[test]
fn push_aligns_labeled_cubes_by_name() {
    let mut cover: Cover<Symbol, Symbol> = Cover::new(CoverType::F);
    cover.push(
        Cube::with_labels(
            &[("a", Some(true)), ("b", Some(false))],
            &[("f", true)],
            CubeType::F,
        )
        .unwrap(),
    );
    // Same assignment, labels listed in the OPPOSITE order: must align by name, not position.
    cover.push(
        Cube::with_labels(
            &[("b", Some(false)), ("a", Some(true))],
            &[("f", true)],
            CubeType::F,
        )
        .unwrap(),
    );

    assert_eq!(cover.num_inputs(), 2); // aligned by name, not duplicated into 4 columns
    assert_eq!(cover.num_cubes(), 2);
    let f = cover
        .output_labels()
        .iter()
        .position(|l| l.as_ref() == "f")
        .unwrap();
    for cube in cover.cubes() {
        assert_eq!(cube.inputs().value_of("a"), Some(true));
        assert_eq!(cube.inputs().value_of("b"), Some(false));
        assert!(cube.outputs().value_at(f));
    }
}

#[test]
fn push_widens_labeled_cover_by_new_name() {
    let mut cover: Cover<Symbol, Symbol> = Cover::new(CoverType::F);
    cover.push(Cube::with_labels(&[("a", Some(true))], &[("f", true)], CubeType::F).unwrap());
    assert_eq!(cover.num_inputs(), 1);

    // A second cube introduces `b`: the cover widens by name, and the first cube gets `b` = don't-care.
    cover.push(
        Cube::with_labels(
            &[("a", Some(false)), ("b", Some(true))],
            &[("f", true)],
            CubeType::F,
        )
        .unwrap(),
    );
    assert_eq!(cover.num_inputs(), 2);

    let rows: Vec<_> = cover.cubes().collect();
    assert_eq!(rows[0].inputs().value_of("a"), Some(true));
    assert_eq!(rows[0].inputs().value_of("b"), None); // widened → don't-care
    assert_eq!(rows[1].inputs().value_of("a"), Some(false));
    assert_eq!(rows[1].inputs().value_of("b"), Some(true));
}

#[test]
fn push_widens_outputs_and_remaps_by_name() {
    // Exercises the OUTPUT widening path: the second cube introduces a new output identity while the
    // cover already holds a cube, so the existing cube is re-homed (self_out_map) and the new cube's
    // output lands on the fresh column (the on-demand identity_position map).
    let mut cover: Cover<Symbol, Symbol> = Cover::new(CoverType::F);
    cover.push(Cube::with_labels(&[("a", Some(true))], &[("f", true)], CubeType::F).unwrap());
    cover.push(Cube::with_labels(&[("a", Some(false))], &[("g", true)], CubeType::F).unwrap());

    assert_eq!(cover.num_outputs(), 2);
    let col = |name: &str| {
        cover
            .output_labels()
            .iter()
            .position(|l| l.as_ref() == name)
            .unwrap()
    };
    let (f, g) = (col("f"), col("g"));
    let rows: Vec<_> = cover.cubes().collect();
    // Cube 0 asserts only f (g was added as an unasserted column for it).
    assert!(rows[0].outputs().value_at(f));
    assert!(!rows[0].outputs().value_at(g));
    // Cube 1 asserts only g.
    assert!(!rows[1].outputs().value_at(f));
    assert!(rows[1].outputs().value_at(g));
}

#[test]
fn push_rehomes_non_f_cube_through_widening() {
    // A D and an R cube re-homed through input widening: cube_type is preserved and the earlier cube
    // gains a don't-care in the new input column.
    let mut cover: Cover<Symbol, Symbol> = Cover::new(CoverType::FDR);
    cover.push(Cube::with_labels(&[("a", Some(true))], &[("f", true)], CubeType::F).unwrap());
    cover.push(
        Cube::with_labels(
            &[("a", Some(false)), ("b", Some(true))],
            &[("f", true)],
            CubeType::D,
        )
        .unwrap(),
    );
    cover.push(
        Cube::with_labels(
            &[("a", Some(true)), ("b", Some(false))],
            &[("f", true)],
            CubeType::R,
        )
        .unwrap(),
    );

    assert_eq!(cover.num_inputs(), 2);
    let rows: Vec<_> = cover.cubes().collect();
    assert_eq!(rows[0].cube_type(), CubeType::F);
    assert_eq!(rows[1].cube_type(), CubeType::D);
    assert_eq!(rows[2].cube_type(), CubeType::R);
    // The first (F) cube was re-homed: a=Some(true), b=don't-care.
    assert_eq!(rows[0].inputs().value_of("a"), Some(true));
    assert_eq!(rows[0].inputs().value_of("b"), None);
    // All three share the single output column f.
    let f = cover
        .output_labels()
        .iter()
        .position(|l| l.as_ref() == "f")
        .unwrap();
    assert!(rows.iter().all(|c| c.outputs().value_at(f)));
}

#[test]
fn from_cubes_labeled_unions_headers_by_name() {
    let cover: Cover<Symbol, Symbol> = Cover::from_cubes(
        CoverType::F,
        [
            Cube::with_labels(
                &[("a", Some(true)), ("b", Some(false))],
                &[("f", true)],
                CubeType::F,
            )
            .unwrap(),
            Cube::with_labels(
                &[("b", Some(true)), ("c", Some(true))],
                &[("g", true)],
                CubeType::F,
            )
            .unwrap(),
        ],
    );
    assert_eq!(cover.num_inputs(), 3); // a, b, c unioned by name
    assert_eq!(cover.num_outputs(), 2); // f, g
    let labels: Vec<&str> = cover.input_labels().iter().map(|l| l.as_ref()).collect();
    assert_eq!(labels, ["a", "b", "c"]);
    let rows: Vec<_> = cover.cubes().collect();
    // Cube 0 lacks `c`, cube 1 lacks `a` → each don't-care where absent.
    assert_eq!(rows[0].inputs().value_of("a"), Some(true));
    assert_eq!(rows[0].inputs().value_of("c"), None);
    assert_eq!(rows[1].inputs().value_of("a"), None);
    assert_eq!(rows[1].inputs().value_of("c"), Some(true));
    // Output columns are remapped by name: cube 0 ⇒ f only, cube 1 ⇒ g only.
    let col = |name: &str| {
        cover
            .output_labels()
            .iter()
            .position(|l| l.as_ref() == name)
            .unwrap()
    };
    let (f, g) = (col("f"), col("g"));
    assert!(rows[0].outputs().value_at(f) && !rows[0].outputs().value_at(g));
    assert!(!rows[1].outputs().value_at(f) && rows[1].outputs().value_at(g));
}

#[test]
fn merge_same_headers_appends_without_widening() {
    // `other`'s identities are all present in `self` → merge's no-widening fast path: `self` keeps its
    // headers/cubes and `other`'s cube is appended, aligned by name.
    let mut a: Cover<Symbol, Symbol> = Cover::new(CoverType::F);
    a.push(
        Cube::with_labels(
            &[("x", Some(true)), ("y", None)],
            &[("o", true)],
            CubeType::F,
        )
        .unwrap(),
    );
    let mut b: Cover<Symbol, Symbol> = Cover::new(CoverType::F);
    // Labels in the opposite order, same header set.
    b.push(
        Cube::with_labels(
            &[("y", Some(false)), ("x", None)],
            &[("o", true)],
            CubeType::F,
        )
        .unwrap(),
    );

    a.merge(&b);
    assert_eq!(a.num_inputs(), 2);
    assert_eq!(a.num_outputs(), 1);
    assert_eq!(a.num_cubes(), 2);
    let rows: Vec<_> = a.cubes().collect();
    assert_eq!(rows[0].inputs().value_of("x"), Some(true));
    assert_eq!(rows[0].inputs().value_of("y"), None);
    assert_eq!(rows[1].inputs().value_of("x"), None);
    assert_eq!(rows[1].inputs().value_of("y"), Some(false));
    let o = a
        .output_labels()
        .iter()
        .position(|l| l.as_ref() == "o")
        .unwrap();
    assert!(rows.iter().all(|c| c.outputs().value_at(o)));
}

#[test]
fn merge_widening_rebuilds_by_identity() {
    // `other` introduces a new input (`z`) and output (`p`) → merge's widening path: `self`'s cube is
    // re-homed (new columns don't-care/unasserted) and `other`'s cube lands on the new columns.
    let mut a: Cover<Symbol, Symbol> = Cover::new(CoverType::F);
    a.push(Cube::with_labels(&[("x", Some(true))], &[("o", true)], CubeType::F).unwrap());
    let mut b: Cover<Symbol, Symbol> = Cover::new(CoverType::F);
    b.push(Cube::with_labels(&[("z", Some(false))], &[("p", true)], CubeType::F).unwrap());

    a.merge(&b);
    assert_eq!(a.num_inputs(), 2); // x, z
    assert_eq!(a.num_outputs(), 2); // o, p
    let rows: Vec<_> = a.cubes().collect();
    assert_eq!(rows[0].inputs().value_of("x"), Some(true));
    assert_eq!(rows[0].inputs().value_of("z"), None); // re-homed: new input don't-care
    assert_eq!(rows[1].inputs().value_of("z"), Some(false));
    assert_eq!(rows[1].inputs().value_of("x"), None);
    let col = |name: &str| {
        a.output_labels()
            .iter()
            .position(|l| l.as_ref() == name)
            .unwrap()
    };
    let (o, p) = (col("o"), col("p"));
    assert!(rows[0].outputs().value_at(o) && !rows[0].outputs().value_at(p));
    assert!(!rows[1].outputs().value_at(o) && rows[1].outputs().value_at(p));
}

#[test]
fn test_minimize() {
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));
    cover = cover.minimize().unwrap();
    // XOR cannot be minimized
    assert_eq!(cover.num_cubes(), 2);
}

// ===== Dynamic Growth Tests =====

#[test]
fn test_dynamic_growth_inputs_only() {
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);

    // Start with 2 inputs
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);

    // Grow to 5 inputs
    cover.push(Cube::anonymous(
        &[Some(true), None, Some(false), None, Some(true)],
        &[true],
        CubeType::F,
    ));
    assert_eq!(cover.num_inputs(), 5);
    assert_eq!(cover.num_outputs(), 1);

    // Verify all cubes have consistent dimensions
    for cube in cover.cubes() {
        assert_eq!(cube.inputs().num_vars(), 5);
        assert_eq!(cube.outputs().num_vars(), 1);
    }
}

#[test]
fn test_dynamic_growth_outputs_only() {
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);

    // Start with 1 output
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);

    // Grow to 3 outputs
    cover.push(Cube::anonymous(
        &[Some(true), None],
        &[true, false, true],
        CubeType::F,
    ));
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 3);

    // Verify all cubes have consistent dimensions
    for cube in cover.cubes() {
        assert_eq!(cube.inputs().num_vars(), 2);
        assert_eq!(cube.outputs().num_vars(), 3);
    }
}

#[test]
fn test_dynamic_growth_both_dimensions() {
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);

    // Start small
    cover.push(Cube::anonymous(&[Some(true)], &[true], CubeType::F));
    assert_eq!(cover.num_inputs(), 1);
    assert_eq!(cover.num_outputs(), 1);

    // Grow both dimensions
    cover.push(Cube::anonymous(
        &[Some(true), Some(false), None],
        &[true, false],
        CubeType::F,
    ));
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.num_outputs(), 2);

    // Add another with even more dimensions
    cover.push(Cube::anonymous(
        &[Some(true), Some(false), None, Some(true)],
        &[true, false, true],
        CubeType::F,
    ));
    assert_eq!(cover.num_inputs(), 4);
    assert_eq!(cover.num_outputs(), 3);

    // All cubes should have been padded
    assert_eq!(cover.num_cubes(), 3);
    for cube in cover.cubes() {
        assert_eq!(cube.inputs().num_vars(), 4);
        assert_eq!(cube.outputs().num_vars(), 3);
    }
}

#[test]
fn test_dynamic_growth_preserves_existing_cubes() {
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);

    // Add first cube
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));

    // Get the first cube's data before growth
    let first_cube_inputs: Vec<_> = cover
        .cubes()
        .next()
        .unwrap()
        .inputs()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(first_cube_inputs, vec![Some(true), Some(false)]);

    // Grow dimensions
    cover.push(Cube::anonymous(
        &[Some(true), Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));

    // First cube should be padded with None
    let first_cube_after: Vec<_> = cover
        .cubes()
        .next()
        .unwrap()
        .inputs()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(first_cube_after, vec![Some(true), Some(false), None]);
}

// ===== Auto-Generated Label Tests =====

// ===== Expression Addition Tests =====

#[test]
fn test_add_expr_basic() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let expr = a.and(&b);

    cover.add_expr(&expr, "output").unwrap();

    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_outputs(), 1);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");
    assert_eq!(cover.output_labels()[0].as_ref(), "output");
    assert!(cover.num_cubes() > 0);
}

#[test]
fn test_add_expr_variable_matching() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let c = crate::BoolExpr::variable("c");

    // Add first expression with variables a and b
    cover.add_expr(&a.and(&b), "out1").unwrap();
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");

    // Add second expression with variables b and c (b should match, c appended)
    cover.add_expr(&b.and(&c), "out2").unwrap();
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");
    assert_eq!(cover.input_labels()[2].as_ref(), "c");

    assert_eq!(cover.num_outputs(), 2);
}

#[test]
fn test_add_expr_duplicate_output_error() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    // Add first expression
    cover.add_expr(&a, "result").unwrap();

    // Try to add another expression with same output name - should fail
    let result = cover.add_expr(&b, "result");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn test_add_expr_to_different_cover_types() {
    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    // F type
    let mut f_cover = Cover::new(CoverType::F);
    f_cover.add_expr(&a.and(&b), "out").unwrap();
    assert_eq!(f_cover.cover_type(), CoverType::F);

    // FD type
    let mut fd_cover = Cover::new(CoverType::FD);
    fd_cover.add_expr(&a.or(&b), "out").unwrap();
    assert_eq!(fd_cover.cover_type(), CoverType::FD);

    // FR type
    let mut fr_cover = Cover::new(CoverType::FR);
    fr_cover.add_expr(&a, "out").unwrap();
    assert_eq!(fr_cover.cover_type(), CoverType::FR);

    // FDR type
    let mut fdr_cover = Cover::new(CoverType::FDR);
    fdr_cover.add_expr(&a.not(), "out").unwrap();
    assert_eq!(fdr_cover.cover_type(), CoverType::FDR);
}

#[test]
fn test_add_expr_multiple_outputs() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let c = crate::BoolExpr::variable("c");

    // Add three different expressions
    cover.add_expr(&a.and(&b), "and_result").unwrap();
    cover.add_expr(&a.or(&c), "or_result").unwrap();
    cover.add_expr(&b.not(), "not_result").unwrap();

    assert_eq!(cover.num_outputs(), 3);
    assert_eq!(cover.output_labels()[0].as_ref(), "and_result");
    assert_eq!(cover.output_labels()[1].as_ref(), "or_result");
    assert_eq!(cover.output_labels()[2].as_ref(), "not_result");

    // All three variables should be present
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "b");
    assert_eq!(cover.input_labels()[2].as_ref(), "c");
}

#[test]
fn test_add_expr_variable_ordering_preserved() {
    let mut cover = Cover::new(CoverType::F);

    let z = crate::BoolExpr::variable("z");
    let a = crate::BoolExpr::variable("a");
    let m = crate::BoolExpr::variable("m");

    // Add expression with variables in non-alphabetical order
    // Variables in BoolExpr are sorted alphabetically internally
    cover.add_expr(&z.and(&a).and(&m), "out").unwrap();

    // Variables should be in alphabetical order (a, m, z)
    assert_eq!(cover.num_inputs(), 3);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
    assert_eq!(cover.input_labels()[1].as_ref(), "m");
    assert_eq!(cover.input_labels()[2].as_ref(), "z");
}

// ===== Expression Conversion Tests =====

#[test]
fn test_to_expr_basic() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    cover.add_expr(&a.and(&b), "result").unwrap();

    let retrieved = cover.to_expr("result").unwrap();

    // Should be able to collect variables
    let vars = retrieved.collect_variables();
    assert_eq!(vars.len(), 2);
    assert!(vars.contains("a"));
    assert!(vars.contains("b"));
}

#[test]
fn to_expr_and_from_pla_string_accept_owned_string() {
    // `Cover::to_expr` and `PlaCover::from_pla_string` take any `AsRef<str>`, not only `&str` — an
    // owned `String` behaves identically to the `&str` form (no string type is privileged).
    let mut cover = Cover::new(CoverType::F);
    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    cover.add_expr(&a.and(&b), "result").unwrap();
    let from_string = cover.to_expr(String::from("result")).unwrap();
    let from_str = cover.to_expr("result").unwrap();
    assert!(from_string.equivalent_to(&from_str));

    let pla = ".i 2\n.o 1\n.p 1\n01 1\n.e\n";
    let from_string = PlaCover::<Symbol>::from_pla_string(String::from(pla)).unwrap();
    let from_str = PlaCover::<Symbol>::from_pla_string(pla).unwrap();
    assert_eq!(from_string.num_inputs(), from_str.num_inputs());
    assert_eq!(from_string.num_outputs(), from_str.num_outputs());
    assert_eq!(from_string.num_cubes(), from_str.num_cubes());
}

#[test]
fn test_to_expr_by_index() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");

    cover.add_expr(&a, "out0").unwrap();
    cover.add_expr(&a.not(), "out1").unwrap();

    let expr0 = cover.to_expr_by_index(0).unwrap();
    let expr1 = cover.to_expr_by_index(1).unwrap();

    assert_eq!(expr0.collect_variables().len(), 1);
    assert_eq!(expr1.collect_variables().len(), 1);
}

#[test]
fn test_to_expr_nonexistent() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    cover.add_expr(&a, "exists").unwrap();

    // Try to get non-existent output
    let result = cover.to_expr("doesnt_exist");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_to_expr_index_out_of_bounds() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    cover.add_expr(&a, "out").unwrap();

    // Try to get out of bounds index
    let result = cover.to_expr_by_index(1);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("out of bounds"));
}

#[test]
fn test_to_exprs_iterator() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let c = crate::BoolExpr::variable("c");

    cover.add_expr(&a, "out1").unwrap();
    cover.add_expr(&b, "out2").unwrap();
    cover.add_expr(&c, "out3").unwrap();

    let exprs: Vec<_> = cover.to_exprs().collect();
    assert_eq!(exprs.len(), 3);

    assert_eq!(exprs[0].0.as_ref(), "out1");
    assert_eq!(exprs[1].0.as_ref(), "out2");
    assert_eq!(exprs[2].0.as_ref(), "out3");

    // Each expression should have one variable
    assert_eq!(exprs[0].1.collect_variables().len(), 1);
    assert_eq!(exprs[1].1.collect_variables().len(), 1);
    assert_eq!(exprs[2].1.collect_variables().len(), 1);
}

#[test]
fn to_exprs_works_for_any_string_input_label() {
    use std::sync::Arc;

    // Build a named cover, then relabel both sides to a *different* string type (Arc<str>).
    let mut cover = Cover::new(CoverType::F);
    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    cover.add_expr(&a.and(&b), "out").unwrap();

    let in_syms = Symbols::new(
        cover
            .input_labels()
            .iter()
            .map(|s| Arc::<str>::from(s.as_ref()))
            .collect::<Vec<_>>()
            .into(),
    );
    let out_syms = Symbols::new(vec![Arc::<str>::from("out")].into());
    let arc_cover: Cover<Arc<str>, Arc<str>> = cover.relabel(in_syms, out_syms).unwrap();

    // to_expr_by_index / to_exprs / to_expr all work on an `Arc<str>`-labelled cover.
    assert_eq!(
        arc_cover
            .to_expr_by_index(0)
            .unwrap()
            .collect_variables()
            .len(),
        2
    );
    let pairs: Vec<_> = arc_cover.to_exprs().collect();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].0.as_ref(), "out"); // (&O, BoolExpr) — output label borrowed
    assert_eq!(
        arc_cover.to_expr("out").unwrap().collect_variables().len(),
        2
    );
}

#[test]
fn test_to_exprs_after_minimization() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let c = crate::BoolExpr::variable("c");

    // Add redundant expression: a*b + a*b*c
    let redundant = a.and(&b).or(&a.and(&b).and(&c));
    cover.add_expr(&redundant, "out").unwrap();

    let cubes_before = cover.num_cubes();
    cover = cover.minimize().unwrap();
    let cubes_after = cover.num_cubes();

    // Should minimize
    assert!(cubes_after <= cubes_before);

    // Should still be able to convert to expression
    let minimized = cover.to_expr("out").unwrap();
    let vars = minimized.collect_variables();
    assert!(vars.len() >= 2); // At least a and b
}

// ===== Cover Type Tests =====

#[test]
fn test_f_type_cover() {
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);

    // F type only accepts Some(true) for outputs
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));
    assert_eq!(cover.num_cubes(), 1);

    // R and D cubes don't count toward an F-type cover's cube count.
    cover.push(Cube::anonymous(
        &[Some(true), Some(true)],
        &[true],
        CubeType::R,
    ));
    cover.push(Cube::anonymous(
        &[Some(false), Some(false)],
        &[true],
        CubeType::D,
    ));

    // Should still have only 1 cube (F type)
    assert_eq!(cover.num_cubes(), 1);
}

#[test]
fn test_fd_type_cover() {
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::FD);

    // FD type accepts Some(true) and None
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    )); // F cube
    cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::D,
    )); // D cube

    // For FD type, num_cubes() only counts F cubes
    assert_eq!(cover.num_cubes(), 1);

    // But internal cubes should have both
    assert_eq!(cover.cubes.len(), 2);
}

#[test]
fn test_fr_type_cover() {
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::FR);

    // FR type accepts Some(true) and Some(false)
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    )); // F cube
    cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::R,
    )); // R cube

    // For FR type, num_cubes() counts all cubes
    assert_eq!(cover.num_cubes(), 2);
}

#[test]
fn test_fdr_type_cover() {
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::FDR);

    // FDR type accepts all: Some(true), Some(false), None
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    )); // F cube
    cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::R,
    )); // R cube
    cover.push(Cube::anonymous(
        &[Some(true), Some(true)],
        &[true],
        CubeType::D,
    )); // D cube

    // For FDR type, num_cubes() counts all cubes
    assert_eq!(cover.num_cubes(), 3);

    // The writer must group cubes F -> D -> R (matching C's fprint_pla), regardless of the push
    // order above (F, R, D). Output char per set: F='1', D='2', R='0'.
    let pla = cover.to_pla_string(CoverType::FDR).unwrap();
    let body: Vec<&str> = pla.lines().filter(|l| !l.starts_with('.')).collect();
    assert_eq!(
        body,
        vec!["10 1", "11 2", "01 0"],
        "cubes must be emitted grouped F->D->R:\n{pla}"
    );
}

// ===== Mixed Operations Tests =====

#[test]
fn test_complex_expression_with_minimization() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let c = crate::BoolExpr::variable("c");

    // Consensus theorem: a*b + ~a*c + b*c (b*c is redundant)
    let expr = a.and(&b).or(&a.not().and(&c)).or(&b.and(&c));
    cover.add_expr(&expr, "consensus").unwrap();

    // BDD automatically optimizes during conversion, so we get 2 cubes directly
    // (b*c is recognized as redundant by the canonical BDD representation)
    assert_eq!(cover.num_cubes(), 2);

    cover = cover.minimize().unwrap();

    // Should still have 2 cubes after minimization
    assert_eq!(cover.num_cubes(), 2);

    // Should still be able to convert back
    let minimized = cover.to_expr("consensus").unwrap();
    assert_eq!(minimized.collect_variables().len(), 3);
}

#[test]
fn test_empty_cover_to_expr() {
    let cover: Cover<Symbol, Symbol> = Cover::new(CoverType::F);

    // Try to get expression from empty cover - should fail
    let result = cover.to_expr_by_index(0);
    assert!(result.is_err());
}

#[test]
fn test_expression_with_constants() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let t = crate::BoolExpr::constant(true);

    // Expression with constant: a * true = a
    let expr = a.and(&t);
    cover.add_expr(&expr, "out").unwrap();

    // Should have one variable
    assert_eq!(cover.num_inputs(), 1);
    assert_eq!(cover.input_labels()[0].as_ref(), "a");
}

#[test]
fn test_pla_roundtrip_with_expressions() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    cover.add_expr(&a.and(&b), "output").unwrap();

    // Convert to PLA string
    let pla_string = cover.to_pla_string(CoverType::F).unwrap();

    // Parse it back
    let cover2 = PlaCover::<Symbol>::from_pla_string(&pla_string).unwrap();

    // Should have same dimensions
    assert_eq!(cover2.num_inputs(), cover.num_inputs());
    assert_eq!(cover2.num_outputs(), cover.num_outputs());
    assert_eq!(cover2.num_cubes(), cover.num_cubes());

    // Labels should be preserved
    assert_eq!(cover2.input_labels()[0].as_ref(), "a");
    assert_eq!(cover2.input_labels()[1].as_ref(), "b");
    assert_eq!(cover2.output_labels()[0].as_ref(), "output");
}

#[test]
fn pla_cover_variant_tracks_label_sections() {
    // Which `.ilb`/`.ob` sections a file carries selects the PlaCover variant, and the writer
    // reproduces exactly that set (label-presence is type-level, never a runtime flag).
    let cubes = "\n01 1\n10 1\n.e\n";
    let both = format!(".i 2\n.o 1\n.ilb a b\n.ob f{cubes}");
    let inputs_only = format!(".i 2\n.o 1\n.ilb a b{cubes}");
    let outputs_only = format!(".i 2\n.o 1\n.ob f{cubes}");
    let neither = format!(".i 2\n.o 1{cubes}");

    let read = |s: &str| PlaCover::<Symbol>::from_pla_string(s).unwrap();
    assert!(matches!(read(&both), PlaCover::InputsOutputsNamed(_)));
    assert!(matches!(read(&inputs_only), PlaCover::InputsNamed(_)));
    assert!(matches!(read(&outputs_only), PlaCover::OutputsNamed(_)));
    assert!(matches!(read(&neither), PlaCover::Positional(_)));

    // Round-trip reproduces the same section set.
    for src in [&both, &inputs_only, &outputs_only, &neither] {
        let out = read(src).to_pla_string(CoverType::F).unwrap();
        assert_eq!(out.contains(".ilb"), src.contains(".ilb"), "ilb: {src}");
        assert_eq!(out.contains(".ob"), src.contains(".ob"), "ob: {src}");
    }
}

#[test]
fn pla_cube_stream_chunks_by_char_count_like_c() {
    // C reads cube data as a continuous stream of significant characters (space/tab/`|`/newline all
    // insignificant) and takes exactly `ni + no` characters per cube — there are no cube separators.
    // We mirror that: extra characters spill into the next cube, and a trailing partial is ignored,
    // rather than erroring. Each case below is asserted equal to its canonical one-cube-per-line form.
    let parse = |s: &str| {
        PlaCover::<Symbol>::from_pla_string(s)
            .unwrap_or_else(|e| panic!("{s:?} should parse, got {e:?}"))
            .to_pla_string(CoverType::FD)
            .expect("serialise")
    };

    // 5 significant chars at width 3: one cube `011` (01|1), trailing `11` ignored.
    assert_eq!(
        parse(".i 2\n.o 1\n0111 1\n.e\n"),
        parse(".i 2\n.o 1\n01 1\n.e\n")
    );

    // 2 chars at width 6: no complete cube — an empty cover, not an error.
    assert_eq!(parse(".i 4\n.o 2\n01\n.e\n"), parse(".i 4\n.o 2\n.e\n"));

    // 7 chars across two lines at width 6: one cube `010111` (0101|11), trailing `1` ignored.
    assert_eq!(
        parse(".i 4\n.o 2\n0101\n111\n.e\n"),
        parse(".i 4\n.o 2\n0101 11\n.e\n")
    );

    // Two cubes packed onto one line (6 chars at width 3) read as two cubes.
    assert_eq!(
        parse(".i 2\n.o 1\n01 1 10 1\n.e\n"),
        parse(".i 2\n.o 1\n01 1\n10 1\n.e\n")
    );

    // A single cube split across lines (exactly ni + no wide) reads as one cube.
    assert_eq!(
        parse(".i 4\n.o 2\n0101\n11\n.e\n"),
        parse(".i 4\n.o 2\n0101 11\n.e\n")
    );

    // `.end` is accepted as a terminator alongside `.e`.
    assert!(PlaCover::<Symbol>::from_pla_string(".i 2\n.o 1\n01 1\n.end\n").is_ok());
}

#[test]
fn relabel_arity_mismatch_errors() {
    use super::ArityMismatch;

    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));

    // Two inputs in the cover, three labels supplied -> input arity mismatch.
    let err = cover
        .clone()
        .relabel(
            Symbols::new(vec![Symbol::from("a"), Symbol::from("b"), Symbol::from("c")].into()),
            Symbols::new(vec![Symbol::from("o")].into()),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        ArityMismatch::Inputs {
            expected: 2,
            actual: 3
        }
    ));

    // One output in the cover, two labels supplied -> output arity mismatch.
    let err = cover
        .relabel_outputs(Symbols::new(
            vec![Symbol::from("x"), Symbol::from("y")].into(),
        ))
        .unwrap_err();
    assert!(matches!(
        err,
        ArityMismatch::Outputs {
            expected: 1,
            actual: 2
        }
    ));
}

#[test]
fn cover_and_cube_equality_is_structural() {
    let build = || {
        let mut c = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
        c.push(Cube::anonymous(&[Some(true), None], &[true], CubeType::F));
        c
    };
    assert_eq!(build(), build());
    assert_eq!(
        build().cubes().next().unwrap(),
        build().cubes().next().unwrap()
    );

    let mut other = build();
    other.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));
    assert_ne!(build(), other);
}

#[test]
fn pla_cover_equality_distinguishes_variants() {
    let read = |s: &str| PlaCover::<Symbol>::from_pla_string(s).unwrap();
    let named = read(".i 2\n.o 1\n.ilb a b\n.ob f\n01 1\n.e\n");
    let named2 = read(".i 2\n.o 1\n.ilb a b\n.ob f\n01 1\n.e\n");
    let positional = read(".i 2\n.o 1\n01 1\n.e\n");

    // PlaCover has no Debug, so compare with `==`/`!=` directly (assert_eq! would need Debug).
    assert!(named == named2);
    assert!(named != positional); // same cubes, different variant -> not equal
}

#[test]
fn minterm_hash_agrees_with_eq() {
    use std::collections::HashSet;

    let minterm = |bits: &[Option<bool>]| {
        let mut c = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
        c.push(Cube::anonymous(bits, &[true], CubeType::F));
        let m = c.cubes().next().unwrap().inputs().clone();
        m
    };
    let a = minterm(&[Some(true), None, Some(false)]);
    let b = minterm(&[Some(true), None, Some(false)]);
    assert_eq!(a, b);

    // Minterm is a fully immutable value (no interior mutability), so it is a sound map key with no
    // `mutable_key_type` lint: equal minterms must collide in the same bucket (Hash/Eq contract).
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}

#[test]
fn minterm_hash_permutation_independent() {
    use std::collections::hash_map::RandomState;
    use std::collections::HashSet;
    use std::hash::BuildHasher;

    // Same assignment over the same variables, but two different header orders. Identity-based `Eq`
    // makes them equal; the `Hash` impl walks identity-sorted order, so they must hash equal too.
    let mk = |order: &[&str], vals: &[Option<bool>]| {
        let syms = Symbols::new(
            order
                .iter()
                .map(|s| Symbol::from(*s))
                .collect::<Vec<_>>()
                .into(),
        );
        Minterm::from_symbols(syms, vals.iter().copied())
    };
    let m1 = mk(&["a", "b", "c"], &[Some(true), None, Some(false)]);
    let m2 = mk(&["c", "a", "b"], &[Some(false), Some(true), None]);
    assert_eq!(m1, m2, "identity-aligned equality across permuted headers");

    let rs = RandomState::new();
    assert_eq!(
        rs.hash_one(&m1),
        rs.hash_one(&m2),
        "equal minterms must hash equal regardless of header order"
    );
    let mut set = HashSet::new();
    set.insert(m1);
    assert!(set.contains(&m2));
}

#[test]
fn value_of_by_name_wide() {
    // >32 variables so the packed values span a word boundary, with labels stored in reverse
    // (non-identity) order so `index_of`'s binary search over the identity-sorted order genuinely
    // differs from storage order.
    let n = 40usize;
    let labels: Vec<Symbol> = (0..n)
        .rev()
        .map(|i| Symbol::from(format!("v{i:02}").as_str()))
        .collect();
    let values: Vec<Option<bool>> = (0..n)
        .map(|i| match i % 3 {
            0 => Some(true),
            1 => Some(false),
            _ => None,
        })
        .collect();
    let m = Minterm::from_symbols(Symbols::new(labels.clone().into()), values.iter().copied());

    for (pos, label) in labels.iter().enumerate() {
        assert_eq!(m.value_of(label.as_ref()), values[pos], "value_of {label}");
        assert_eq!(
            m.value_of(label.as_ref()),
            m.value_at(pos),
            "value_of vs value_at for {label}"
        );
    }
    assert_eq!(m.value_of("not_a_var"), None);
}

#[test]
fn pla_cover_minimize_preserves_variant() {
    let cubes = "\n01 1\n10 1\n.e\n";
    let cases = [
        (format!(".i 2\n.o 1\n.ilb a b\n.ob f{cubes}"), true, true),
        (format!(".i 2\n.o 1\n.ilb a b{cubes}"), true, false),
        (format!(".i 2\n.o 1\n.ob f{cubes}"), false, true),
        (format!(".i 2\n.o 1{cubes}"), false, false),
    ];
    for (src, has_ilb, has_ob) in cases {
        let cover = PlaCover::<Symbol>::from_pla_string(&src).unwrap();
        let min = cover.minimize().unwrap();
        // Minimisation goes through every variant arm of `map_inner_cover!` and preserves which sides
        // are named.
        let variant_ok = match &min {
            PlaCover::InputsOutputsNamed(_) => has_ilb && has_ob,
            PlaCover::InputsNamed(_) => has_ilb && !has_ob,
            PlaCover::OutputsNamed(_) => !has_ilb && has_ob,
            PlaCover::Positional(_) => !has_ilb && !has_ob,
        };
        assert!(
            variant_ok,
            "variant not preserved for ilb={has_ilb} ob={has_ob}"
        );
        let out = min.to_pla_string(CoverType::F).unwrap();
        assert_eq!(out.contains(".ilb"), has_ilb, "ilb in {src}");
        assert_eq!(out.contains(".ob"), has_ob, "ob in {src}");
    }
}

#[test]
fn large_cover_builds_and_drops() {
    // A wide cover with many cubes builds and drops without issue. `Minterm`/`Cube` own flat
    // `Arc<[…]>` storage (no recursive ownership), so Drop is iterative by construction — this guards
    // that a future change to the ownership shape doesn't reintroduce deep recursive teardown.
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    for i in 0..5000u32 {
        cover.push(Cube::anonymous(
            &[Some(i & 1 == 0), None, Some(i & 2 == 0)],
            &[true],
            CubeType::F,
        ));
    }
    assert_eq!(cover.num_cubes(), 5000);
    drop(cover); // exercised explicitly; no stack overflow on teardown
}

#[test]
fn defaults_and_symbols_clone() {
    use std::sync::Arc;

    assert_eq!(CoverType::default(), CoverType::F);
    assert_eq!(CubeType::default(), CubeType::F);

    // Symbols is Clone with no `L: Clone` bound (it shares the Arc-backed label storage).
    let table = Symbols::<Symbol>::new(Arc::from([Symbol::new("a"), Symbol::new("b")]));
    let cloned: Symbols<Symbol> = (*table).clone();
    assert_eq!(cloned.arity(), 2);
    assert_eq!(*table, cloned);
}

#[test]
fn wide_minimise_crosses_word_boundary() {
    // 36 inputs (> 32 → multi-word C cube vectors). Two cubes differing only in input 0 merge to a
    // single cube with input 0 = don't-care — the only path exercising the multi-word FFI bit-packing
    // (`(bit >> 5) + 1`) under real minimisation.
    const N: usize = 36;
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    let a = vec![Some(true); N];
    let mut b = vec![Some(true); N];
    b[0] = Some(false);
    cover.push(Cube::anonymous(&a, &[true], CubeType::F));
    cover.push(Cube::anonymous(&b, &[true], CubeType::F));

    let min = cover.minimize().unwrap();
    assert_eq!(min.num_cubes(), 1);
    let cube = min.cubes().next().unwrap();
    let inputs: Vec<_> = cube.inputs().iter().collect();
    assert_eq!(inputs[0], None, "input 0 should become don't-care");
    assert!(
        inputs[1..].iter().all(|&v| v == Some(true)),
        "the other 35 inputs should stay 1"
    );
}

#[test]
fn multi_output_minimise_round_trips_across_word_boundary() {
    // 70 outputs, so output bit `i` straddles both the C 32-bit and the OutputSet 64-bit word boundary.
    // Two cubes assert the *same* 70-bit output pattern (with bits on both sides of 64) and differ in
    // one input, so they merge to a single cube that must carry that exact pattern back from C. This is
    // the path that would silently corrupt outputs if the output-region bit-blit shifted by the wrong
    // amount — and the only `cargo test` (no C oracle) that exercises a multi-word output round-trip.
    const NO: usize = 70;
    let asserted = [0usize, 1, 63, 64, 65, 69]; // span the 64-bit boundary
    let mut membership = vec![false; NO];
    for &i in &asserted {
        membership[i] = true;
    }

    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    let a = [Some(true), Some(true), Some(true), Some(true)];
    let mut b = a;
    b[0] = Some(false);
    cover.push(Cube::anonymous(&a, &membership, CubeType::F));
    cover.push(Cube::anonymous(&b, &membership, CubeType::F));

    let min = cover.minimize().unwrap();
    assert!(min.num_cubes() >= 1);
    // Whether or not the two cubes merged, every returned cube must carry the exact output pattern —
    // bit-for-bit, including the outputs past index 64.
    for cube in min.cubes() {
        assert_eq!(
            cube.outputs().iter().collect::<Vec<bool>>(),
            membership,
            "output membership must round-trip across the u64 word boundary"
        );
        assert!(cube.outputs().value_at(64) && cube.outputs().value_at(69));
        assert!(!cube.outputs().value_at(62) && !cube.outputs().value_at(66));
    }
}

#[test]
fn multi_output_minimise_keeps_each_output_independent() {
    // Three outputs, each asserted by a distinct fully-specified input minterm. Distinct inputs with
    // distinct outputs cannot merge or expand under F-type minimisation, so each (input, output) pair
    // must survive verbatim — catching any per-output `(0..no)` indexing slip without a C oracle.
    let expected: [([Option<bool>; 2], [bool; 3]); 3] = [
        ([Some(true), Some(false)], [true, false, false]), // output 0 only
        ([Some(false), Some(true)], [false, true, false]), // output 1 only
        ([Some(true), Some(true)], [false, false, true]),  // output 2 only
    ];

    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    for (ins, outs) in &expected {
        cover.push(Cube::anonymous(ins, outs, CubeType::F));
    }

    let min = cover.minimize().unwrap();
    let got: std::collections::HashSet<(Vec<Option<bool>>, Vec<bool>)> = min
        .cubes()
        .map(|c| (c.inputs().iter().collect(), c.outputs().iter().collect()))
        .collect();
    let want: std::collections::HashSet<(Vec<Option<bool>>, Vec<bool>)> = expected
        .iter()
        .map(|(i, o)| (i.to_vec(), o.to_vec()))
        .collect();
    assert_eq!(
        got, want,
        "each output's minterm must round-trip independently"
    );
}

#[test]
fn write_pla_surfaces_io_error() {
    use super::pla::PLAWriteError;
    use std::io::{self, Write};

    // A writer that always fails: write_pla must surface PLAWriteError::Io, not panic.
    struct FailingWriter;
    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "boom"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover.push(Cube::anonymous(&[Some(false), None], &[true], CubeType::F));
    let err = cover
        .write_pla(&mut FailingWriter, CoverType::F)
        .expect_err("writing to a failing writer should error");
    assert!(matches!(err, PLAWriteError::Io(_)));
}

#[test]
fn cover_minimize_exact_reduces_and_preserves() {
    // f(a,b,c) = 1 exactly when a == 0 (minterms 000,001,010,011). The unique exact
    // minimum is the single prime implicant `0--`.
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    for b in [false, true] {
        for c in [false, true] {
            cover.push(Cube::anonymous(
                &[Some(false), Some(b), Some(c)],
                &[true],
                CubeType::F,
            ));
        }
    }
    assert_eq!(cover.num_cubes(), 4);

    // Exact minimisation collapses the four minterms to one cube `0--`. This exercises the
    // distinct `esp.minimize_exact` path (not heuristic `minimize`); a regression that broke or
    // silently aliased it would change this result or fail to return.
    let exact = cover.minimize_exact().unwrap();
    assert_eq!(exact.num_cubes(), 1);
    let cube = exact.cubes().next().unwrap();
    // `0--` covers exactly {000,001,010,011} — i.e. logically equivalent to the input by construction.
    assert_eq!(
        cube.inputs().iter().collect::<Vec<_>>(),
        vec![Some(false), None, None]
    );
}

#[test]
fn pla_cover_minimize_exact_preserves_variant() {
    let src = ".i 3\n.o 1\n.ilb a b c\n.ob f\n000 1\n001 1\n010 1\n011 1\n.e\n";
    let cover = PlaCover::<Symbol>::from_pla_string(src).unwrap();
    let exact = cover.minimize_exact().unwrap();
    // Exact minimisation dispatches through the same variant arms as heuristic minimisation and
    // preserves which sides are named.
    match &exact {
        PlaCover::InputsOutputsNamed(c) => assert_eq!(c.num_cubes(), 1),
        _ => panic!("variant not preserved by exact minimisation"),
    }
}

#[test]
fn try_minimize_surfaces_instance_conflict() {
    use crate::error::MinimizationError;
    use crate::espresso::Espresso;

    // Hold a live low-level Espresso of dimensions (3,1) on this thread.
    let _held = Espresso::new(3, 1, &crate::EspressoConfig::default());

    // A cover of *different* dimensions (2,1) cannot create its instance while `_held` is alive.
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));

    // try_minimize returns the conflict as an error rather than panicking.
    let err = cover.try_minimize().unwrap_err();
    assert!(matches!(err, MinimizationError::Instance(_)));
    let err = cover.try_minimize_exact().unwrap_err();
    assert!(matches!(err, MinimizationError::Instance(_)));
}

#[test]
#[should_panic(expected = "instance conflict")]
fn minimize_panics_on_instance_conflict() {
    use crate::espresso::Espresso;

    let _held = Espresso::new(3, 1, &crate::EspressoConfig::default());
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));
    // The panicking entry point raises the same conflict loudly.
    let _ = cover.minimize();
}

#[test]
fn display_and_into_iter_surface() {
    use std::collections::HashSet;

    // Minterm Display is the bare 1/0/- row; Cube adds the output field; Cover joins cubes by line.
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover.push(Cube::anonymous(&[Some(false), None], &[true], CubeType::F));
    cover.push(Cube::anonymous(
        &[Some(true), Some(true)],
        &[true],
        CubeType::F,
    ));
    let cube = cover.cubes().next().unwrap();
    assert_eq!(cube.inputs().to_string(), "0-");
    assert_eq!(cube.to_string(), "0- 1");
    assert_eq!(format!("{cover}"), "0- 1\n11 1");

    // `for cube in &cover` mirrors cover.cubes().
    let by_ref: Vec<_> = (&cover).into_iter().collect();
    assert_eq!(by_ref.len(), 2);
    // `for value in &minterm` mirrors minterm.iter().
    let values: Vec<_> = cube.inputs().into_iter().collect();
    assert_eq!(values, vec![Some(false), None]);

    // PlaCover is Hash/Clone/Debug; equal covers (same variant) share a HashSet bucket.
    let pla =
        PlaCover::<Symbol>::from_pla_string(".i 2\n.o 1\n.ilb a b\n.ob f\n01 1\n.e\n").unwrap();
    let mut set = HashSet::new();
    set.insert(pla.clone());
    assert!(set.contains(&pla));
    assert!(format!("{pla:?}").contains("InputsOutputsNamed"));
}

#[test]
fn symbol_from_str() {
    use std::str::FromStr;
    assert_eq!(Symbol::from_str("xyz").unwrap(), Symbol::new("xyz"));
    assert_eq!("abc".parse::<Symbol>().unwrap().as_str(), "abc");
}

#[test]
fn malformed_pla_other_errors() {
    use super::pla::{PLAError, PLAReadError};

    let err = |s: &str| PlaCover::<Symbol>::from_pla_string(s).expect_err("should error");
    // .ilb declares fewer labels than .i inputs.
    assert!(matches!(
        err(".i 2\n.o 1\n.ilb a\n01 1\n.e\n"),
        PLAReadError::PLA(PLAError::LabelCountMismatch { .. })
    ));
    // A cube before any `.i`/`.o` is rejected: dimensions are required up front (as in C), never
    // inferred from the cube. A single-token line...
    assert!(matches!(
        err("0101\n.e\n"),
        PLAReadError::PLA(PLAError::MissingDimensions)
    ));
    // ...and a two-token line (which an earlier whitespace-inference path wrongly *accepted*) are
    // both rejected now.
    assert!(matches!(
        err("01 1\n.e\n"),
        PLAReadError::PLA(PLAError::MissingDimensions)
    ));
    // Invalid character in the input field.
    assert!(matches!(
        err(".i 2\n.o 1\n0z 1\n.e\n"),
        PLAReadError::PLA(PLAError::InvalidInputCharacter { .. })
    ));
    // Invalid character in the output field.
    assert!(matches!(
        err(".i 1\n.o 1\n0 9\n.e\n"),
        PLAReadError::PLA(PLAError::InvalidOutputCharacter { .. })
    ));
    // Non-numeric .i directive value.
    assert!(matches!(
        err(".i two\n.o 1\n01 1\n.e\n"),
        PLAReadError::PLA(PLAError::InvalidInputDirective { .. })
    ));
    // Non-numeric .o directive value (symmetric to the .i case above).
    assert!(matches!(
        err(".i 2\n.o two\n01 1\n.e\n"),
        PLAReadError::PLA(PLAError::InvalidOutputDirective { .. })
    ));
    // .i present but no .o.
    assert!(matches!(
        err(".i 2\n.e\n"),
        PLAReadError::PLA(PLAError::MissingOutputDirective)
    ));
    // .o present but no .i. (Order between .i and .o does not matter; only that both are present.)
    assert!(matches!(
        err(".o 1\n.e\n"),
        PLAReadError::PLA(PLAError::MissingInputDirective)
    ));
    // Unrecognised .type value is rejected (consistent with bad .i/.o), not silently defaulted.
    assert!(matches!(
        err(".i 2\n.o 1\n.type bogus\n01 1\n.e\n"),
        PLAReadError::PLA(PLAError::InvalidTypeDirective { .. })
    ));
}

#[test]
fn pla_delimiters_match_c_positional_reading() {
    // C's read_cube (cvrin.c) treats space, tab and '|' as insignificant delimiters that may appear
    // anywhere; the input/output boundary is positional, fixed by .i/.o. So '|' is NOT a boundary
    // marker — multiple '|' (and spaces) mid-line are skipped and the field is split at .i.
    let serialise = |pla: &str| {
        PlaCover::<Symbol>::from_pla_string(pla)
            .unwrap_or_else(|e| panic!("{pla:?} should parse, got {e:?}"))
            .to_pla_string(CoverType::FD)
            .expect("serialise")
    };
    let canonical = serialise(".i 4\n.o 2\n0110 11\n.e\n");

    // Same cube with '|' sprinkled through both fields (and inner spaces) must read to the identical
    // cover, exactly as the C reader would read it character by character.
    for variant in [
        ".i 4\n.o 2\n01|10|11\n.e\n",
        ".i 4\n.o 2\n0 1 1 0 | 1 1\n.e\n",
        ".i 4\n.o 2\n|0110|11|\n.e\n",
    ] {
        assert_eq!(
            serialise(variant),
            canonical,
            "variant {variant:?} should read positionally like C"
        );
    }
}

#[test]
fn accepts_i_and_o_directives_in_either_order() {
    // C forbids `.o` before `.i`, but this crate intentionally accepts the two directives in any
    // order — only that both are present before the first cube matters.
    let normal = PlaCover::<Symbol>::from_pla_string(".i 2\n.o 1\n01 1\n.e\n").unwrap();
    let swapped = PlaCover::<Symbol>::from_pla_string(".o 1\n.i 2\n01 1\n.e\n")
        .expect(".o before .i is accepted");
    assert_eq!(swapped.num_inputs(), 2);
    assert_eq!(swapped.num_outputs(), 1);
    assert_eq!(
        swapped.to_pla_string(CoverType::FD).unwrap(),
        normal.to_pla_string(CoverType::FD).unwrap(),
        "directive order must not affect the parsed cover"
    );
}

#[test]
fn from_pla_file_missing_path_is_io_error() {
    use super::pla::PLAReadError;

    // A nonexistent path surfaces as the IO variant (not a PLA-format error), so callers can
    // distinguish "couldn't open the file" from "the file's contents are malformed".
    let result = PlaCover::<Symbol>::from_pla_file("/no/such/espresso_input.pla");
    assert!(
        matches!(result, Err(PLAReadError::Io(_))),
        "missing file should be PLAReadError::Io, got {result:?}"
    );
}

#[test]
fn minimise_empty_cover_is_ok_with_no_cubes() {
    // A declared-but-empty cover (labels set, zero cubes) must minimise without panicking on the
    // degenerate 0-cube FFI path, returning an equally-empty cover.
    let cover: Cover<Symbol, Symbol> = Cover::with_labels(CoverType::F, &["a"], &["o"]);
    assert_eq!(cover.num_cubes(), 0);
    let minimised = cover
        .minimize()
        .expect("empty cover should minimise cleanly");
    assert_eq!(minimised.num_cubes(), 0);
    assert_eq!(minimised.num_inputs(), 1);
    assert_eq!(minimised.num_outputs(), 1);
}

#[test]
fn pla_input_field_chars_match_c() {
    use super::pla::{PLAError, PLAReadError};

    // `~`, `x`, `X` are rejected in the input field (C rejects them there — only `0 1 2 - ?` are valid).
    for bad in ["~0 1", "x0 1", "X0 1"] {
        let src = format!(".i 2\n.o 1\n{bad}\n.e\n");
        assert!(
            matches!(
                PlaCover::<Symbol>::from_pla_string(&src),
                Err(PLAReadError::PLA(PLAError::InvalidInputCharacter { .. }))
            ),
            "input {bad:?} should be rejected"
        );
    }

    // `?` is the empty literal: it parses (no error), but the cube covers no minterm, so it is dropped
    // at minimisation — leaving the function defined only by the real cubes.
    let to_pla = |s: &str| {
        PlaCover::<Symbol>::from_pla_string(s)
            .expect("parses")
            .minimize()
            .expect("minimises")
            .to_pla_string(CoverType::F)
            .expect("serialise")
    };
    assert_eq!(
        to_pla(".i 2\n.o 1\n?1 1\n10 1\n.e\n"),
        to_pla(".i 2\n.o 1\n10 1\n.e\n"),
        "an empty (?) cube must be dropped, leaving just the real cube"
    );

    // Read-then-write (no minimisation) echoes the empty literal `?` faithfully, like C's `print_cube`
    // (`"?01-"[field]`). The `Option<bool>` view would fold `00`→`-`; the writer reads the raw fields.
    let roundtrip = PlaCover::<Symbol>::from_pla_string(".i 4\n.o 1\n?01- 1\n.e\n")
        .expect("parses")
        .to_pla_string(CoverType::F)
        .expect("serialise");
    assert!(
        roundtrip.contains("?01- 1"),
        "empty literal `?` must survive a read-then-write round-trip, got:\n{roundtrip}"
    );
}

#[test]
fn minimise_rejects_non_orthogonal_on_off_sets() {
    use crate::espresso::MinimizationError;

    // An FR cover where the same minterm `10` is asserted in BOTH the ON-set (F) and OFF-set (R) for
    // output 0 is contradictory. C's `expand` would `exit(1)` the process; we reject it with an error.
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::FR);
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::R,
    ));
    let err = cover
        .minimize()
        .expect_err("overlapping ON/OFF sets must be rejected");
    assert!(
        matches!(err, MinimizationError::NonOrthogonal { output: 0 }),
        "expected NonOrthogonal at output 0, got {err:?}"
    );
}

#[test]
fn try_new_rejects_oversized_dimensions() {
    use crate::espresso::{Espresso, InstanceError, MinimizationError};

    // A dimension beyond what the C core's `c_int` cube indices can represent must be a recoverable
    // error, not a process `exit(1)`.
    let err = Espresso::try_new(usize::MAX, 1, None).expect_err("oversized dimension must error");
    assert!(
        matches!(
            err,
            MinimizationError::Instance(InstanceError::DimensionTooLarge { .. })
        ),
        "expected DimensionTooLarge, got {err:?}"
    );

    // Pathological pair: each dimension is within `c_int::MAX` on its own, but the C core's
    // `cube.size = 2*num_inputs + num_outputs` accumulation would overflow `c_int` and wrap negative.
    // The guard's checked arithmetic must still reject it rather than letting `cube_setup` abort.
    let big = (i32::MAX as usize) - 1;
    let err = Espresso::try_new(big, big, None).expect_err("overflowing cube.size must error");
    assert!(
        matches!(
            err,
            MinimizationError::Instance(InstanceError::DimensionTooLarge { .. })
        ),
        "expected DimensionTooLarge for the overflowing pair, got {err:?}"
    );
}

#[test]
fn cover_hash_and_blanket_default() {
    use std::collections::HashSet;

    // Default is now generic over any label types (Symbol no longer privileged).
    let _: Cover<Anonymous, Anonymous> = Cover::default();
    let _: Cover<Symbol, Symbol> = Cover::default();
    let _: Cover<u32, u32> = Cover::default();
    assert_eq!(Cover::<Anonymous, Anonymous>::default().num_cubes(), 0);

    // Cover and Cube are Hash, consistent with their Eq.
    let build = || {
        let mut c = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
        c.push(Cube::anonymous(&[Some(true), None], &[true], CubeType::F));
        c
    };
    let mut set = HashSet::new();
    set.insert(build());
    assert!(set.contains(&build()));
}

#[test]
fn test_minimize_preserves_structure() {
    let mut cover = Cover::new(CoverType::F);

    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");

    cover.add_expr(&a.and(&b), "out1").unwrap();
    cover.add_expr(&a.or(&b), "out2").unwrap();

    let inputs_before = cover.num_inputs();
    let outputs_before = cover.num_outputs();

    cover = cover.minimize().unwrap();

    // Dimensions should be preserved
    assert_eq!(cover.num_inputs(), inputs_before);
    assert_eq!(cover.num_outputs(), outputs_before);

    // Should still be able to extract both expressions
    let expr1 = cover.to_expr("out1").unwrap();
    let expr2 = cover.to_expr("out2").unwrap();

    assert!(expr1.collect_variables().len() <= 2);
    assert!(expr2.collect_variables().len() <= 2);
}

// ===== Generic label type / anonymous covers (M3) =====

#[test]
fn anonymous_cover_minimizes() {
    // Pure positional cover, no labels (L = Anonymous).
    let mut cover: Cover<Anonymous, Anonymous> = Cover::anonymous(CoverType::F);
    cover.push(Cube::anonymous(
        &[Some(false), Some(true)],
        &[true],
        CubeType::F,
    )); // 01 -> 1
    cover.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    )); // 10 -> 1
    assert_eq!(cover.num_inputs(), 2);
    assert_eq!(cover.num_cubes(), 2);
    let min = cover.minimize().unwrap();
    assert_eq!(min.num_inputs(), 2);
    assert!(min.num_cubes() >= 1);
}

#[test]
fn custom_u32_labels_via_relabel() {
    let mut cover: Cover<Anonymous, Anonymous> = Cover::anonymous(CoverType::F);
    cover.push(Cube::anonymous(
        &[Some(true), None, Some(false)],
        &[true],
        CubeType::F,
    ));
    // Explicitly relabel to a u32-labelled cover, position-for-position.
    let labeled: Cover<u32, u32> = cover
        .relabel(
            Symbols::new(vec![10u32, 20, 30].into()),
            Symbols::new(vec![1u32].into()),
        )
        .unwrap();
    assert_eq!(labeled.num_inputs(), 3);
    let first = labeled.cubes().next().unwrap();
    assert_eq!(first.inputs().value_of(&10u32), Some(true));
    assert_eq!(first.inputs().value_of(&20u32), None);
    assert_eq!(first.inputs().value_of(&30u32), Some(false));
    assert_eq!(first.inputs().value_of(&99u32), None); // absent variable
    let _ = labeled.minimize().unwrap();
}

#[test]
fn anonymize_drops_labels_preserving_values() {
    // Build positionally, label explicitly, then anonymise back — values preserved throughout.
    let mut anon = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    anon.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));
    let labeled = anon
        .relabel(
            Symbols::new(vec![Symbol::from("a"), Symbol::from("b")].into()),
            Symbols::new(vec![Symbol::from("out")].into()),
        )
        .unwrap();
    assert_eq!(labeled.num_inputs(), 2);

    let back: Cover<Anonymous, Anonymous> = labeled.anonymize();
    assert_eq!(back.num_cubes(), 1);
    let cube = back.cubes().next().unwrap();
    assert_eq!(cube.inputs().value_at(0), Some(true));
    assert_eq!(cube.inputs().value_at(1), Some(false));
}

#[test]
fn cover_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Cover<Anonymous, Anonymous>>();
    assert_send_sync::<Cover<u32, u32>>();
    assert_send_sync::<Cover<Symbol, Symbol>>();
}

// ===== extend / merge =====

/// Membership rows (`Some(true)`=asserted) of every cube, in order.
fn output_rows<I, O>(c: &Cover<I, O>) -> Vec<Vec<bool>> {
    c.cubes()
        .map(|cube| cube.outputs().iter().collect())
        .collect()
}

#[test]
fn extend_appends_anonymous_outputs() {
    let mut a = Cover::from_cubes(
        CoverType::F,
        [Cube::anonymous(
            &[Some(true), Some(false)],
            &[true],
            CubeType::F,
        )],
    );
    let b = Cover::from_cubes(
        CoverType::F,
        [Cube::anonymous(
            &[Some(false), Some(true)],
            &[true],
            CubeType::F,
        )],
    );
    a.extend(&b);

    assert_eq!(a.num_inputs(), 2);
    assert_eq!(a.num_outputs(), 2); // appended: 1 + 1
    assert_eq!(a.num_cubes(), 2);
    // a's cube asserts output 0 only; b's cube asserts output 1 only.
    assert_eq!(output_rows(&a), vec![vec![true, false], vec![false, true]]);
}

#[test]
fn merge_overlays_anonymous_outputs_by_position() {
    let mut a = Cover::from_cubes(
        CoverType::F,
        [Cube::anonymous(
            &[Some(true), Some(false)],
            &[true],
            CubeType::F,
        )],
    );
    let b = Cover::from_cubes(
        CoverType::F,
        [Cube::anonymous(
            &[Some(false), Some(true)],
            &[true],
            CubeType::F,
        )],
    );
    a.merge(&b);

    assert_eq!(a.num_inputs(), 2);
    assert_eq!(a.num_outputs(), 1); // overlaid: max(1, 1)
    assert_eq!(a.num_cubes(), 2);
    // Both cubes assert the same (position-0) output.
    assert_eq!(output_rows(&a), vec![vec![true], vec![true]]);
}

#[test]
fn extend_aligns_named_inputs_anonymous_outputs() {
    let sym = |s: &str| Symbol::from(s);
    // Labelled inputs, anonymous output, built by relabelling the inputs of an anonymous cover.
    let mut a = Cover::from_cubes(
        CoverType::F,
        [Cube::anonymous(&[Some(true)], &[true], CubeType::F)],
    )
    .relabel_inputs(Symbols::new(vec![sym("x")].into()))
    .unwrap();
    let b = Cover::from_cubes(
        CoverType::F,
        [Cube::anonymous(&[Some(true)], &[true], CubeType::F)],
    )
    .relabel_inputs(Symbols::new(vec![sym("y")].into()))
    .unwrap();

    a.extend(&b);
    assert_eq!(a.num_inputs(), 2); // union {x, y}
    assert_eq!(a.input_labels(), &[sym("x"), sym("y")]);
    assert_eq!(a.num_outputs(), 2); // appended
    assert_eq!(a.num_cubes(), 2);
}

#[test]
fn extend_equals_merge_for_distinct_named_outputs() {
    // When the two covers' output names DON'T collide, extend (append) and merge (overlay) coincide:
    // both keep the two distinct columns. They diverge only on a collision (tests below).
    let mut by_extend = Cover::new(CoverType::F);
    by_extend
        .add_expr(&crate::BoolExpr::variable("x"), "f")
        .unwrap();
    let mut other = Cover::new(CoverType::F);
    other
        .add_expr(&crate::BoolExpr::variable("y"), "g")
        .unwrap();

    let mut by_merge = by_extend.clone();
    by_extend.extend(&other);
    by_merge.merge(&other);

    assert_eq!(by_extend.num_inputs(), by_merge.num_inputs());
    assert_eq!(by_extend.num_outputs(), by_merge.num_outputs());
    assert_eq!(by_extend.input_labels(), by_merge.input_labels());
    assert_eq!(by_extend.output_labels(), by_merge.output_labels());
    assert_eq!(output_rows(&by_extend), output_rows(&by_merge));
    // Two distinct named outputs from two single-output expressions.
    assert_eq!(by_extend.num_outputs(), 2);
    assert_eq!(by_extend.num_inputs(), 2); // union {x, y}
}

#[test]
fn extend_renames_colliding_named_outputs() {
    // Both covers output "f"; extend always appends, reconciling the clash to "f0".
    let mut a = Cover::new(CoverType::F);
    a.add_expr(&crate::BoolExpr::variable("x"), "f").unwrap();
    let mut b = Cover::new(CoverType::F);
    b.add_expr(&crate::BoolExpr::variable("y"), "f").unwrap();

    a.extend(&b);
    assert_eq!(a.num_outputs(), 2); // distinct columns, not overlaid
    assert_eq!(a.output_labels()[0].as_ref(), "f");
    assert_eq!(a.output_labels()[1].as_ref(), "f0"); // reconciled
    assert_eq!(a.num_inputs(), 2); // union {x, y}
}

#[test]
fn extend_reconciles_repeated_output_collisions() {
    // Three covers all output "f"; each extend reconciles against the names already present, so the
    // suffixes advance f -> f0 -> f1 rather than colliding again.
    let mut a = Cover::new(CoverType::F);
    a.add_expr(&crate::BoolExpr::variable("x"), "f").unwrap();
    let mut b = Cover::new(CoverType::F);
    b.add_expr(&crate::BoolExpr::variable("y"), "f").unwrap();
    let mut c = Cover::new(CoverType::F);
    c.add_expr(&crate::BoolExpr::variable("z"), "f").unwrap();

    a.extend(&b);
    a.extend(&c);

    assert_eq!(a.num_outputs(), 3);
    assert_eq!(a.output_labels()[0].as_ref(), "f");
    assert_eq!(a.output_labels()[1].as_ref(), "f0");
    assert_eq!(a.output_labels()[2].as_ref(), "f1");
    assert_eq!(a.num_inputs(), 3); // union {x, y, z}
}

#[test]
fn merge_overlays_colliding_named_outputs() {
    // Both covers output "f"; merge overlays them onto one column (pins the divergence from extend).
    let mut a = Cover::new(CoverType::F);
    a.add_expr(&crate::BoolExpr::variable("x"), "f").unwrap();
    let mut b = Cover::new(CoverType::F);
    b.add_expr(&crate::BoolExpr::variable("y"), "f").unwrap();

    a.merge(&b);
    assert_eq!(a.num_outputs(), 1); // single overlaid column
    assert_eq!(a.output_labels()[0].as_ref(), "f");
    // Both source cubes now assert the one shared output.
    assert!(output_rows(&a).iter().all(|row| row == &vec![true]));
}

// ===== BoolExpr -> Cover<Symbol, Anonymous> and per-side relabel =====

#[test]
fn expr_into_anonymous_output_cover_roundtrips() {
    let a = crate::BoolExpr::variable("a");
    let b = crate::BoolExpr::variable("b");
    let expr = a.and(&b).or(&a.and(&b)); // redundant on purpose

    // From<&BoolExpr> yields a labelled-input, anonymous-output cover (via the BDD).
    let cover: Cover<Symbol, Anonymous> = (&expr).into();
    assert_eq!(cover.num_outputs(), 1);
    assert!(!cover.input_labels().is_empty()); // inputs are named (a, b)

    // Reconstruction is index-addressed (no output name needed) and recovers the function.
    let back = cover.to_expr_by_index(0).unwrap();
    assert!(back.equivalent_to(&expr));
}

#[test]
fn relabel_outputs_keeps_inputs() {
    let mut named = Cover::new(CoverType::F);
    named
        .add_expr(&crate::BoolExpr::variable("x"), "f")
        .unwrap();

    // Drop only the output label, keeping the named inputs.
    let anon_out: Cover<Symbol, Anonymous> = named
        .clone()
        .relabel_outputs(Symbols::<Anonymous>::anonymous(1))
        .unwrap();
    assert_eq!(anon_out.input_labels(), named.input_labels());
    assert_eq!(anon_out.num_outputs(), 1);
    assert_eq!(io_rows(&anon_out), io_rows(&named));

    // Dual: relabel only the inputs, keeping the named output.
    let anon_in: Cover<Anonymous, Symbol> = named
        .clone()
        .relabel_inputs(Symbols::<Anonymous>::anonymous(named.num_inputs()))
        .unwrap();
    assert_eq!(anon_in.output_labels(), named.output_labels());
    assert_eq!(anon_in.num_inputs(), named.num_inputs());
}

// --- Requirement 2: cube / cover expansion over an explicit variable set -----------------------

/// Collect a cover's input minterms as a `BTreeSet` (they share one canonical header after
/// `maximize`, so they compare on the fast path).
fn input_minterm_set(cover: &Cover<Symbol, Symbol>) -> std::collections::BTreeSet<Minterm<Symbol>> {
    cover.cubes().map(|c| c.inputs().clone()).collect()
}

/// A cube `a=1` expanded over [a, b] yields exactly {a:1,b:0}, {a:1,b:1}.
#[test]
fn cube_expand_to_splits_unconstrained_var() {
    let cube = Cube::<Symbol, Symbol>::with_labels(
        &[("a", Some(true))],
        &[("f", true)],
        CubeType::F,
    )
    .unwrap();
    let got: std::collections::BTreeSet<_> = cube
        .expand_to(&[Symbol::from("a"), Symbol::from("b")])
        .into_iter()
        .collect();
    let header = Symbols::new([Symbol::from("a"), Symbol::from("b")].into_iter().collect());
    let want: std::collections::BTreeSet<_> = [
        Minterm::from_symbols(Arc::clone(&header), [Some(true), Some(false)]),
        Minterm::from_symbols(Arc::clone(&header), [Some(true), Some(true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(got, want);
}

/// A cube `a=1` expanded over [a, b, c] (c absent from the cube) splits c into both polarities,
/// yielding 4 minterms.
#[test]
fn cube_expand_to_widens_with_absent_var() {
    let cube = Cube::<Symbol, Symbol>::with_labels(
        &[("a", Some(true))],
        &[("f", true)],
        CubeType::F,
    )
    .unwrap();
    let got: std::collections::BTreeSet<_> = cube
        .expand_to(&[Symbol::from("a"), Symbol::from("b"), Symbol::from("c")])
        .into_iter()
        .collect();
    let header = Symbols::new(
        [Symbol::from("a"), Symbol::from("b"), Symbol::from("c")]
            .into_iter()
            .collect(),
    );
    let want: std::collections::BTreeSet<_> = [
        Minterm::from_symbols(Arc::clone(&header), [Some(true), Some(false), Some(false)]),
        Minterm::from_symbols(Arc::clone(&header), [Some(true), Some(false), Some(true)]),
        Minterm::from_symbols(Arc::clone(&header), [Some(true), Some(true), Some(false)]),
        Minterm::from_symbols(Arc::clone(&header), [Some(true), Some(true), Some(true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(got, want);
    assert_eq!(got.len(), 4);
}

/// `Cover::maximize` of an already-maximal cover over the same variables is a no-op: the input
/// minterm set is unchanged and every cube assigns every variable.
#[test]
fn cover_maximize_is_idempotent_when_already_maximal() {
    let vars = [Symbol::from("a"), Symbol::from("b")];
    // An already-maximal cover: both cubes assign every variable, no don't-cares.
    let cover = Cover::<Symbol, Symbol>::from_cubes(
        CoverType::F,
        [
            Cube::with_labels(&[("a", Some(true)), ("b", Some(false))], &[("f", true)], CubeType::F)
                .unwrap(),
            Cube::with_labels(&[("a", Some(true)), ("b", Some(true))], &[("f", true)], CubeType::F)
                .unwrap(),
        ],
    );

    let maximised = cover.maximize(&vars);
    // Same input minterm set as the input (idempotent).
    assert_eq!(input_minterm_set(&maximised), input_minterm_set(&cover));
    // Re-maximising changes nothing further.
    let twice = maximised.maximize(&vars);
    assert_eq!(input_minterm_set(&twice), input_minterm_set(&maximised));
    // Every minterm is fully assigned (no don't-cares left).
    for cube in maximised.cubes() {
        assert!(cube.inputs().iter().all(|v| v.is_some()));
    }
}

/// `Cover::maximize` expands a cube with a don't-care into both polarities over the explicit header.
#[test]
fn cover_maximize_expands_dont_care() {
    let vars = [Symbol::from("a"), Symbol::from("b")];
    // a=1, b unconstrained → should expand to {a:1,b:0}, {a:1,b:1}.
    let cover = Cover::<Symbol, Symbol>::from_cubes(
        CoverType::F,
        [Cube::with_labels(&[("a", Some(true))], &[("f", true)], CubeType::F).unwrap()],
    );
    let maximised = cover.maximize(&vars);
    let header = Symbols::new([Symbol::from("a"), Symbol::from("b")].into_iter().collect());
    let want: std::collections::BTreeSet<_> = [
        Minterm::from_symbols(Arc::clone(&header), [Some(true), Some(false)]),
        Minterm::from_symbols(Arc::clone(&header), [Some(true), Some(true)]),
    ]
    .into_iter()
    .collect();
    assert_eq!(input_minterm_set(&maximised), want);
}
