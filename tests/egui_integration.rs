//! Tests for `egui` specific integration (Plugin, UI Extensions, Widgets).
//! Uses a headless egui context. See `egui_wasm.rs` for the WASM counterparts.

#![cfg(all(feature = "egui", not(target_family = "wasm")))]

use std::{
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use egui_async::{Bind, EguiAsyncPlugin, UiExt, bind::CURR_FRAME};

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn with_context<F>(f: F)
where
    F: FnOnce(&egui::Context),
{
    // Use lock to ensure global frame timers don't conflict between tests
    let _guard = test_lock().lock().expect("Global test mutex poisoned");

    // The plugin stores the first context it sees in a process-wide global, so all tests share
    // one context to make sure background repaints target the context under test.
    static CTX: OnceLock<egui::Context> = OnceLock::new();
    let ctx = CTX.get_or_init(|| {
        let ctx = egui::Context::default();
        // Register plugin once to init globals
        ctx.plugin_or_default::<EguiAsyncPlugin>();
        ctx
    });

    f(ctx);
}

/// Runs a single frame, discarding its output.
///
/// In debug builds, egui asserts that each frame's texture changes are handled before the
/// output is dropped, so they are cleared here as a headless backend would after uploading them.
fn run_frame(ctx: &egui::Context, input: egui::RawInput, run_ui: impl FnMut(&mut egui::Ui)) {
    let mut output = ctx.run_ui(input, run_ui);
    output.textures_delta.clear();
}

#[test]
fn plugin_updates_globals() {
    with_context(|ctx| {
        // Manipulate input time
        let input = egui::RawInput {
            time: Some(123.45),
            ..Default::default()
        };

        run_frame(ctx, input, |ui| {
            // Plugin hook runs in begin_frame via implicit call in `ctx.run_ui` or manual plugin usage.
            // EguiAsyncPlugin updates on `on_begin_pass`.
            // We must simulate the plugin call.
            let mut plugin = EguiAsyncPlugin;

            egui::Plugin::on_begin_pass(&mut plugin, ui);

            #[allow(clippy::float_cmp)]
            {
                assert_eq!(
                    CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed),
                    123.45
                );
            }
        });
    });
}

#[test]
fn plugin_debug_name() {
    let plugin = EguiAsyncPlugin;
    assert_eq!(egui::Plugin::debug_name(&plugin), "egui_async");
}

#[test]
fn ui_ext_read_methods() {
    with_context(|ctx| {
        run_frame(ctx, egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut b_ok: Bind<i32, String> = Bind::new(false);
                b_ok.fill(Ok(10));

                let mut b_err: Bind<i32, String> = Bind::new(false);
                b_err.fill(Err("oops".into()));

                let mut b_idle: Bind<i32, String> = Bind::new(false);

                // 1. read_or_error
                assert_eq!(b_ok.read_or_error(|| async { Ok(0) }, ui), Some(&10));
                // Should trigger popup logic (returns None for data)
                assert_eq!(b_err.read_or_error(|| async { Ok(0) }, ui), None);
                assert_eq!(b_idle.read_or_error(|| async { Ok(0) }, ui), None);

                // 2. read_mut_or_error
                if let Some(val) = b_ok.read_mut_or_error(|| async { Ok(0) }, ui) {
                    *val += 5;
                }
                assert_eq!(
                    b_ok.read()
                        .as_ref()
                        .expect("data missing")
                        .as_ref()
                        .expect("data is error"),
                    &15
                );
                assert!(b_err.read_mut_or_error(|| async { Ok(0) }, ui).is_none());

                // 3. read_or_request_or_error
                // Idle -> requests
                assert!(
                    b_idle
                        .read_or_request_or_error(|| async { Ok(99) }, ui)
                        .is_none()
                );
                assert!(b_idle.is_pending());

                // Err -> triggers popup logic
                assert!(
                    b_err
                        .read_or_request_or_error(|| async { Ok(0) }, ui)
                        .is_none()
                );

                // Ok -> returns data
                assert_eq!(
                    b_ok.read_or_request_or_error(|| async { Ok(0) }, ui),
                    Some(&15)
                );

                // 4. read_mut_or_request_or_error
                // Reset idle
                b_idle.clear();
                assert!(
                    b_idle
                        .read_mut_or_request_or_error(|| async { Ok(88) }, ui)
                        .is_none()
                );
                assert!(b_idle.is_pending());
            });
        });
    });
}

