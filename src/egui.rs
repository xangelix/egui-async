//! `egui` integration for `egui-async`.
//!
//! This module provides the [`EguiAsyncPlugin`], which is necessary to integrate
//! `egui-async` into an `egui` application.
//!
//! To use `egui-async`, you must register the plugin with your `egui::Context`.
//! The simplest way is to call `ctx.plugin_or_default::<EguiAsyncPlugin>();`
//! once per frame in your application's update loop.

use std::fmt::Debug;

use super::bind::{self, Bind, MaybeSend, State};

/// The plugin that drives `egui-async`'s per-frame updates.
///
/// This plugin **must be registered** with `egui` for `egui-async` to work.
/// It is responsible for updating frame timers and setting the global `egui::Context`
/// so that background tasks can request repaints.
///
/// The easiest way to register it is to call `ctx.plugin_or_default::<EguiAsyncPlugin>();`
/// in your `eframe::App::update` method or equivalent. `egui` ensures this is a
/// cheap, idempotent operation.
#[derive(Default)]
pub struct EguiAsyncPlugin;

impl egui::Plugin for EguiAsyncPlugin {
    fn debug_name(&self) -> &'static str {
        "egui_async"
    }

    fn on_begin_pass(&mut self, ctx: &egui::Context) {
        // This logic was previously in `ContextExt::loop_handle`.
        // It's responsible for updating the frame timers and ensuring the global
        // context is set for background tasks to request repaints.
        bind::CTX.get_or_init(|| ctx.clone());
        let time = ctx.input(|i| i.time);

        let last_frame = bind::CURR_FRAME.swap(time, std::sync::atomic::Ordering::Relaxed);
        bind::LAST_FRAME.store(last_frame, std::sync::atomic::Ordering::Relaxed);
    }
}

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
            let error_string = format!("{e:?}");
            if ui.popup_error(&error_string) {
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
    /// If there was an error, the popup will have a "Retry" button that will
    /// trigger the given future.
    /// If the data is not available, returns `None`.
    /// This does NOT automatically request the data if it is not available.
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
            let error_string = format!("{e:?}");
            if ui.popup_error(&error_string) {
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
    /// If there was an error, the popup will have a "Retry" button that will
    /// trigger the given future.
    /// If the data is not available, returns `None`.
    /// This automatically requests the data if it is not available.
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
            let error_string = format!("{e:?}");
            if ui.popup_error(&error_string) {
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
    /// If there was an error, the popup will have a "Retry" button that will
    /// trigger the given future.
    /// If the data is not available, returns `None`.
    /// This automatically requests the data if it is not available.
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
            let error_string = format!("{e:?}");
            if ui.popup_error(&error_string) {
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

/// Extension trait for [`egui::Ui`] providing convenient UI widgets.
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

    /// Adds a refresh button that triggers a new future when clicked.
    ///
    /// This widget also handles periodic refreshing.
    ///
    /// The button shows a tooltip with the time remaining until the next automatic refresh.
    /// If the button is clicked, it triggers an immediate refresh, unless the last refresh
    /// was less than `secs / 4.0` seconds ago to prevent spamming.
    fn refresh_button<T, E, Fut>(
        &mut self,
        bind: &mut bind::Bind<T, E>,
        f: impl FnOnce() -> Fut,
        secs: f64,
    ) where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend + 'static,
        E: MaybeSend + 'static;
}

const REFRESH_DEBOUNCE_FACTOR: f64 = 4.0;

impl UiExt for egui::Ui {
    fn popup_error(&self, error: &str) -> bool {
        let screen_rect = self.ctx().content_rect();
        let total_width = screen_rect.width();
        let total_height = screen_rect.height();

        let id = self.id().with("error_window");
        egui::Window::new("Error")
            .id(id)
            .collapsible(false)
            .default_width(total_width * 0.25)
            .default_height(total_height * 0.20)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(self.ctx(), |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(error).color(egui::Color32::RED));

                    ui.add_space(10.0);

                    ui.label("Please retry the request, or contact support if the error persists.");

                    ui.add_space(10.0);

                    ui.button("Retry").clicked()
                })
                .inner
            })
            .is_some_and(|r| r.inner.is_some_and(|r| r))
    }
    fn popup_notify(&self, info: &str) -> bool {
        let screen_rect = self.ctx().content_rect();
        let total_width = screen_rect.width();
        let total_height = screen_rect.height();

        let id = self.id().with("notify_window");
        egui::Window::new("Info")
            .id(id)
            .collapsible(false)
            .default_width(total_width * 0.25)
            .default_height(total_height * 0.20)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(self.ctx(), |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(info);

                    ui.add_space(10.0);

                    ui.button("Ok").clicked()
                })
                .inner
            })
            .is_some_and(|r| r.inner.is_some_and(|r| r))
    }

    fn refresh_button<T, E, Fut>(
        &mut self,
        bind: &mut bind::Bind<T, E>,
        f: impl FnOnce() -> Fut,
        secs: f64,
    ) where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend + 'static,
        E: MaybeSend + 'static,
    {
        let resp = self.button("🔄");

        // Only actually refresh when clicked if the last completion was more than 1/4 of the interval ago
        let diff = if bind.since_completed() > secs / REFRESH_DEBOUNCE_FACTOR && resp.clicked() {
            bind.refresh(f());
            -1.0
        } else {
            bind.request_every_sec(f, secs)
        };

        resp.on_hover_text(if diff < 0.0 {
            "Refreshing now!".to_string()
        } else {
            format!("Refreshing automatically in {diff:.0}s...")
        });
    }
}
