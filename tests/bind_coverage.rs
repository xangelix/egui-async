//! Comprehensive coverage tests for `egui-async::bind`.
//! Covers Debug impls, getters, setters, and edge-case polling logic.

use egui_async::{
    Bind, State, StateWithData,
    bind::{CURR_FRAME, LAST_FRAME},
};
use std::sync::{Mutex, OnceLock};

// --- Test Helpers ---

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn with_lock<T>(f: impl FnOnce() -> T) -> T {
    let _keep = match test_lock().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    f()
}

fn set_time(curr: f64, last: f64) {
    CURR_FRAME.store(curr, std::sync::atomic::Ordering::Relaxed);
    LAST_FRAME.store(last, std::sync::atomic::Ordering::Relaxed);
}

// Helper to simulate frame advancement
fn advance_frame(seconds: f64) {
    let current = CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed);
    CURR_FRAME.store(current + seconds, std::sync::atomic::Ordering::Relaxed);
}

// --- Tests ---

#[test]
fn debug_impls_coverage() {
    with_lock(|| {
        // Cover StateWithData Debug
        let idle: StateWithData<i32, i32> = StateWithData::Idle;
        assert_eq!(format!("{idle:?}"), "Idle");

        let pending: StateWithData<i32, i32> = StateWithData::Pending;
        assert_eq!(format!("{pending:?}"), "Pending");

        let finished: StateWithData<i32, i32> = StateWithData::Finished(&10);
        assert_eq!(format!("{finished:?}"), "Finished(10)");

        let failed: StateWithData<i32, i32> = StateWithData::Failed(&500);
        assert_eq!(format!("{failed:?}"), "Failed(500)");

        // Cover Bind Debug (Idle/None)
        let b_empty: Bind<i32, i32> = Bind::default();
        let dbg = format!("{b_empty:?}");
        assert!(dbg.contains("state: Idle"));
        // Debug for &str adds quotes, so we look for "None" with quotes
        assert!(dbg.contains("data: \"None\""));
        assert!(dbg.contains("in_flight: \"None\""));

        // Cover Bind Debug (Finished/Some)
        let mut b_full: Bind<i32, ()> = Bind::new(false);
        b_full.fill(Ok(42));
        let dbg_full = format!("{b_full:?}");
        assert!(dbg_full.contains("state: Finished"));
        assert!(dbg_full.contains("data: \"Some(...)\""));

        // Cover Bind Debug (Pending/Recv)
        let mut b_pending: Bind<i32, i32> = Bind::new(false);
        b_pending.request(async { Ok(1) });
        let dbg_pending = format!("{b_pending:?}");
        assert!(dbg_pending.contains("state: Pending"));
        assert!(dbg_pending.contains("in_flight: InFlight"));
    });
}

#[test]
fn getters_and_setters_coverage() {
    with_lock(|| {
        set_time(10.0, 9.0);
        let mut b: Bind<i32, String> = Bind::new(false);

        // Retain setter/getter
        assert!(!b.retain());
        b.set_retain(true);
        assert!(b.retain());

        // Read accessors
        b.fill(Ok(100));
        assert_eq!(b.read_as_ref(), Some(Ok(&100)));
        assert!(b.read_mut().is_some());

        // read_as_mut (Ok)
        if let Some(Ok(val)) = b.read_as_mut() {
            *val += 1;
        }
        assert_eq!(b.ok_ref(), Some(&101));
        assert!(b.err_ref().is_none());

        // Reset state before filling again to avoid "Cannot fill a Bind that is not Idle" panic
        b.clear();

        // read_as_mut (Err)
        b.fill(Err("error".to_string()));
        if let Some(Err(e)) = b.read_as_mut() {
            e.push('!');
        }
        assert_eq!(b.err_ref(), Some(&"error!".to_string()));
        assert!(b.ok_ref().is_none());

        // take_ok (Err case)
        // b is currently Err. take_ok should return None and leave data as Err.
        assert!(b.take_ok().is_none());
        assert!(b.is_err());

        // Reset state before filling again
        b.clear();

        // take_ok (Ok case)
        b.fill(Ok(999));
        assert_eq!(b.take_ok(), Some(999));
        assert!(b.is_idle()); // take_ok resets to idle
    });
}

#[test]
fn retain_true_does_not_clear() {
    with_lock(|| {
        set_time(0.0, -1.0);
        let mut b: Bind<i32, ()> = Bind::new(true); // Retain = true
        b.fill(Ok(1));

        // Frame 1: Draw it
        b.poll();
        assert!(b.is_finished());

        // Frame 2: Simulate "Skip" (don't poll)
        // Frame 3: Poll.
        // Because we skipped Frame 2, `was_drawn_last_frame` would be false if we were at Frame 3 checking Frame 2.
        // However, we need to manually manipulate globals to simulate the gap.

        // Setup: Last drawn was frame 0.0.
        // Current Global is now 2.0.
        set_time(2.0, 1.0);

        // Bind thinks last drawn was 0.0.
        // Current is 2.0.
        // It was NOT drawn at 1.0.

        b.poll(); // Should NOT clear because retain is true.
        assert!(b.is_finished());
        assert!(b.read().is_some());
    });
}

#[test]
fn poll_early_exit_optimization() {
    with_lock(|| {
        set_time(10.0, 9.0);
        let mut b: Bind<i32, ()> = Bind::new(false);
        b.fill(Ok(1));

        // First poll updates internal timestamps
        b.poll();
        assert!(b.was_drawn_this_frame());

        // Mutate the data manually to detect if poll resets anything or if logic runs
        // (In reality we just want to ensure the line `curr_frame == self.drawn_time_last` is hit)
        // We can verify this by checking that `drawn_time_prev` does not change if we poll again same frame.

        let prev = b.was_drawn_last_frame();
        b.poll(); // Early exit
        assert_eq!(b.was_drawn_last_frame(), prev);
    });
}

