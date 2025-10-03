//! Core state management for asynchronous operations.
//!
//! This module provides the `Bind` struct, which is the heart of `egui-async`. It acts as a
//! state machine to manage the lifecycle of a `Future`, from initiation to completion, and
//! holds the resulting data or error.
use std::{fmt::Debug, future::Future};

use atomic_float::AtomicF64;
use tokio::sync::oneshot;
use tracing::warn;

/// The `egui` time of the current frame, updated by `ContextExt::loop_handle`.
pub static CURR_FRAME: AtomicF64 = AtomicF64::new(0.0);
/// The `egui` time of the previous frame, updated by `ContextExt::loop_handle`.
pub static LAST_FRAME: AtomicF64 = AtomicF64::new(0.0);

/// A lazily initialized Tokio runtime for executing async tasks on non-WASM targets.
#[cfg(not(target_family = "wasm"))]
pub static ASYNC_RUNTIME: std::sync::LazyLock<tokio::runtime::Runtime> =
    std::sync::LazyLock::new(|| {
        tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime.")
    });

/// A global holder for the `egui::Context`, used to request repaints from background tasks.
///
/// This is initialized once by `egui::ContextExt::loop_handle`.
#[cfg(feature = "egui")]
pub static CTX: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();

/// Represents the execution state of an asynchronous operation managed by `Bind`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// No operation is running, and no data is available from a previous run.
    #[default]
    Idle,
    /// An operation is currently in-flight.
    Pending,
    /// An operation has completed, and its result (success or error) is available.
    Finished,
}

/// Represents the detailed state of a `Bind`, including available data.
pub enum StateWithData<'a, T, E> {
    /// No operation is running.
    Idle,
    /// An operation is currently in-flight.
    Pending,
    /// An operation has completed with a successful result.
    Finished(&'a T),
    /// An operation has completed with an error.
    Failed(&'a E),
}

/// A state manager for a single asynchronous operation, designed for use with `egui`.
///
/// `Bind` tracks the lifecycle of a `Future` and stores its `Result<T, E>`. It acts as a
/// bridge between the immediate-mode UI and the background async task, ensuring the UI
/// can react to changes in state (e.g., show a spinner while `Pending`, display the
/// result when `Finished`, or show an error).
pub struct Bind<T, E> {
    /// The `egui` time of the most recent frame where this `Bind` was polled.
    drawn_time_last: f64,
    /// The `egui` time of the second most recent frame where this `Bind` was polled.
    drawn_time_prev: f64,

    /// The result of the completed async operation. `None` if the task is not `Finished`.
    pub(crate) data: Option<Result<T, E>>,
    /// The receiving end of a one-shot channel used to get the result from the background task.
    /// This is `Some` only when the state is `Pending`.
    recv: Option<oneshot::Receiver<Result<T, E>>>,

    /// The current execution state of the async operation.
    pub(crate) state: State,
    /// The `egui` time when the most recent operation was started.
    last_start_time: f64,
    /// The `egui` time when the most recent operation was completed.
    last_complete_time: f64,

    /// If `true`, the `data` from a `Finished` state is preserved even if the `Bind` instance
    /// is not polled for one or more frames. If `false`, the data is cleared.
    retain: bool,

    /// A counter for how many times an async operation has been started.
    times_executed: usize,
}

impl<T, E> Debug for Bind<T, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = f.debug_struct("Bind");
        let mut out = out
            .field("state", &self.state)
            .field("retain", &self.retain)
            .field("drawn_time_last", &self.drawn_time_last)
            .field("drawn_time_prev", &self.drawn_time_prev)
            .field("last_start_time", &self.last_start_time)
            .field("last_complete_time", &self.last_complete_time)
            .field("times_executed", &self.times_executed);

        // Avoid printing the full data/recv content for cleaner debug output.
        if self.data.is_some() {
            out = out.field("data", &"Some(...)");
        } else {
            out = out.field("data", &"None");
        }

        if self.recv.is_some() {
            out = out.field("recv", &"Some(...)");
        } else {
            out = out.field("recv", &"None");
        }

        out.finish()
    }
}

