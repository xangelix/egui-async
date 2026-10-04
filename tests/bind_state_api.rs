//! Focused tests for getters and `StateWithData` mapping.

use std::sync::{Mutex, MutexGuard, OnceLock};

use egui_async::bind::{CURR_FRAME, LAST_FRAME};
use egui_async::{Bind, StateWithData};

/// Serializes tests, since they share the global frame timers.
fn test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    // Be resilient to a previously-poisoned lock so one failure doesn't cascade.
    match LOCK.get_or_init(|| Mutex::new(())).lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn set_frame_times(curr: f64, last: f64) {
    CURR_FRAME.store(curr, std::sync::atomic::Ordering::Relaxed);
    LAST_FRAME.store(last, std::sync::atomic::Ordering::Relaxed);
}

#[test]
fn state_method_maps_ok_and_err_variants() -> Result<(), String> {
    let _lock = test_lock();
    set_frame_times(10.0, 9.0);

    let mut ok: Bind<&'static str, &'static str> = Bind::new(true);
    ok.fill(Ok("good"));
    let StateWithData::Finished(v) = ok.state() else {
        return Err("expected Finished(..) variant".into());
    };
    assert_eq!(*v, "good");

    let mut err: Bind<&'static str, &'static str> = Bind::new(true);
    err.fill(Err("bad"));
    let StateWithData::Failed(e) = err.state() else {
        return Err("expected Failed(..) variant".into());
    };
    assert_eq!(*e, "bad");

    Ok(())
}

#[allow(clippy::float_cmp)]
#[test]
fn elapsed_and_since_helpers_are_coherent() {
    let _lock = test_lock();
    set_frame_times(100.0, 99.0);
    let mut b: Bind<i32, i32> = Bind::new(true);

    // Simulate a run fully inside one frame for simplicity.
    b.fill(Ok(1));
    let start = b.get_start_time(); // may be the default (e.g., 0.0) since `fill` skips "start"
    let complete = b.get_complete_time();
    let elapsed = b.get_elapsed();

    assert!(complete >= start);
    assert_eq!(elapsed, complete - start);

    // Advance two frames and check "since" helpers grow accordingly.
    CURR_FRAME.store(102.0, std::sync::atomic::Ordering::Relaxed);
    assert!((b.since_completed() - (102.0 - complete)).abs() < 1e-9);
    assert!((b.since_started() - (102.0 - start)).abs() < 1e-9);
}
