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

        // Prevent `retain=false` Binds from aggressively clearing their state when
        // the application is minimized, occluded, or suspended by the OS.
        // If the UI isn't rendering an actual frame area, user widgets aren't running,
        // which means they can't call `.poll()`. Pausing the time-tracker here
        // prevents them from thinking they missed a frame.
        let is_suspended = ui.input(|i| {
            let info = i.viewport();
            info.minimized.unwrap_or(false)
                || info.occluded.unwrap_or(false)
                || !info.visible().unwrap_or(true)
        });

        if is_suspended {
            return;
        }

        let time = ui.input(|i| i.time);
        let curr_time = bind::CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed);

        // Multiple viewports (like tooltips or popups) trigger multiple passes
        // per frame, which can cause frame drift. We only advance the global
        // clock if the input time has actually progressed.
        #[allow(clippy::float_cmp)]
        if curr_time != time {
            let last_frame = bind::CURR_FRAME.swap(time, std::sync::atomic::Ordering::Relaxed);
            bind::LAST_FRAME.store(last_frame, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
