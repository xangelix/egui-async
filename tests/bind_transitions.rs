//! Core state machine tests for `egui-async::Bind`.

use egui_async::{
    Bind, State, StateWithData,
    bind::{CURR_FRAME, LAST_FRAME},
};

use std::sync::{Mutex, OnceLock};

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn with_lock<T>(f: impl FnOnce() -> T) -> T {
    // Be resilient to a previously-poisoned lock so one failure doesn't cascade.
    let _keep = match test_lock().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    f()
}

fn set_frame_times(curr: f64, last: f64) {
    CURR_FRAME.store(curr, std::sync::atomic::Ordering::Relaxed);
    LAST_FRAME.store(last, std::sync::atomic::Ordering::Relaxed);
}

fn bump_frame<T: 'static, E: 'static>(b: &mut Bind<T, E>) {
    // Advance to a new egui frame and poll the bind once.
    let curr = CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed);
    LAST_FRAME.store(curr, std::sync::atomic::Ordering::Relaxed);
    CURR_FRAME.store(curr + 1.0, std::sync::atomic::Ordering::Relaxed);
    b.poll();
}

fn drive_until_finished<T: 'static, E: 'static>(b: &mut Bind<T, E>, max_frames: usize) -> bool {
    use std::{thread, time::Duration};
    for _ in 0..max_frames {
        bump_frame(b);
        if b.is_finished() {
            return true;
        }
        // Give Tokio worker(s) a tiny slice to run; makes tests deterministic.
        thread::sleep(Duration::from_millis(2));
    }
    false
}

/// Basic happy-path: request → Pending → Finished(Ok) → `take()` → Idle.
#[test]
fn request_ok_flow() {
    with_lock(|| {
        set_frame_times(0.0, -1.0);
        let mut b: Bind<u32, String> = Bind::new(false);

        assert_eq!(b.get_state(), State::Idle);
        assert!(b.read().is_none());

        // Start a request that immediately succeeds.
        b.request(async { Ok::<_, String>(42) });
        assert_eq!(b.get_state(), State::Pending);
        assert!(!b.is_finished());

        // Advance frames until the background future result is received.
        assert!(
            drive_until_finished(&mut b, 200),
            "future did not finish in time"
        );

        // Verify finished data and timing flags.
        assert!(b.is_finished());
        assert!(b.just_completed());

        let r = b.read().as_ref().expect("expected Some(..)");
        assert_eq!(r, &Ok(42));

        // take() consumes and returns the data, then resets to Idle.
        let taken = b.take();
        assert!(matches!(taken, Some(Ok(42))));
        assert!(b.is_idle());
        assert!(b.read().is_none());

        // Execution counter is incremented.
        assert_eq!(b.count_executed(), 1);
    });
}

/// Error path: request → Pending → Finished(Err).
#[test]
fn request_err_flow() -> Result<(), String> {
    with_lock(|| {
        set_frame_times(0.0, -1.0);
        let mut b: Bind<u32, String> = Bind::default();

        b.request(async { Err::<u32, _>("nope".to_string()) });
        assert_eq!(b.get_state(), State::Pending);

        assert!(drive_until_finished(&mut b, 200));

        let StateWithData::Failed(e) = b.state() else {
            return Err("expected Failed(..) state".to_string());
        };
        assert_eq!(e, "nope");

        assert!(b.is_err());
        assert!(!b.is_ok());
        Ok(())
    })
}

/// `just_started` is true only in the start frame; `just_completed` only in the completion frame.
#[test]
fn just_started_and_just_completed_flags() {
    with_lock(|| {
        set_frame_times(5.0, 4.0);
        let mut b: Bind<&'static str, &'static str> = Bind::new(true);

        // Start the request on frame 5
        b.request(async { Ok::<_, _>("done") });
        assert!(b.just_started());

        // Next frame: no longer "just started"
        bump_frame(&mut b);
        assert!(!b.just_started());

        // Complete and check "just completed" at the completion frame
        assert!(drive_until_finished(&mut b, 200));
        assert!(b.just_completed());

        // Next frame: the flag resets
        bump_frame(&mut b);
        assert!(!b.just_completed());
    });
}

/// `refresh()` clears and immediately restarts.
#[test]
fn refresh_restarts_and_replaces_data() {
    with_lock(|| {
        set_frame_times(0.0, -1.0);
        let mut b: Bind<i32, String> = Bind::new(false);

        // Seed with finished data.
        b.fill(Ok(7));
        assert!(b.is_finished());
        assert!(matches!(b.read(), Some(Ok(7))));

        // refresh -> Pending immediately / data cleared
        b.refresh(async { Ok::<_, String>(99) });
        assert_eq!(b.get_state(), State::Pending);
        assert!(b.read().is_none());

        assert!(drive_until_finished(&mut b, 200));
        assert!(matches!(b.read(), Some(Ok(99))));
    });
}

/// `take()` consumes finished data and resets to Idle.
#[test]
fn take_consumes_and_resets() {
    with_lock(|| {
        set_frame_times(0.0, -1.0);
        let mut b: Bind<String, String> = Bind::default();

        b.fill(Ok("hello".to_string()));
        let got = b.take();
        assert!(matches!(got, Some(Ok(s)) if s == "hello"));
        assert!(b.is_idle());
        assert!(b.read().is_none());
    });
}

