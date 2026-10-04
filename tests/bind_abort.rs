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
mod wasm_tests {
    use std::{cell::Cell, rc::Rc};

    use egui_async::Bind;
    use tokio::sync::oneshot;
    use wasm_bindgen_futures::{JsFuture, js_sys::Promise, wasm_bindgen::JsValue};
    use wasm_bindgen_test::wasm_bindgen_test;

    struct DropTracker(Rc<Cell<bool>>);

    impl Drop for DropTracker {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    #[derive(Clone, Default)]
    struct Flags {
        started: Rc<Cell<bool>>,
        completed: Rc<Cell<bool>>,
        dropped: Rc<Cell<bool>>,
    }

    /// A task that holds a drop guard and only completes once `gate` is released.
    async fn gated_task(flags: Flags, gate: oneshot::Receiver<()>) -> Result<(), ()> {
        let _guard = DropTracker(flags.dropped.clone());
        flags.started.set(true);
        gate.await.map_err(|_| ())?;
        flags.completed.set(true);
        Ok(())
    }

    /// Yields to the event loop, letting spawned local tasks run.
    async fn tick() {
        let _ = JsFuture::from(Promise::resolve(&JsValue::UNDEFINED)).await;
    }

    async fn wait_for(flag: &Cell<bool>) -> bool {
        for _ in 0..100 {
            if flag.get() {
                return true;
            }
            tick().await;
        }
        flag.get()
    }

    async fn settle() {
        for _ in 0..100 {
            tick().await;
        }
    }

    #[wasm_bindgen_test]
    async fn abort_drops_task_without_completing() {
        let flags = Flags::default();
        let (gate_tx, gate_rx) = oneshot::channel();
        let mut b: Bind<(), ()> = Bind::new(true);
        b.set_abort(true);

        b.request(gated_task(flags.clone(), gate_rx));
        assert!(wait_for(&flags.started).await, "Task failed to start");

        b.abort();
        assert!(b.is_idle());
        assert!(
            wait_for(&flags.dropped).await,
            "Task was not physically aborted"
        );

        // Releasing the gate cannot resume a dropped task.
        assert!(gate_tx.send(()).is_err(), "Task still holds the gate");
        settle().await;
        assert!(!flags.completed.get(), "Aborted task ran to completion");
    }

    #[wasm_bindgen_test]
    async fn request_replaces_and_aborts_previous() {
        let flags = Flags::default();
        let (_gate_tx, gate_rx) = oneshot::channel();
        let mut b: Bind<(), ()> = Bind::new(true);
        b.set_abort(true);

        b.request(gated_task(flags.clone(), gate_rx));
        assert!(wait_for(&flags.started).await, "First task failed to start");

        // This request triggers abort() on the first one
        b.request(async { Ok(()) });

        assert!(
            wait_for(&flags.dropped).await,
            "Previous task was not aborted on new request"
        );
        assert!(!flags.completed.get(), "Aborted task ran to completion");
    }

    #[wasm_bindgen_test]
    async fn abort_without_flag_lets_task_complete() {
        let flags = Flags::default();
        let (gate_tx, gate_rx) = oneshot::channel();
        let mut b: Bind<(), ()> = Bind::new(true);

        b.request(gated_task(flags.clone(), gate_rx));
        assert!(wait_for(&flags.started).await, "Task failed to start");

        b.abort();
        assert!(b.is_idle());
        settle().await;
        assert!(!flags.dropped.get(), "Task was aborted without the flag");

        gate_tx.send(()).expect("Task no longer holds the gate");
        assert!(
            wait_for(&flags.completed).await,
            "Task did not run to completion"
        );
        assert!(flags.dropped.get());

        // The result is discarded: the Bind stays idle without data.
        assert!(b.is_idle());
        assert!(b.read().is_none());
    }

    #[wasm_bindgen_test]
    async fn dropped_bind_lets_task_complete() {
        let flags = Flags::default();
        let (gate_tx, gate_rx) = oneshot::channel();
        let mut b: Bind<(), ()> = Bind::new(true);
        b.set_abort(true);

        b.request(gated_task(flags.clone(), gate_rx));
        assert!(wait_for(&flags.started).await, "Task failed to start");

        drop(b);
        settle().await;
        assert!(
            !flags.dropped.get(),
            "Task was aborted by dropping the Bind"
        );

        gate_tx.send(()).expect("Task no longer holds the gate");
        assert!(
            wait_for(&flags.completed).await,
            "Task did not run to completion"
        );
    }
}
