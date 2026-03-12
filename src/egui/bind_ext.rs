//! Extension traits and helper functions for integrating `egui-async` logic ergonomically.

use std::fmt::Debug;

use super::widgets::ErrorPopup;
use crate::bind::{Bind, MaybeSend, State};

impl<T: 'static, E: Debug + 'static> Bind<T, E> {
    /// Reads the data if available, otherwise shows an error popup if there was an error.
    /// If there was an error, the popup will have a "Retry" button that will trigger the given future.
    /// If the data is not available, returns `None`.
    /// This does NOT automatically request the data if it is not available.
    pub fn read_or_error<Fut>(&mut self, f: impl FnOnce() -> Fut, ui: &mut egui::Ui) -> Option<&T>
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend,
        E: MaybeSend,
    {
        self.poll();

        if let Some(Err(e)) = &self.data {
            if ErrorPopup::new(&format!("{e:?}")).show(ui.ctx()) {
                self.request(f());
            }
            None
        } else if let Some(Ok(data)) = self.data.as_ref() {
            Some(data)
        } else {
            None
        }
    }

    /// Reads the data mutably if available, otherwise shows an error popup if there was an error.
    pub fn read_mut_or_error<Fut>(
        &mut self,
        f: impl FnOnce() -> Fut,
        ui: &mut egui::Ui,
    ) -> Option<&mut T>
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend,
        E: MaybeSend,
    {
        self.poll();

        if let Some(Err(e)) = &self.data {
            if ErrorPopup::new(&format!("{e:?}")).show(ui.ctx()) {
                self.request(f());
            }
            None
        } else if let Some(Ok(data)) = self.data.as_mut() {
            Some(data)
        } else {
            None
        }
    }

    /// Reads the data if available, otherwise requests it using the given future.
    /// If there was an error, the popup will have a "Retry" button that will trigger the given future.
    /// If the data is not available, returns `None`.
    pub fn read_or_request_or_error<Fut>(
        &mut self,
        f: impl FnOnce() -> Fut,
        ui: &mut egui::Ui,
    ) -> Option<&T>
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend,
        E: MaybeSend,
    {
        self.poll();

        if matches!(self.state, State::Idle) {
            self.request(f());
            None
        } else if let Some(Err(e)) = &self.data {
            if ErrorPopup::new(&format!("{e:?}")).show(ui.ctx()) {
                self.request(f());
            }
            None
        } else if let Some(Ok(data)) = self.data.as_ref() {
            Some(data)
        } else {
            None
        }
    }

    /// Reads the data mutably if available, otherwise requests it using the given future.
    pub fn read_mut_or_request_or_error<Fut>(
        &mut self,
        f: impl FnOnce() -> Fut,
        ui: &mut egui::Ui,
    ) -> Option<&mut T>
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend,
        E: MaybeSend,
    {
        self.poll();

        if matches!(self.state, State::Idle) {
            self.request(f());
            None
        } else if let Some(Err(e)) = &self.data {
            if ErrorPopup::new(&format!("{e:?}")).show(ui.ctx()) {
                self.request(f());
            }
            None
        } else if let Some(Ok(data)) = self.data.as_mut() {
            Some(data)
        } else {
            None
        }
    }
}
