//! Tests for thread safety feature
//!
//! These tests verify that the library is thread-safe using C11 thread-local
//! storage for global state. Multiple threads can safely use the library
//! concurrently without synchronization.

use espresso_logic::Anonymous;
use espresso_logic::{Cover, CoverType, Cube, CubeType, Minimizable};
use std::thread;
use std::time::Duration;

#[test]
fn test_basic_thread_safety() {
    // Create a cover and add cubes (XOR pattern)
    let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
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

    // Minimize using thread-safe C library
    cover = cover.minimize().expect("Minimization failed");

    // Verify XOR pattern cannot be minimized - should remain 2 cubes
    assert_eq!(
        cover.num_cubes(),
        2,
        "XOR pattern should have exactly 2 cubes"
    );
}

#[test]
fn test_concurrent_execution() {
    // Each thread executes Espresso independently - no shared state due to thread-local storage!
    let handles: Vec<_> = (0..4)
        .map(|i| {
            thread::spawn(move || {
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

                // Thread-safe - each thread executes with independent global state
                cover = cover.minimize().expect("Minimization failed");

                let num_cubes = cover.num_cubes();
                println!("Thread {} completed with {} cubes", i, num_cubes);
                num_cubes
            })
        })
        .collect();

    // Wait for all threads to complete
    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    // All should succeed with XOR pattern (cannot be minimized)
    assert_eq!(results.len(), 4);
    for num_cubes in results {
        assert_eq!(num_cubes, 2, "XOR pattern should have exactly 2 cubes");
    }
}