/// `read_or_request` starts a request when empty and returns None until completion.
#[test]
fn read_or_request_semantics() {
    with_lock(|| {
        set_frame_times(0.0, -1.0);
        let mut b: Bind<&'static str, &'static str> = Bind::new(false);

        // First call triggers a request and returns None.
        let first = b.read_or_request(|| async { Ok::<_, _>("val") });
        assert!(first.is_none());
        assert_eq!(b.get_state(), State::Pending);

        // Finish the request.
        assert!(drive_until_finished(&mut b, 200));

        // Now it returns Some(&Result). Compare using as_deref() for clean equality.
        let read = b
            .read_or_request(|| async { unreachable!() })
            .expect("expected Some after completion");
        assert_eq!(read.as_deref(), Ok("val"));
    });
}

/// `read_as_mut` allows in-place mutation of successful data.
#[test]
fn read_as_mut_allows_mutation() -> Result<(), String> {
    with_lock(|| {
        set_frame_times(0.0, -1.0);
        let mut b: Bind<String, String> = Bind::default();
        b.fill(Ok("abc".into()));

        let Some(Ok(s)) = b.read_as_mut() else {
            return Err("expected Some(Ok(_))".to_string());
        };
        s.push_str("123");

        assert!(matches!(b.read(), Some(Ok(s)) if s == "abc123"));
        Ok(())
    })
}

/// When `retain=false`, skipping a frame where the bind is not drawn clears data.
#[test]
fn retain_false_clears_when_not_drawn_last_frame() {
    with_lock(|| {
        // Start at frame 0
        set_frame_times(0.0, -1.0);
        let mut b: Bind<i32, String> = Bind::new(false);

        // Fill finished data at frame 0.
        b.fill(Ok(123));
        assert!(b.is_finished());
        assert!(matches!(b.read(), Some(Ok(123))));

        // Simulate *skipping* polling in frame 1 and then polling in frame 2.
        LAST_FRAME.store(1.0, std::sync::atomic::Ordering::Relaxed);
        CURR_FRAME.store(2.0, std::sync::atomic::Ordering::Relaxed);

        // Now we poll: since retain=false and not drawn last frame, data is cleared.
        b.poll();
        assert!(b.is_idle());
        assert!(b.read().is_none());
    });
}

/// Periodic refresh: initial call triggers immediately (huge `since_completed`),
/// then respects the interval until overdue, at which point it triggers again.
#[test]
fn request_every_sec_behaves_as_timer() {
    with_lock(|| {
        set_frame_times(0.0, -1.0);
        let mut b: Bind<&'static str, String> = Bind::new(false);

        // Initial call should trigger immediately (last_complete_time starts very low).
        let dt = b.request_every_sec(|| async { Ok::<_, String>("tick") }, 10.0);
        assert!(
            dt.is_sign_negative(),
            "expected negative/overdue first interval"
        );
        assert_eq!(b.count_executed(), 1);
        assert_eq!(b.get_state(), State::Pending);

        // Complete it.
        assert!(drive_until_finished(&mut b, 200));

        // In the completion frame:
        let t_until = b.request_every_sec(|| async { Ok::<_, String>("tick2") }, 10.0);
        assert!(
            (9.0..=10.0).contains(&t_until),
            "expected ~10s remaining, got {t_until}"
        );

        // Advance time 11s: should trigger again.
        let now = CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed);
        LAST_FRAME.store(now, std::sync::atomic::Ordering::Relaxed);
        CURR_FRAME.store(now + 11.0, std::sync::atomic::Ordering::Relaxed);
        let overdue = b.request_every_sec(|| async { Ok::<_, String>("tick2") }, 10.0);
        assert!(
            overdue.is_sign_negative(),
            "should be overdue and re-trigger"
        );
        assert_eq!(b.count_executed(), 2);
        assert_eq!(b.get_state(), State::Pending);
    });
}

/// Clearing while pending discards the in-flight result (no transition to Finished).
/// This test uses a gate to delay the future until after `clear()`.
#[cfg(not(target_family = "wasm"))]
#[test]
fn clear_while_pending_discards_result() {
    use tokio::sync::oneshot;

    with_lock(|| {
        use std::{thread, time::Duration};

        set_frame_times(0.0, -1.0);
        let mut b: Bind<&'static str, String> = Bind::new(false);

        // Gate future completion on a oneshot we control.
        let (gate_tx, gate_rx) = oneshot::channel::<()>();
        b.request(async move {
            // Wait until the test tells us to finish:
            let _ = gate_rx.await;
            Ok::<_, String>("late")
        });

        assert_eq!(b.get_state(), State::Pending);

        // Clear immediately: should drop data, reset to Idle.
        b.clear();
        assert!(b.is_idle());
        assert!(b.read().is_none());

        // Let the background future "finish" now.
        let _ = gate_tx.send(());

        // Give the runtime a moment for the send to occur.
        thread::sleep(Duration::from_millis(5));

        // Advance frames; despite the sender completing, the bind must stay Idle with no data.
        for _ in 0..5 {
            bump_frame(&mut b);
            assert!(b.is_idle());
            assert!(b.read().is_none());
        }

        // A new request should work normally.
        b.request(async { Ok::<_, String>("fresh") });
        assert!(drive_until_finished(&mut b, 200));
        assert!(matches!(b.read(), Some(Ok("fresh"))));
    });
}
