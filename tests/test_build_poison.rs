//! A panic inside a `BoolExpr::build` closure must poison the BDD manager lock and propagate — the
//! crate never silently recovers from a poisoned lock, because a mid-build panic may have left the
//! manager's hash-cons tables inconsistent.
//!
//! The BDD manager is process-global, so this lives in its own integration-test binary: poisoning the
//! manager here cannot disturb other tests, which run in separate processes. Keep this file to a single
//! test for the same reason (tests within one binary share the process and run concurrently).

use espresso_logic::BoolExpr;

#[test]
fn build_panic_poisons_the_manager_and_propagates() {
    // Silence the default panic hook: we deliberately trigger panics and catch them, and don't want the
    // backtraces on stderr to look like failures.
    std::panic::set_hook(Box::new(|_| {}));

    // Hold a live expression so the manager's Arc — and therefore the poisoned lock — stays alive across
    // the panic (otherwise the manager would be freed and lazily recreated, unpoisoned, on next use).
    let _keep = BoolExpr::variable("keep");

    let panicked = std::panic::catch_unwind(|| {
        BoolExpr::build(|b| {
            let _ = b.var("a");
            panic!("boom inside build");
        })
    });
    assert!(
        panicked.is_err(),
        "a panic in the closure must propagate out of build"
    );

    // The manager lock is now poisoned; a subsequent operation that locks it must also propagate the
    // poison (panic) rather than observe a half-updated manager.
    let after = std::panic::catch_unwind(|| BoolExpr::variable("z"));
    assert!(
        after.is_err(),
        "operations after a poisoning panic must propagate the poison, not recover"
    );
}
