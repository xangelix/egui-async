#[cfg(not(target_family = "wasm"))]
mod native_tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use egui_async::Bind;
    use tokio::time::sleep;

    struct DropTracker(Arc<AtomicBool>);

    impl Drop for DropTracker {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    async fn long_running_task(
        started: Arc<AtomicBool>,
        dropped: Arc<AtomicBool>,
    ) -> Result<(), ()> {
        let _guard = DropTracker(dropped);
        started.store(true, Ordering::SeqCst);
        sleep(Duration::from_mins(1)).await;
        Ok(())
    }

    async fn wait_for_condition(flag: &Arc<AtomicBool>, timeout_ms: u64) -> bool {
        for _ in 0..(timeout_ms / 5) {
            if flag.load(Ordering::SeqCst) {
                return true;
            }
            sleep(Duration::from_millis(5)).await;
        }
        false
    }

    #[tokio::test]
    async fn test_explicit_abort_terminates_task() {
        let started = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));
        let mut b: Bind<(), ()> = Bind::new(false);
        b.set_abort(true);

        b.request(long_running_task(started.clone(), dropped.clone()));

        // Ensure the task actually started and is sitting at the sleep() await point
        assert!(
            wait_for_condition(&started, 500).await,
            "Task failed to start"
        );

        b.abort();

        // Now check for the drop
        assert!(
            wait_for_condition(&dropped, 500).await,
            "Task was not physically aborted"
        );
        assert!(b.is_idle());
    }

    #[tokio::test]
    async fn test_request_replaces_and_aborts_previous() {
        let started = Arc::new(AtomicBool::new(false));
        let dropped_first = Arc::new(AtomicBool::new(false));
        let mut b: Bind<(), ()> = Bind::new(false);
        b.set_abort(true);

        b.request(long_running_task(started.clone(), dropped_first.clone()));
        assert!(
            wait_for_condition(&started, 500).await,
            "First task failed to start"
        );

        // This request triggers abort() on the first one
        b.request(async { Ok(()) });

        assert!(
            wait_for_condition(&dropped_first, 500).await,
            "Previous task was not aborted on new request"
        );
    }
}

#[cfg(target_family = "wasm")]
#[test]
fn test_wasm_abort_is_safe_noop() {
    // On WASM, we just want to ensure the API exists and doesn't crash.
    let mut b: egui_async::Bind<(), ()> = egui_async::Bind::new(false);
    b.set_abort(true);
    b.abort();
    assert!(b.is_idle());
}