impl<T: 'static, E: 'static> Default for Bind<T, E> {
    /// Creates a default `Bind` instance in an `Idle` state.
    ///
    /// The `retain` flag is set to `false`. This implementation does not require `T` or `E`
    /// to implement `Default`.
    fn default() -> Self {
        Self::new(false)
    }
}

/// A trait alias for `Send` on native targets.
///
/// On WASM, this trait has no bounds, allowing non-`Send` types to be used in `Bind`
/// since WASM is single-threaded.
#[cfg(not(target_family = "wasm"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_family = "wasm"))]
impl<T: Send> MaybeSend for T {}

/// A trait alias with no bounds on WASM targets.
///
/// This allows `Bind` to work with `!Send` futures and data types in a single-threaded
/// web environment.
#[cfg(target_family = "wasm")]
pub trait MaybeSend {}
#[cfg(target_family = "wasm")]
impl<T> MaybeSend for T {}

impl<T: 'static, E: 'static> Bind<T, E> {
    /// Creates a new `Bind` instance with a specific retain policy.
    ///
    /// # Parameters
    /// - `retain`: If `true`, the result of the operation is kept even if the `Bind`
    ///   is not polled in a frame. If `false`, the result is cleared if not polled
    ///   for one frame, returning the `Bind` to an `Idle` state.
    #[must_use]
    pub const fn new(retain: bool) -> Self {
        Self {
            drawn_time_last: 0.0,
            drawn_time_prev: 0.0,
            data: None,
            recv: None,
            state: State::Idle,
            last_start_time: 0.0,
            last_complete_time: f64::MIN, // Set to a very low value to ensure `since_completed` is large initially.
            retain,
            times_executed: 0,
        }
    }

    /// Internal helper to prepare the state and communication channel for a new async request.
    #[allow(clippy::type_complexity)]
    fn prepare_channel(
        &mut self,
    ) -> (
        oneshot::Sender<Result<T, E>>,
        oneshot::Receiver<Result<T, E>>,
    ) {
        self.poll(); // Ensure state is up-to-date before starting.

        self.last_start_time = CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed);
        self.state = State::Pending;