#[test]
fn ui_widgets_execute() {
    with_context(|ctx| {
        run_frame(ctx, egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                // Check popup_error execution
                // We aren't clicking, so it returns false
                assert!(!ui.popup_error("Testing Error Popup"));

                // Check popup_notify execution
                assert!(!ui.popup_notify("Testing Info Popup"));

                // Check refresh_button
                let mut b: Bind<i32, ()> = Bind::default();

                // Case 1: Button render (Idle)
                ui.refresh_button(&mut b, || async { Ok(1) }, 60.0);

                // Case 2: Button render (Pending)
                b.request(async { Ok(1) });
                ui.refresh_button(&mut b, || async { Ok(1) }, 60.0);

                // Case 3: Button render (Finished recently - Debounce logic)
                // FIX: Clear pending state before filling to avoid panic "Cannot fill a Bind that is not Idle"
                b.clear();
                b.fill(Ok(1));

                // Manually simulate "Just finished"
                // CURR_FRAME is updated by plugin test helpers usually, but inside ctx.run
                // we rely on what we set before.
                ui.refresh_button(&mut b, || async { Ok(1) }, 60.0);
            });
        });
    });
}

#[test]
fn refresh_button_interaction() {
    with_context(|ctx| {
        // We want to simulate a click.
        // In egui immediate mode, we can't easily "inject" a click into a specific widget ID
        // without knowing the ID generation logic or using a harness.
        // However, we can force the "clicked" branch by faking the response if we were using `egui_kittest`
        // but for pure `egui` unit tests, covering the function call (render) is usually accepted as
        // "code coverage" for the library unless we write a full integration harness.
        //
        // However, we CAN verify that `refresh_button` calls `request_every_sec` internally.

        run_frame(ctx, egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut b: Bind<i32, ()> = Bind::default();
                // Set time such that it IS overdue to force automatic refresh logic
                // Since bind uses CURR_FRAME global...

                // If we can't click, we rely on the automatic part of refresh_button:
                // It calls `bind.request_every_sec`.

                // Let's ensure that code path runs.
                ui.refresh_button(&mut b, || async { Ok(1) }, 0.1);
            });
        });
    });
}

#[test]
fn result_after_first_pass_requests_repaint() {
    with_context(|ctx| {
        // The first pass lets the plugin register the context for background repaints.
        run_frame(ctx, egui::RawInput::default(), |_| {});

        let repainted = Arc::new(AtomicBool::new(false));
        ctx.set_request_repaint_callback({
            let repainted = repainted.clone();
            move |_| repainted.store(true, Ordering::SeqCst)
        });

        // Settle any repaints egui requested for itself, so only the Bind's request is observed.
        for _ in 0..10 {
            if !ctx.has_requested_repaint() {
                break;
            }
            run_frame(ctx, egui::RawInput::default(), |_| {});
        }
        assert!(
            !ctx.has_requested_repaint(),
            "egui kept requesting repaints"
        );
        repainted.store(false, Ordering::SeqCst);

        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
        let mut b: Bind<i32, ()> = Bind::new(true);
        b.request(async move {
            release_rx.await.map_err(|_| ())?;
            Ok(7)
        });
        assert!(
            !repainted.load(Ordering::SeqCst),
            "repaint requested before the result was delivered"
        );

        release_tx.send(()).expect("background task is gone");

        let mut delivered = false;
        for _ in 0..200 {
            if repainted.load(Ordering::SeqCst) {
                delivered = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(delivered, "delivering the result did not request a repaint");
        assert!(ctx.has_requested_repaint());

        // The repaint lets the next frame pick up the result.
        run_frame(ctx, egui::RawInput::default(), |_| {
            assert_eq!(b.read_as_ref(), Some(Ok(&7)));
        });
    });
}
