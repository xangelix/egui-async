//! Tests for choosing the Tokio runtime that `Bind` tasks are spawned onto.
//!
//! The global runtime can only be set once per process, so only `global_runtime_precedence`
//! sets it, and the other tests pass whether or not it has been set.

#![cfg(not(target_family = "wasm"))]

use std::{
    sync::{LazyLock, Mutex, MutexGuard, OnceLock, atomic::Ordering::Relaxed},
    thread,
    time::Duration,
};

use egui_async::{
    Bind,
    bind::{CURR_FRAME, LAST_FRAME},
};
use tokio::runtime::{Builder, Runtime};

/// Serializes tests, since they share the global frame timers.
fn test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    // Be resilient to a previously-poisoned lock so one failure doesn't cascade.
    match LOCK.get_or_init(|| Mutex::new(())).lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Builds a single-worker runtime whose worker thread has the given name.
fn named_runtime(name: &str) -> Runtime {
    Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name(name)
        .build()
        .expect("failed to build runtime")
}

/// The runtime set as the global runtime. It lives for the whole process, as it must.
static GLOBAL: LazyLock<Runtime> = LazyLock::new(|| named_runtime("global-rt"));

/// The name of the built-in runtime's worker threads, which Tokio chooses.
fn default_thread() -> Option<String> {
    let runtime = &*egui_async::bind::ASYNC_RUNTIME;
    let task = runtime.spawn(async { thread::current().name().map(str::to_owned) });
    runtime
        .block_on(task)
        .expect("task on built-in runtime failed")
}

type ThreadBind = Bind<Option<String>, ()>;

/// Requests the name of the thread the task runs on, driving frames until the result arrives.
///
/// Returns `None` if the request did not finish, e.g., because it was cancelled.
fn thread_of(b: &mut ThreadBind) -> Option<String> {
    b.request(async { Ok(thread::current().name().map(str::to_owned)) });
    for _ in 0..500 {
        // Advance a frame so `poll` checks for the result again.
        let curr = CURR_FRAME.load(Relaxed);
        LAST_FRAME.store(curr, Relaxed);
        CURR_FRAME.store(curr + 1.0, Relaxed);

        if !b.is_pending() {
            return b.take().and_then(Result::ok).flatten();
        }
        thread::sleep(Duration::from_millis(2));
    }
    None
}

#[test]
fn global_runtime_precedence() {
    let _lock = test_lock();
    let mut b = ThreadBind::new(true);
    assert!(b.runtime().is_none());

    // Without any override, tasks run on the built-in runtime.
    assert_eq!(thread_of(&mut b), default_thread());

    // Setting the global runtime moves later requests of existing and new Binds onto it.
    assert!(egui_async::set_global_runtime(GLOBAL.handle().clone()).is_ok());
    assert_eq!(thread_of(&mut b).as_deref(), Some("global-rt"));
    assert_eq!(
        thread_of(&mut ThreadBind::new(true)).as_deref(),
        Some("global-rt")
    );

    // It can only be set once.
    let other = named_runtime("other-rt");
    assert!(egui_async::set_global_runtime(other.handle().clone()).is_err());
    assert_eq!(thread_of(&mut b).as_deref(), Some("global-rt"));

    // A Bind's own runtime takes precedence over the global one.
    let own = named_runtime("bind-rt");
    let mut b = ThreadBind::new(true).with_runtime(own.handle().clone());
    assert_eq!(thread_of(&mut b).as_deref(), Some("bind-rt"));

    // Clearing it returns the Bind to the global runtime.
    b.set_runtime(None);
    assert_eq!(thread_of(&mut b).as_deref(), Some("global-rt"));
}

#[test]
fn bind_runtime_is_used_and_can_be_changed() {
    let _lock = test_lock();
    let first = named_runtime("first-rt");
    let second = named_runtime("second-rt");

    let mut b = ThreadBind::new(true).with_runtime(first.handle().clone());
    assert!(b.runtime().is_some());
    assert_eq!(thread_of(&mut b).as_deref(), Some("first-rt"));

    // The runtime survives clearing, like the rest of the Bind's configuration.
    b.clear();
    assert_eq!(thread_of(&mut b).as_deref(), Some("first-rt"));

    b.set_runtime(Some(second.handle().clone()));
    assert_eq!(thread_of(&mut b).as_deref(), Some("second-rt"));

    // Without its own runtime, the Bind uses the global one, whichever that currently is.
    b.set_runtime(None);
    assert!(b.runtime().is_none());
    let global = thread_of(&mut b);
    assert!(
        global.as_deref() == Some("global-rt") || global == default_thread(),
        "unexpected runtime thread: {global:?}"
    );
}

#[test]
fn abort_works_on_bind_runtime() {
    let _lock = test_lock();
    let own = named_runtime("abort-rt");
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (dropped_tx, dropped_rx) = std::sync::mpsc::channel::<()>();

    let mut b: Bind<(), ()> = Bind::new(true).with_runtime(own.handle().clone());
    b.set_abort(true);
    b.request(async move {
        // Signals when the task is dropped, through the sender being dropped.
        let _dropped = dropped_tx;
        started_tx.send(()).ok();
        std::future::pending::<()>().await;
        Ok(())
    });

    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("task did not start");
    b.abort();
    assert_eq!(
        dropped_rx.recv_timeout(Duration::from_secs(5)),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected),
        "task was not aborted"
    );
    assert!(b.is_idle());
}

#[test]
fn request_on_shut_down_runtime_returns_to_idle() {
    let _lock = test_lock();
    let handle = named_runtime("dead-rt").handle().clone();

    // The runtime was dropped above, so the task is cancelled instead of run.
    let mut b = ThreadBind::new(true).with_runtime(handle);
    assert_eq!(thread_of(&mut b), None);
    assert!(b.is_idle());
    assert!(b.read().is_none());
}
