//! `egui` integration for `egui-async`.
//!
//! This module provides the [`EguiAsyncPlugin`], which is necessary to integrate
//! `egui-async` into an `egui` application, alongside async immediate mode widgets.

pub mod bind_ext;
pub mod widgets;

pub use widgets::*;

use crate::bind;

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

    fn on_begin_pass(&mut self, ui: &mut egui::Ui) {
        bind::CTX.get_or_init(|| ui.ctx().clone());

        let time = ui.input(|i| i.time);

        let last_frame = bind::CURR_FRAME.swap(time, std::sync::atomic::Ordering::Relaxed);
        bind::LAST_FRAME.store(last_frame, std::sync::atomic::Ordering::Relaxed);
    }
}