#[test]
fn on_finished_callback() {
    with_lock(|| {
        set_time(10.0, 9.0);
        let mut b: Bind<i32, ()> = Bind::new(false);

        // Not finished yet
        let mut called = false;
        b.on_finished(|_| called = true);
        assert!(!called);

        // Finish it now
        b.fill(Ok(10));
        // `fill` sets completion time to NOW (10.0).

        b.on_finished(|res| {
            called = true;
            assert_eq!(res.as_ref().expect("should be ok"), &10);
        });
        assert!(called);

        // Move to next frame, callback should not fire
        set_time(11.0, 10.0);
        called = false;
        b.on_finished(|_| called = true);
        assert!(!called);
    });
}

#[test]
fn state_method_unreachable_guard() {
    let mut b: Bind<i32, i32> = Bind::new(true);
    assert!(matches!(b.state(), StateWithData::Idle));

    b.request(std::future::pending::<Result<i32, i32>>());
    assert!(matches!(b.state(), StateWithData::Pending));
}

#[test]
fn read_mut_or_request_logic() {
    with_lock(|| {
        set_time(0.0, -1.0);
        let mut b: Bind<i32, ()> = Bind::new(true);

        // 1. Idle -> Request
        let res = b.read_mut_or_request(std::future::pending::<Result<i32, ()>>);
        assert!(res.is_none());
        assert!(b.is_pending());

        // 2. Reset to allow filling
        b.clear();

        // 3. Finish (Mock finish)
        b.fill(Ok(10));

        // 4. Finished -> Return Mut
        if let Some(Ok(val)) = b.read_mut_or_request(|| async { panic!("Should not run") }) {
            *val = 20;
        }

        assert_eq!(b.ok_ref(), Some(&20));
    });
}

#[test]
fn poll_handle_closed_channel() {
    // Simulate the channel closing (future dropped/panicked)
    with_lock(|| {
        // We need to bump the frame every time we poll in the loop, otherwise poll() early exits.
        let mut curr_time = 5.0;
        set_time(curr_time, 4.0);

        let mut b: Bind<i32, ()> = Bind::new(false);

        // We start a request that panics, dropping the sender side.
        b.request(async {
            panic!("Force drop sender");
        });

        // Loop with timeout waiting for the state to become Idle.
        let start = std::time::Instant::now();
        loop {
            assert!(
                start.elapsed() <= std::time::Duration::from_secs(2),
                "Timed out waiting for Bind to handle closed channel"
            );

            // Advance frame to bypass "early exit" optimization in poll()
            curr_time += 1.0;
            CURR_FRAME.store(curr_time, std::sync::atomic::Ordering::Relaxed);

            b.poll();

            if b.get_state() == State::Idle {
                break;
            }

            // Brief sleep to allow runtime to process
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert_eq!(b.get_state(), State::Idle);
        assert!(b.read().is_none());
    });
}

#[allow(clippy::float_cmp)]
#[tokio::test]
async fn test_fill_from_idle() {
    let mut bind: Bind<i32, ()> = Bind::new(true);

    // Action: Fill from Idle
    bind.fill(Ok(42));

    // Assertions
    assert!(bind.is_finished());
    assert_eq!(bind.read_as_ref(), Some(Ok(&42)));

    assert_eq!(
        bind.get_elapsed(),
        0.0,
        "Elapsed time for immediate fill should be 0.0"
    );
}

#[tokio::test]
async fn test_fill_overwrites_pending_and_aborts() {
    let mut bind: Bind<i32, ()> = Bind::new(true);

    // Setup: Start a long-running request
    bind.request(async {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        Ok(999)
    });

    assert!(bind.is_pending(), "Bind should be pending initially");

    // Action: Interrupt Pending with fill
    // This would previously PANIC. Now it should safely overwrite.
    bind.fill(Ok(100));

    // Assertions
    assert!(bind.is_finished(), "State should be Finished");
    assert!(!bind.is_pending(), "Pending state should be cleared");
    assert_eq!(
        bind.read_as_ref(),
        Some(Ok(&100)),
        "Data should be updated to filled value"
    );

    // Verify internal cleanup
    bind.poll();
    assert_eq!(
        bind.read_as_ref(),
        Some(Ok(&100)),
        "Old task result must not overwrite filled data"
    );
}

#[allow(clippy::float_cmp)]
#[tokio::test]
async fn test_fill_overwrites_finished() {
    let mut bind: Bind<i32, ()> = Bind::new(true); // Retain=true to keep old data

    // Setup: Pre-fill with data
    bind.fill(Ok(1));
    assert_eq!(bind.read_as_ref(), Some(Ok(&1)));

    // Action: Overwrite existing data
    bind.fill(Ok(2));

    // Assertions
    assert!(bind.is_finished());
    assert_eq!(bind.read_as_ref(), Some(Ok(&2)));
    assert_eq!(
        bind.get_start_time(),
        bind.get_complete_time(),
        "Fill should update start and complete times to the same instant"
    );
}

#[allow(clippy::float_cmp)]
#[tokio::test]
async fn test_fill_updates_timestamps() {
    let mut bind: Bind<&str, ()> = Bind::new(false);

    // Initial Frame
    advance_frame(1.0);
    let start_time = CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed);

    bind.fill(Ok("test"));

    assert_eq!(bind.get_start_time(), start_time);
    assert_eq!(bind.get_complete_time(), start_time);
    assert_eq!(bind.since_started(), 0.0);

    // Advance Frame
    advance_frame(0.5);
    assert_eq!(bind.since_started(), 0.5);
    assert_eq!(bind.since_completed(), 0.5);
}
