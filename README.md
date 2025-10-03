# egui-async

[![Crates.io](https://img.shields.io/crates/v/egui-async)](https://crates.io/crates/egui-async)
[![Docs.rs](https://docs.rs/egui-async/badge.svg)](https://docs.rs/egui-async)
[![License](https://img.shields.io/crates/l/egui-async)](https://snyk.io/articles/apache-license/#apache-license-vs-mit)

A simple, batteries-included, library for running async tasks across frames in [`egui`](https://crates.io/crates/egui) and binding their results to your UI.

Supports both native and wasm32 targets.

## What is this?

Immediate-mode GUI libraries like `egui` are fantastic, but they pose a challenge: how do you run a long-running or async task (like a network request), between frames, without blocking the UI thread?

`egui-async` provides a simple `Bind<T, E>` struct that wraps an async task, manages its state (`Idle`, `Pending`, `Finished`), and provides ergonomic helpers to render the UI based on that state.

It works with both `tokio` on native and `wasm-bindgen-futures` on the web, right out of the box.

## Features

- **Simple State Management**: Wraps any `Future` and tracks its state.
- **WASM Support**: Works seamlessly on both native and `wasm32` targets.
- **Ergonomic Helpers**: Methods like `read_or_request_or_error` simplify UI logic into a single line.
- **Convenient Widgets**: Includes a `refresh_button` and helpers for error popups.
- **Minimal Dependencies**: Built on `tokio` and (for wasm) `wasm-bindgen-futures`.

## How it Works

`egui-async` works by bridging `egui`'s immediate-mode rendering loop with a background async runtime.

1.  `ctx.loop_handle()`: You must call this once per frame. It updates a global frame timer that `Bind` uses to track its state.
2.  `Bind::request()`: When you start an operation, it spawns a `Future` onto a runtime (`tokio` on native, `wasm-bindgen-futures` on web).
3.  **Communication**: The spawned task is given a `tokio::sync::oneshot::Sender`. When the future completes, it sends the `Result` back to the `Bind` instance, which holds the `Receiver`.
4.  **Polling**: On each frame, `Bind` checks its receiver to see if the result has arrived. If it has, `Bind` transitions from the `Pending` state to the `Finished` state.
5.  **UI Update**: Your UI code can then check the `Bind`'s state and display the data, an error, or a loading indicator.

## Quickstart

Here is a minimal example using `eframe` that shows how to fetch data from an async function.

First, add `egui-async` to your dependencies:
```sh
cargo add egui-async
```

Then, use the `Bind` struct in your application:

```rust
use eframe::egui;
use egui_async::{Bind, ContextExt};

struct MyApp {
    /// The Bind struct holds the state of our async operation.
    data_bind: Bind<String, String>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            // We initialize the Bind and tell it to not retain data
            // if it's not visible for a frame.
            // If set to true, this will retain data even as the
            // element goes undrawn.
            data_bind: Bind::new(false), // Same as Bind::default()
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // This must be called every frame to update the internal time
        // and drive the polling mechanism.
        ctx.loop_handle(); // <-- REQUIRED

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Async Data Demo");
            ui.add_space(10.0);

            // Request if `data_bind` is None and idle
            // Otherwise, just read it
            if let Some(res) = self.data_bind.read_or_request(|| async {
                reqwest::get("https://icanhazip.com/")
                    .await
                    .map_err(|e| e.to_string())?
                    .text()
                    .await
                    .map_err(|e| e.to_string())
            }) {
                match res {
                    Ok(ip) => {
                        ui.label(format!("Your public IP is: {ip}"));
                    }
                    Err(err) => {
                        ui.colored_label(
                            egui::Color32::RED,
                            format!("Could not fetch IP.\nError: {err}"),
                        );
                    }
                }
            } else {
                ui.label("Getting public IP...");
                ui.spinner();
            }
        });
    }
}

// Boilerplate
fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "egui-async example",
        native_options,
        Box::new(|_cc| Box::new(MyApp::default())),
    )
    .unwrap();
}
```

## Common API Patterns

`egui-async` offers several helper methods on `Bind` to handle common UI scenarios. Here are the most frequently used patterns.

### The Full State Machine: `state_or_request`

This is the most powerful and explicit pattern. Use it when you want to render a different UI for every possible state: `Pending`, `Finished` with data, `Failed` with an error, or `Idle`. It's perfect for detailed components that need to show loading spinners, error messages, and the final data.

```rust
match self.data_bind.state_or_request(my_async_fn) {
    StateWithData::Idle => { /* This is usually skipped */ }
    StateWithData::Pending => { ui.spinner(); }
    StateWithData::Finished(data) => { ui.label(format!("Success: {data}")); }
    StateWithData::Failed(err) => { ui.colored_label(egui::Color32::RED, err); }
}
```

-----

### Simple Data Display: `read_or_request`

Use this pattern when you primarily care about the successful result and want a simple loading state. It returns an `Option<&Result<T, E>>`. If the value is `Some`, you can handle the `Ok` and `Err` cases. If it's `None`, the request is `Pending`, so you can show a spinner.

```rust
if let Some(result) = self.data_bind.read_or_request(my_async_fn) {
    match result {
        Ok(data) => { ui.label(format!("Your IP is: {data}")); }
        Err(err) => { ui.colored_label(egui::Color32::RED, err); }
    }
} else {
    ui.spinner();
    ui.label("Loading...");
}
```

-----

### Periodic Refresh: `request_every_sec`

Use this for data that should be updated automatically on a timer, like a dashboard widget. You provide an interval in seconds, and `egui-async` will trigger a new request when the interval has passed since the last successful completion.

```rust
// In your update loop:
let refresh_interval_secs = 20.0;
self.live_data.request_every_sec(fetch_live_data, refresh_interval_secs);

// You can still read the data to display it
if let Some(Ok(data)) = self.live_data.read() {
    ui.label(format!("Live data: {data}"));
}
```

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](https://spdx.org/licenses/Apache-2.0))
- MIT license ([LICENSE-MIT](https://spdx.org/licenses/MIT))

at your option.

## Contribution

Contributions are welcome! Please feel free to submit a pull request or open an issue.

## Todo

In the future I may consider a registry architecture rather than polling on each request, which would allow mature threading-- however this poses unique difficulties of its own. Feel free to take a shot at it in a PR.

A builder API is a likely "want" for 1.0.

## Notes

This is **not** an official `egui` product. Please refer to [https://github.com/emilk/egui](https://github.com/emilk/egui) for official crates and recommendations.
