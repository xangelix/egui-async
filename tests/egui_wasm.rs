//! WASM tests for `egui` specific integration, run with `wasm-bindgen-test`.
//! Uses a headless egui context.

#![cfg(all(feature = "egui", target_family = "wasm"))]

use egui_async::{Bind, EguiAsyncPlugin};
use wasm_bindgen_futures::{JsFuture, js_sys::Promise, wasm_bindgen::JsValue};
use wasm_bindgen_test::wasm_bindgen_test;

/// Runs a single frame, discarding its output.
///
/// In debug builds, egui asserts that each frame's texture changes are handled before the
/// output is dropped, so they are cleared here as a headless backend would after uploading them.
fn run_frame(ctx: &egui::Context, run_ui: impl FnMut(&mut egui::Ui)) {
    let mut output = ctx.run_ui(egui::RawInput::default(), run_ui);
    output.textures_delta.clear();
}

/// Yields to the event loop, letting spawned local tasks run.
async fn tick() {
    let _ = JsFuture::from(Promise::resolve(&JsValue::UNDEFINED)).await;
}

// The plugin keeps the first context it sees, so this must remain the only test in this file
// that runs a frame.
#[wasm_bindgen_test]
async fn result_after_first_pass_requests_repaint() {
    let ctx = egui::Context::default();
    ctx.plugin_or_default::<EguiAsyncPlugin>();

    // The first pass lets the plugin register the context for background repaints.
    run_frame(&ctx, |_| {});

    // Settle any repaints egui requested for itself, so only the Bind's request is observed.
    for _ in 0..10 {
        if !ctx.has_requested_repaint() {
            break;
        }
        run_frame(&ctx, |_| {});
    }
    assert!(
        !ctx.has_requested_repaint(),
        "egui kept requesting repaints"
    );

    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
    let mut b: Bind<i32, ()> = Bind::new(true);
    b.request(async move {
        release_rx.await.map_err(|_| ())?;
        Ok(7)
    });
    tick().await;
    assert!(
        !ctx.has_requested_repaint(),
        "repaint requested before the result was delivered"
    );

    release_tx.send(()).expect("background task is gone");
    for _ in 0..100 {
        if ctx.has_requested_repaint() {
            break;
        }
        tick().await;
    }
    assert!(
        ctx.has_requested_repaint(),
        "delivering the result did not request a repaint"
    );

    // The repaint lets the next frame pick up the result.
    run_frame(&ctx, |_| {
        assert_eq!(b.read_as_ref(), Some(Ok(&7)));
    });
}
