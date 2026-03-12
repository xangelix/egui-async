//! A specialized button that manages a periodic background fetch and user-triggered refetches.

use crate::bind::{Bind, MaybeSend};

/// A button widget that tracks an asynchronous `Bind` to provide manual and automated periodic refreshing.
#[must_use = "You should call .show() on this widget to render it"]
pub struct RefreshButton<'a, T, E> {
    bind: &'a mut Bind<T, E>,
    interval_secs: f64,
    debounce_factor: f64,
    text: String,
}

impl<'a, T, E> RefreshButton<'a, T, E> {
    /// Creates a new refresh button attached to a `Bind`.
    pub fn new(bind: &'a mut Bind<T, E>) -> Self {
        Self {
            bind,
            interval_secs: 60.0,
            debounce_factor: 4.0,
            text: "🔄".to_string(),
        }
    }

    /// Sets the automated interval in seconds.
    pub const fn interval_secs(mut self, secs: f64) -> Self {
        self.interval_secs = secs;
        self
    }

    /// Shows the button in the UI. Automatically manages `Pending` visual states.
    pub fn show<Fut>(self, ui: &mut egui::Ui, f: impl FnOnce() -> Fut) -> egui::Response
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend + 'static,
        E: MaybeSend + 'static,
    {
        let is_pending = self.bind.is_pending();
        let resp = ui.add_enabled(!is_pending, egui::Button::new(self.text));

        // Only actually refresh when clicked if the last completion was more than 1/4 of the interval ago
        let diff = if self.bind.since_completed() > self.interval_secs / self.debounce_factor
            && resp.clicked()
            && !is_pending
        {
            self.bind.refresh(f());
            -1.0
        } else {
            self.bind.request_every_sec(f, self.interval_secs)
        };

        let hover_text = if is_pending {
            "Refreshing...".to_string()
        } else if diff < 0.0 {
            "Refreshing now!".to_string()
        } else {
            format!("Refreshing automatically in {diff:.0}s...")
        };

        resp.on_hover_text(hover_text)
    }
}
