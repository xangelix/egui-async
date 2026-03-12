//! Asynchronous widgets for `egui`.
//!
//! All widgets follow the `egui` builder pattern: configure the widget via chained methods,
//! then render it by calling `.show()`.

mod async_button;
mod async_search;
mod async_view;
mod popups;
mod refresh_button;

pub use async_button::AsyncButton;
pub use async_search::AsyncSearch;
pub use async_view::{AsyncView, StateLayout};
pub use popups::{ErrorPopup, NotifyPopup};
pub use refresh_button::RefreshButton;

use crate::bind::{Bind, MaybeSend};

/// Extension trait for [`egui::Ui`] providing backwards-compatible convenience wrappers for widgets.
pub trait UiExt {
    /// Pops up an error window with the given error message.
    ///
    /// # Returns
    /// `true` if the "Retry" button was clicked.
    fn popup_error(&self, error: &str) -> bool;

    /// Pops up a notification window with the given info message.
    ///
    /// # Returns
    /// `true` if the "Ok" button was clicked.
    fn popup_notify(&self, info: &str) -> bool;

    /// Adds a refresh button that triggers a new future when clicked, or on an interval.
    fn refresh_button<T, E, Fut>(
        &mut self,
        bind: &mut Bind<T, E>,
        f: impl FnOnce() -> Fut,
        secs: f64,
    ) where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend + 'static,
        E: MaybeSend + 'static;
}

impl UiExt for egui::Ui {
    fn popup_error(&self, error: &str) -> bool {
        ErrorPopup::new(error).show(self.ctx())
    }

    fn popup_notify(&self, info: &str) -> bool {
        NotifyPopup::new(info).show(self.ctx())
    }

    fn refresh_button<T, E, Fut>(
        &mut self,
        bind: &mut Bind<T, E>,
        f: impl FnOnce() -> Fut,
        secs: f64,
    ) where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend + 'static,
        E: MaybeSend + 'static,
    {
        RefreshButton::new(bind).interval_secs(secs).show(self, f);
    }
}