        oneshot::channel()
    }

    /// Internal async function that awaits the user's future and sends the result back.
    async fn req_inner<F>(fut: F, tx: oneshot::Sender<Result<T, E>>)
    where
        F: Future<Output = Result<T, E>> + 'static,
        T: MaybeSend,
    {
        let result = fut.await;
        if matches!(tx.send(result), Ok(())) {
            // If the send was successful, request a repaint to show the new data.
            #[cfg(feature = "egui")]
            if let Some(ctx) = CTX.get() {
                ctx.request_repaint();
            }
        } else {
            // This occurs if the `Bind` was dropped before the future completed.
            warn!("Future result was dropped because the receiver was gone.");
        }
    }

    /// Starts an asynchronous operation if the `Bind` is not already `Pending`.
    ///
    /// The provided future `f` is spawned onto the appropriate runtime (`tokio` for native,
    /// `wasm-bindgen-futures` for WASM). The `Bind` state transitions to `Pending`.
    ///
    /// This method calls `poll()` internally.
    pub fn request<Fut>(&mut self, f: Fut)
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend,
        E: MaybeSend,
    {
        #[cfg(not(target_family = "wasm"))]
        {
            let (tx, rx) = self.prepare_channel();
            ASYNC_RUNTIME.spawn(Self::req_inner(f, tx));
            self.recv = Some(rx);
        }

        #[cfg(target_family = "wasm")]
        {
            let (tx, rx) = self.prepare_channel();
            wasm_bindgen_futures::spawn_local(Self::req_inner(f, tx));
            self.recv = Some(rx);
        }

        self.times_executed += 1;
    }

    /// Requests an operation to run periodically.
    ///
    /// If the `Bind` is not `Pending` and more than `secs` seconds have passed since the
    /// last completion, a new request is started by calling `f`.
    ///
    /// # Returns
    /// The time in seconds remaining until the next scheduled refresh. A negative value
    /// indicates a refresh is overdue.
    pub fn request_every_sec<Fut>(&mut self, f: impl FnOnce() -> Fut, secs: f64) -> f64
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend,
        E: MaybeSend,
    {
        let since_completed = self.since_completed();

        if self.get_state() != State::Pending && since_completed > secs {
            self.request(f());
        }

        secs - since_completed
    }

    /// Clears any existing data and immediately starts a new async operation.
    ///
    /// If an operation was `Pending`, its result will be discarded. The background task is not
    /// cancelled and will run to completion.
    ///
    /// This is a convenience method equivalent to calling `clear()` followed by `request()`.
    pub fn refresh<Fut>(&mut self, f: Fut)
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend,
        E: MaybeSend,
    {
        self.clear();
        self.request(f);
    }

    /// Takes ownership of the result if the operation is `Finished`.
    ///
    /// If the state is `Finished`, this method returns `Some(result)`, consumes the data
    /// internally, and resets the state to `Idle`. If the state is not `Finished`,
    /// it returns `None`.
    ///
    /// This method calls `poll()` internally.
    pub fn take(&mut self) -> Option<Result<T, E>> {
        self.poll();

        if matches!(self.state, State::Finished) {
            assert!(
                self.data.is_some(),
                "State was Finished but data was None. This indicates a bug."
            );
            self.state = State::Idle;
            self.data.take()
        } else {
            None
        }
    }

    /// Manually sets the data and moves the state to `Finished`.
    ///
    /// This can be used to inject data into the `Bind` without running an async operation.
    ///
    /// # Panics
    /// Panics if the current state is not `Idle`.
    pub fn fill(&mut self, data: Result<T, E>) {
        self.poll();

        assert!(
            matches!(self.state, State::Idle),
            "Cannot fill a Bind that is not Idle."
        );

        self.state = State::Finished;
        self.last_complete_time = CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed);
        self.data = Some(data);
    }

    /// Checks if the current state is `Idle`.
    /// This method calls `poll()` internally.
    pub fn is_idle(&mut self) -> bool {
        self.poll();
        matches!(self.state, State::Idle)
    }

    /// Checks if the current state is `Pending`.
    /// This method calls `poll()` internally.
    pub fn is_pending(&mut self) -> bool {
        self.poll();
        matches!(self.state, State::Pending)
    }

    /// Checks if the current state is `Finished`.
    /// This method calls `poll()` internally.
    pub fn is_finished(&mut self) -> bool {
        self.poll();
        matches!(self.state, State::Finished)
    }

    /// Returns `true` if the operation finished during the current `egui` frame.
    /// This method calls `poll()` internally.
    #[allow(clippy::float_cmp)]
    pub fn just_completed(&mut self) -> bool {
        self.poll();
        self.last_complete_time == CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// If the operation just completed this frame, invokes the provided closure with
    /// a reference to the result.
    pub fn on_finished(&mut self, f: impl FnOnce(&Result<T, E>)) {
        if self.just_completed()
            && let Some(ref d) = self.data
        {
            f(d);
        }
    }

    /// Returns `true` if the operation started during the current `egui` frame.
    /// This method calls `poll()` internally.
    #[allow(clippy::float_cmp)]
    pub fn just_started(&mut self) -> bool {
        self.poll();
        self.last_start_time == CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Gets the `egui` time when the operation started.
    /// This method calls `poll()` internally.
    pub fn get_start_time(&mut self) -> f64 {
        self.poll();
        self.last_start_time
    }

    /// Gets the `egui` time when the operation completed.
    /// This method calls `poll()` internally.
    pub fn get_complete_time(&mut self) -> f64 {
        self.poll();
        self.last_complete_time
    }

    /// Gets the duration between the start and completion of the operation.
    /// This method calls `poll()` internally.
    pub fn get_elapsed(&mut self) -> f64 {
        self.poll();
        self.last_complete_time - self.last_start_time
    }

    /// Gets the time elapsed since the operation started.
    /// This method calls `poll()` internally.
    pub fn since_started(&mut self) -> f64 {
        self.poll();
        CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed) - self.last_start_time
    }

    /// Gets the time elapsed since the operation completed.
    /// This method calls `poll()` internally.
    pub fn since_completed(&mut self) -> f64 {
        self.poll();
        CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed) - self.last_complete_time
    }

    /// Returns an immutable reference to the stored data, if any.
    /// This method calls `poll()` internally.
    pub fn read(&mut self) -> &Option<Result<T, E>> {
        self.poll();
        &self.data
    }
    /// Returns an immutable reference in the ref pattern to the stored data, if any.
    /// This method calls `poll()` internally.
    pub fn read_as_ref(&mut self) -> Option<Result<&T, &E>> {
        self.poll();
        self.data.as_ref().map(Result::as_ref)
    }

    /// Returns a mutable reference to the stored data, if any.
    /// This method calls `poll()` internally.
    pub fn read_mut(&mut self) -> &mut Option<Result<T, E>> {
        self.poll();
        &mut self.data
    }
    /// Returns a mutable reference in the ref pattern to the stored data, if any.
    /// This method calls `poll()` internally.
    pub fn read_as_mut(&mut self) -> Option<Result<&mut T, &mut E>> {
        self.poll();
        self.data.as_mut().map(Result::as_mut)
    }

    /// Returns the current `State` of the binding.
    /// This method calls `poll()` internally.
    pub fn get_state(&mut self) -> State {
        self.poll();
        self.state
    }

    /// Returns the ref filled state of the `Bind`, allowing for exhaustive pattern matching.
    ///
    /// This is often the most ergonomic way to display UI based on the `Bind`'s state.
    /// This method calls `poll()` internally.
    ///
    /// # Example
    /// ```ignore
    /// match my_bind.state() {
    ///     StateWithData::Idle => { /* ... */ }
    ///     StateWithData::Pending => { ui.spinner(); }
    ///     StateWithData::Finished(data) => { ui.label(format!("Data: {data:?}")); }
    ///     StateWithData::Failed(err) => { ui.label(format!("Error: {err:?}")); }
    /// }
    /// ```
    pub fn state(&mut self) -> StateWithData<'_, T, E> {
        self.poll();
        match self.state {
            State::Idle => StateWithData::Idle,
            State::Pending => StateWithData::Pending,
            State::Finished => match self.data.as_ref() {
                Some(Ok(data)) => StateWithData::Finished(data),
                Some(Err(err)) => StateWithData::Failed(err),
                None => {
                    // This case should be unreachable due to internal invariants.
                    // If state is Finished, data must be Some.
                    self.state = State::Idle;
                    StateWithData::Idle
                }
            },
        }
    }

    /// Returns the ref filled state or starts a new request if idle.
    ///
    /// This method is an ergonomic way to drive a UI. If the `Bind` is `Idle` and has no
    /// data, it immediately calls the provided closure `f` to start an async operation,
    /// transitioning the state to `Pending`.
    ///
    /// In all cases, it returns the current `StateWithData` for immediate use in a `match`
    /// statement, making it easy to display a loading indicator, the finished data, or an error.
    ///
    /// # Example
    /// ```ignore
    /// // In your UI update function:
    /// match my_bind.state_or_request(fetch_data) {
    ///     StateWithData::Idle => { /* This branch is typically not reached on the first call */ }
    ///     StateWithData::Pending => { ui.spinner(); }
    ///     StateWithData::Finished(data) => { ui.label(format!("Data: {:?}", data)); }
    ///     StateWithData::Failed(err) => { ui.label(format!("Error: {:?}", err)); }
    /// }
    /// ```
    pub fn state_or_request<Fut>(&mut self, f: impl FnOnce() -> Fut) -> StateWithData<'_, T, E>
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend,
        E: MaybeSend,
    {
        self.poll();

        if self.data.is_none() && matches!(self.state, State::Idle) {
            self.request(f());
        }
        self.state()
    }

    /// Clears any stored data and resets the state to `Idle`.
    ///
    /// If an operation was `Pending`, its result will be discarded. The background task is not
    /// cancelled and will run to completion.
    ///
    /// This method calls `poll()` internally.
    pub fn clear(&mut self) {
        self.poll();
        self.state = State::Idle;
        self.data = None;
    }

    /// Returns a reference to the data, or starts a new request if idle.
    ///
    /// If data is already available (`Finished`), it returns a reference to it.
    /// If the state is `Idle` and no data is present, it calls `f` to start a new async
    /// operation and returns `None`.
    /// If `Pending`, it returns `None`.
    ///
    /// This method calls `poll()` internally.
    pub fn read_or_request<Fut>(&mut self, f: impl FnOnce() -> Fut) -> Option<&Result<T, E>>
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend,
        E: MaybeSend,
    {
        self.poll();

        if self.data.is_none() && matches!(self.state, State::Idle) {
            self.request(f());
        }
        self.data.as_ref()
    }

    /// Returns a mutable reference to the data, or starts a new request if idle.
    ///
    /// This is the mutable version of `read_or_request`.
    ///
    /// This method calls `poll()` internally.
    pub fn read_mut_or_request<Fut>(&mut self, f: impl FnOnce() -> Fut) -> Option<&mut Result<T, E>>
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend,
        E: MaybeSend,
    {
        self.poll();

        if self.data.is_none() && matches!(self.state, State::Idle) {
            self.request(f());
        }
        self.data.as_mut()
    }

    /// Drives the state machine. This should be called once per frame before accessing state.
    ///
    /// **Note**: Most other methods on `Bind` call this internally, so you usually don't
    /// need to call it yourself.
    ///
    /// This method performs several key actions:
    /// 1. Checks if a pending future has completed and, if so, updates the state to `Finished`.
    /// 2. Updates internal frame timers used for `retain` logic and time tracking.
    /// 3. If `retain` is `false`, it clears the data if the `Bind` was not polled in the previous frame.
    ///
    /// # Panics
    /// - Panics if the state is `Pending` but the internal receiver is missing. This indicates a bug in `egui-async`.
    /// - Panics if the `oneshot` channel's sender is dropped without sending a value, which would mean the
    ///   spawned task terminated unexpectedly.
    pub fn poll(&mut self) {
        let curr_frame = CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed);

        // Avoid re-polling within the same frame.
        #[allow(clippy::float_cmp)]
        if curr_frame == self.drawn_time_last {
            return;
        }

        // Shift frame times for tracking visibility across frames.
        self.drawn_time_prev = self.drawn_time_last;
        self.drawn_time_last = curr_frame;

        // If `retain` is false and the UI element associated with this `Bind` was not rendered
        // in the previous frame, we clear its data to free resources and ensure a fresh load.
        if !self.retain && !self.was_drawn_last_frame() {
            // Manually clear state to avoid a recursive call to poll() from clear().
            self.state = State::Idle;
            self.data = None;
        }

        if matches!(self.state, State::Pending) {
            match self
                .recv
                .as_mut()
                .expect("BUG: State is Pending but receiver is missing.")
                .try_recv()
            {
                Ok(result) => {
                    self.data = Some(result);
                    self.last_complete_time = CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed);
                    self.state = State::Finished;
                    self.recv = None; // Drop the receiver as it's no longer needed.
                }
                Err(oneshot::error::TryRecvError::Empty) => {
                    // Future is still running, do nothing.
                }
                Err(oneshot::error::TryRecvError::Closed) => {
                    // This is a critical error: the task's sender was dropped without sending a value.
                    // This should only happen if the runtime shuts down unexpectedly.
                    panic!("Async task's sender was dropped without sending a result.");
                }
            }
        }
    }

    /// Checks if this `Bind` has been polled during the current `egui` frame.
    #[allow(clippy::float_cmp)]
    pub fn was_drawn_this_frame(&self) -> bool {
        self.drawn_time_last == CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Checks if this `Bind` was polled during the previous `egui` frame.
    ///
    /// This is used internally to implement the `retain` logic.
    #[allow(clippy::float_cmp)]
    pub fn was_drawn_last_frame(&self) -> bool {
        self.drawn_time_prev == LAST_FRAME.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Returns the total number of times an async operation has been executed.
    pub const fn count_executed(&self) -> usize {
        self.times_executed
    }
}
