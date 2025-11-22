use egui_async::run_once;

#[test]
fn test_run_once_macro() {
    // We must wrap the macro in a closure or function to test that the SAME callsite
    // respects the Once semantics. Calling the macro twice on different lines
    // generates two DIFFERENT static variables.
    let run_check = || {
        run_once! {
            let _ = 1 + 1;
        }
    };

    // 1. First run returns true
    let first = run_check();
    assert!(first, "First execution should return true");

    // 2. Second run returns false (same callsite)
    let second = run_check();
    assert!(!second, "Second execution should return false");
}

#[test]
fn test_run_once_side_effects() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let increment = || {
        run_once! {
            COUNTER.fetch_add(1, Ordering::Relaxed);
        }
    };

    // Call it multiple times
    let res1 = increment();
    let res2 = increment();
    let res3 = increment();

    // Only the first should return true
    assert!(res1);
    assert!(!res2);
    assert!(!res3);

    // The counter should only have incremented once
    assert_eq!(COUNTER.load(Ordering::Relaxed), 1);
}