#[test]
fn test_consistent_results() {
    // Test that multiple executions produce consistent results

    // First execution
    let mut cover1 = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover1.push(Cube::anonymous(
        &[Some(false), Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));
    cover1.push(Cube::anonymous(
        &[Some(false), Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));
    cover1.push(Cube::anonymous(
        &[Some(true), Some(false), Some(false)],
        &[true],
        CubeType::F,
    ));
    let min1 = cover1.minimize().unwrap();

    // Second execution
    let mut cover2 = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover2.push(Cube::anonymous(
        &[Some(false), Some(false), Some(true)],
        &[true],
        CubeType::F,
    ));
    cover2.push(Cube::anonymous(
        &[Some(false), Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));
    cover2.push(Cube::anonymous(
        &[Some(true), Some(false), Some(false)],
        &[true],
        CubeType::F,
    ));
    let min2 = cover2.minimize().unwrap();

    // The *minimised* covers (not the untouched inputs) must match — same cube count and, by value,
    // the same cubes. Comparing the inputs would be tautological.
    assert_eq!(
        min1.num_cubes(),
        min2.num_cubes(),
        "Results should have same number of cubes"
    );
    assert_eq!(
        min1, min2,
        "identical inputs must minimise to identical covers"
    );
}

#[test]
fn test_multiple_sizes() {
    // Test that Cover can handle DIFFERENT dimensions sequentially
    // (unlike EspressoCover which has dimension restrictions)

    // Create first cover with 2 inputs, 1 output
    let mut cover1 = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover1.push(Cube::anonymous(
        &[Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));
    cover1.minimize().unwrap();
    assert_eq!(cover1.num_cubes(), 1, "Cover1 (2x1) should have 1 cube");

    // Cover should handle different dimensions thanks to automatic cleanup
    let mut cover2 = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
    cover2.push(Cube::anonymous(
        &[Some(false), Some(true), Some(false)],
        &[true],
        CubeType::F,
    ));
    cover2.minimize().unwrap();
    assert_eq!(cover2.num_cubes(), 1, "Cover2 (3x1) should have 1 cube");

    // Even after minimization, both should maintain their independence
    assert_eq!(cover1.num_cubes(), 1, "Cover1 should still have 1 cube");
    assert_eq!(cover2.num_cubes(), 1, "Cover2 should still have 1 cube");
}

#[test]
fn test_different_sizes_in_different_threads() {
    // Each thread can have its own dimensions
    use std::thread;

    let handle1 = thread::spawn(|| {
        let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
        cover.push(Cube::anonymous(
            &[Some(true), Some(false)],
            &[true],
            CubeType::F,
        ));
        cover = cover.minimize().unwrap();
        cover.num_cubes()
    });

    let handle2 = thread::spawn(|| {
        let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
        cover.push(Cube::anonymous(
            &[Some(true), Some(false), Some(true)],
            &[true, false],
            CubeType::F,
        ));
        cover = cover.minimize().unwrap();
        cover.num_cubes()
    });

    assert!(handle1.join().unwrap() > 0);
    assert!(handle2.join().unwrap() > 0);
}

#[test]
fn test_stress_concurrent() {
    // Stress test with many concurrent operations
    let handles: Vec<_> = (0..10)
        .map(|i| {
            thread::spawn(move || {
                // Each thread performs multiple operations
                for j in 0..3 {
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

                    match cover.minimize() {
                        Ok(minimized) => {
                            cover = minimized;
                            // XOR pattern should have exactly 2 cubes (cannot be minimized)
                            assert_eq!(cover.num_cubes(), 2);
                        }
                        Err(e) => {
                            eprintln!("Thread {}-{} failed: {}", i, j, e);
                            panic!("Minimization failed");
                        }
                    }

                    // Small delay to create more overlap
                    thread::sleep(Duration::from_millis(10));
                }
            })
        })
        .collect();

    // All threads should complete successfully
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}

#[test]
fn concurrent_symbol_covers() {
    use espresso_logic::{BoolExpr, Symbol};

    // The other cases all minimise an anonymous 2x1 XOR. This one builds per-thread `Symbol`-labelled
    // covers with distinct names, exercising the shared `Symbol` intern pool and the labelled
    // Cover/minimise path across threads.
    let handles: Vec<_> = (0..8)
        .map(|t| {
            thread::spawn(move || {
                let a = BoolExpr::variable(format!("t{t}_a"));
                let b = BoolExpr::variable(format!("t{t}_b"));
                let mut cover: Cover<Symbol, Symbol> = Cover::new(CoverType::F);
                // a*b + a*~b  ==  a  (so b drops out under minimisation)
                cover
                    .add_expr(&a.and(&b).or(&a.and(&b.not())), format!("out{t}"))
                    .unwrap();
                let min = cover.minimize().unwrap();
                assert!(min.num_cubes() >= 1);
                assert!(
                    min.input_labels()
                        .iter()
                        .all(|s| s.as_ref().starts_with(&format!("t{t}_"))),
                    "labels must stay this thread's own"
                );
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}

/// Many threads build the **same** overlapping expressions against one shared, thread-safe
/// [`SyncBddBuilder`], hammering its read-mostly double-checked locking (concurrent node interning) and
/// the ITE cache-commit transaction. Canonicity must hold under contention: identical expressions
/// reduce to one shared root, so every thread's handles must be `equivalent_to` the reference, with no
/// deadlock, panic, or duplicate nodes (a duplicated node would give a different root and fail the
/// equivalence check).
#[test]
fn concurrent_shared_manager_building_stays_canonical() {
    use espresso_logic::{sync_bdd_builder, Bdd, BoolExpr, Brand, SyncBddBuilder};

    // A suite of varied shapes over shared variable names, exercising var/and/or/not/xor/ite, the
    // expression `build`, and the parser — all against the one shared context.
    fn build_suite<B: Brand>(ctx: &SyncBddBuilder<B>) -> Vec<Bdd<'_, B>> {
        let a = ctx.var("share_a");
        let b = ctx.var("share_b");
        let c = ctx.var("share_c");
        vec![
            (a ^ b) ^ c,             // XOR chain
            (a & b) | (b & c) | (a & c), // majority
            ctx.parse("share_a * share_b + ~share_c").unwrap(),
            // Same function as the parsed entry above, but constructed via owned `BoolExpr` operators
            // and fed through `ctx.build`; canonicity means it must reduce to the same root.
            ctx.build(
                &((BoolExpr::var("share_a") & BoolExpr::var("share_b")) | !BoolExpr::var("share_c")),
            ),
            a.ite(b, c),
        ]
    }

    const THREADS: usize = 16;
    const ITERS: usize = 200;

    // One thread-safe context, shared by reference across every worker (`SyncBddBuilder` is Send + Sync).
    let ctx = sync_bdd_builder!();

    // Reference built on the main thread.
    let reference = build_suite(&ctx);

    thread::scope(|scope| {
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                scope.spawn(|| {
                    // Re-build the suite many times under contention; return the last for the check.
                    let mut last = build_suite(&ctx);
                    for _ in 1..ITERS {
                        last = build_suite(&ctx);
                    }
                    last
                })
            })
            .collect();

        for handle in handles {
            let suite = handle
                .join()
                .expect("builder thread must not panic or deadlock");
            assert_eq!(suite.len(), reference.len());
            for (got, want) in suite.iter().zip(reference.iter()) {
                assert!(
                    got.equivalent_to(*want),
                    "concurrent builds against the shared context must yield identical canonical BDDs"
                );
            }
        }
    });
}
