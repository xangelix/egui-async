//! A structured container that fully manages the four foundational states of data loading.

use std::fmt::Debug;

use crate::bind::{Bind, MaybeSend, StateWithData};

/// Determines how the intermediate states (Loading, Error) are positioned within the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateLayout {
    /// Centers the content and greedily consumes all available space in the parent `Ui`.
    ///
    /// **Note:** This can cause the UI to snap or shrink abruptly when transitioning
    /// to the `Finished` state if the successful content does not also fill the space.
    FillAndCenter,
    /// Centers the content horizontally, consuming only the vertical space required.
    ///
    /// This prevents the surrounding UI from aggressively shifting and is the recommended default.
    #[default]
    CenterHorizontal,
    /// Lays out the state content inline, directly following the parent `Ui`'s standard flow.
    Inline,
}

/// A standardized widget that exhaustively handles the `Idle`, `Pending`,
/// `Failed`, and `Finished` states of a data fetch.
///
/// Use this to wrap the core visual components of your app that rely on external data.
#[must_use = "You should call .show() on this widget to render it"]
pub struct AsyncView<'a, T, E> {
    bind: &'a mut Bind<T, E>,
    loading_text: String,
    error_retry_text: String,
    state_layout: StateLayout,
}

impl<'a, T, E> AsyncView<'a, T, E> {
    /// Constructs a new `AsyncView`.
    pub fn new(bind: &'a mut Bind<T, E>) -> Self {
        Self {
            bind,
            loading_text: "Loading...".to_string(),
            error_retry_text: "Retry".to_string(),
            state_layout: StateLayout::default(),
        }
    }

    /// Sets the text to display below the spinner when the fetch is pending.
    pub fn loading_text(mut self, text: impl Into<String>) -> Self {
        self.loading_text = text.into();
        self
    }

    /// Sets the text to display on the retry button when the fetch fails.
    pub fn error_retry_text(mut self, text: impl Into<String>) -> Self {
        self.error_retry_text = text.into();
        self
    }

    /// Configures the layout strategy for the intermediate loading and error states.
    pub const fn state_layout(mut self, layout: StateLayout) -> Self {
        self.state_layout = layout;
        self
    }

    /// Runs the state machine. If data is successfully loaded, it invokes `on_ok`
    /// to let you render your successful data.
    ///
    /// # Returns
    /// `Some(R)` if the data is available and successfully rendered, otherwise `None`.
    pub fn show<Fut, R>(
        self,
        ui: &mut egui::Ui,
        fetch: impl FnOnce() -> Fut,
        on_ok: impl FnOnce(&mut egui::Ui, &T) -> R,
    ) -> Option<R>
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend + 'static,
        E: Debug + MaybeSend + 'static,
    {
        let mut should_clear = false;
        let mut ret = None;

        match self.bind.state_or_request(fetch) {
            StateWithData::Idle | StateWithData::Pending => {
                Self::apply_layout(self.state_layout, ui, |ui| {
                    ui.spinner();
                    ui.add_space(8.0);
                    ui.label(&self.loading_text);
                });
            }
            StateWithData::Finished(data) => {
                ret = Some(Self::apply_layout(self.state_layout, ui, |ui| {
                    on_ok(ui, data)
                }));
            }
            StateWithData::Failed(err) => {
                Self::apply_layout(self.state_layout, ui, |ui| {
                    ui.label(
                        egui::RichText::new("⚠ Request Failed")
                            .color(ui.visuals().error_fg_color)
                            .size(16.0)
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(format!("{err:?}"));
                    ui.add_space(12.0);

                    if ui.button(&self.error_retry_text).clicked() {
                        should_clear = true;
                    }
                });
            }
        }

        // Apply mutation safely after the match block drops its borrow on `self.bind`
        if should_clear {
            self.bind.clear(); // Will trigger fetch on next frame
        }

        ret
    }

    /// Safely applies the configured `StateLayout` to the given closure without borrow conflicts.
    fn apply_layout<R>(
        layout: StateLayout,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        match layout {
            StateLayout::FillAndCenter => {
                ui.centered_and_justified(|ui| ui.vertical_centered(add_contents).inner)
                    .inner
            }
            StateLayout::CenterHorizontal => ui.vertical_centered(add_contents).inner,
            StateLayout::Inline => add_contents(ui),
        }
    }
}
