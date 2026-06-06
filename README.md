# 🔮 egui-async

[![Crates.io](https://img.shields.io/crates/v/egui-async)](https://crates.io/crates/egui-async)
[![Docs.rs](https://docs.rs/egui-async/badge.svg)](https://docs.rs/egui-async)
[![License](https://img.shields.io/crates/l/egui-async)](https://snyk.io/articles/apache-license/#apache-license-vs-mit)
[![GitHub Sponsors](https://img.shields.io/github/sponsors/xangelix?color=ea4aaa&style=flat)](https://github.com/sponsors/xangelix)

A simple, batteries-included library for running async tasks across frames in [`egui`](https://egui.rs/) and binding their results to your UI.

Supports both **native** (Tokio) and **wasm32** (Web) targets out of the box.

## 📖 Overview

Immediate-mode GUI libraries like `egui` are fantastic, but they pose a significant challenge: **how do you run long-running async tasks (like HTTP requests or file I/O) without freezing the UI thread?**

`egui-async` solves this by providing a `Bind<T, E>` struct. This struct acts as a state machine that bridges the gap between your immediate-mode render loop and your background async runtime.

It handles the lifecycle of the Future, manages the state transitions (`Idle` → `Pending` → `Finished`), and provides ergonomic UI widgets to visualize that state.

## ✨ Features

- 🔄 **Smart State Management**: Automatically tracks `Idle`, `Pending`, and `Finished` states. No more manual `Option<Result<...>>` juggling.
- 🌐 **Universal Support**: Seamlessly switches between `tokio` (native) and `wasm-bindgen-futures` (web). Write your code once, run everywhere.
- ⚡ **Lazy Loading**: `read_or_request` allows you to ergonomically trigger async fetches just by trying to read the data in your UI code.
- ⏱️ **Periodic Updates**: Built-in support for polling data at specific intervals (e.g., every 10 seconds).
- 🛑 **Task Abortion**: Supports physically aborting background tasks on native targets when the UI state changes.
- 🧩 **Widgets**: Drop-in UI components like `AsyncButton`, `AsyncSearch`, and `AsyncView` that handle spinners, debouncing, and layout snapping automatically.
- 🛠️ **Batteries Included**: Includes the async runtime for you, along with helper extension traits for popups and retry logic.

## 📦 Installation

```bash
cargo add egui-async
```

### 🧩 Compatibility

`egui` APIs change frequently. Ensure you are using a compatible version of `egui-async` for your project.

| `egui-async` | `egui` |
| ------------ | ------ |
| `>=0.4.0`    | `0.34` |
| `>=0.2.0`    | `0.33` |
| `<=0.1.1`    | `0.32` |

## 🚀 Quick Start

Using `egui-async` requires two steps: registering the plugin and using a `Bind`.

### 1. Register the Plugin

You **must** register the `EguiAsyncPlugin`. With `egui` 0.34, the best place to do this is in the `logic` step of your app, which runs before `ui` and manages background repaints.

```rust
impl eframe::App for MyApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 👇 Crucial: Call this once per frame!
        ctx.plugin_or_default::<egui_async::EguiAsyncPlugin>();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // We use `show_inside` because `ui` already provides a root UI context.
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Your UI code here...
        });
    }
}
```

### 2. Bind Data

Use `Bind<T, E>` to manage your data.

```rust
use egui_async::Bind;

struct MyApp {
    // Holds a Result<String, String>
    my_ip: Bind<String, String>,
}

// Inside your update loop:
if let Some(res) = self.my_ip.read_or_request(|| async {
    // This async block runs in the background!
    reqwest::get("https://icanhazip.com/")
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string()) // Our Bind<, E> error type is String
}) {
    match res {
        Ok(ip)  => ui.label(format!("IP: {ip}")),
        Err(err) => ui.colored_label(egui::Color32::RED, err),
    }
} else {
    // While the future is running, this block (None) block executes:
    ui.spinner();
}
```

## 💡 Common Usage Patterns & Examples

`egui-async` is designed to fit several different UI patterns depending on how much control you need.

### 1. Lazy Loading

Use `read_or_request` to defer network or I/O operations until the UI explicitly attempts to render the data. If the data is absent, the request is triggered and the method returns `None`, allowing you to render a loading state.

```rust
// If data is missing, start fetching it.
// If fetching, show a spinner.
// If finished, show the result.
if let Some(result) = self.data.read_or_request(fetch_data) {
    ui.label(format!("Data: {:?}", result));
} else {
    ui.spinner();
}
```

### 2. Explicit State Control

For more complex UI flows, such as authentication screens, use `state_or_request` to exhaustively match against the underlying state machine (`Idle`, `Pending`, `Finished`, or `Failed`). This provides full control over the layout during transitions.

```rust
use egui_async::StateWithData;

match self.login.state() {
    StateWithData::Idle => {
        // State: Idle -> Show Input Form
        ui.label("Please enter your credentials:");
        ui.add_space(10.0);

        // Use a Grid to align the labels and text boxes nicely
        egui::Grid::new("login_form")
            .num_columns(2)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                ui.label("Username:");
                ui.text_edit_singleline(&mut self.username);
                ui.end_row();

                ui.label("Password:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.password).password(true),
                );
                ui.end_row();
            });

        ui.add_space(20.0);

        // TRIGGER: User clicks button -> Transitions to Pending
        if ui.button("Log In").clicked() {
            let fut = perform_login(self.username.clone(), self.password.clone());
            self.login.request(fut);
        }
    }

    StateWithData::Pending => {
        // State: Pending -> Show Loading Indicator
        // We disable inputs or just hide them. Here we show a spinner.
        ui.spinner();
        ui.label("Authenticating...");

        ui.add_space(10.0);

        // Option: Allow cancelling the request
        if ui.button("Cancel").clicked() {
            // On native, this physically aborts the tokio task if configured
            self.login.clear();
        }
    }

    StateWithData::Finished(success_msg) => {
        // State: Finished (Ok) -> Show Success Screen
        ui.label(
            egui::RichText::new("Login Successful!")
                .color(egui::Color32::GREEN)
                .size(20.0),
        );
        ui.label(success_msg);

        ui.add_space(20.0);

        // TRIGGER: Reset to Idle to allow logging in again
        if ui.button("Log Out").clicked() {
            self.username.clear();
            self.password.clear();
            self.login.clear();
        }
    }

    StateWithData::Failed(err_msg) => {
        // State: Failed (Err) -> Show Error and Retry
        ui.label(
            egui::RichText::new("Login Failed")
                .color(egui::Color32::RED)
                .strong(),
        );
        ui.label(err_msg);

        ui.add_space(20.0);

        // TRIGGER: Reset to Idle to try again (keeps previous username/pass typed in)
        if ui.button("Try Again").clicked() {
            self.login.clear();
        }
    }
}
```

### 3. Periodic Polling

Use `request_every_sec` to automatically re-trigger asynchronous tasks at fixed intervals. To prevent request stacking, the interval timer is calculated strictly from the completion of the previous request.

```rust
// Automatically re-run the future if 5.0 seconds have passed since the last finish.
self.server_status.request_every_sec(check_server_health, 5.0);

// Display the current (cached) data
if let Some(Ok(status)) = self.server_status.read() {
    ui.label(format!("Server Status: {status}"));
}

```

### 4. UI Extension Traits

`egui-async` provides the `UiExt` trait to inject asynchronous behaviors directly into `egui::Ui`. This includes standardized components like debounced refresh buttons and error popups.

```rust
use egui_async::UiExt; // Import the trait

// Renders a button that spins while Pending.
// Clicking it forces a refresh.
// It also auto-refreshes every 60 seconds.
//
// We use egui's underlying monotonic clock for timing,
// so we aren't making IO time calls every frame! 😉
ui.refresh_button(&mut self.data, fetch_data, 60.0);

// If the bind failed, show a popup window with the error and a "Retry" button.
self.data.read_or_error(fetch_data, ui);

```

### 5. Other Built-in Widgets

For other standard use cases, utilize the included drop-in widgets (`AsyncButton`, `AsyncSearch`, `AsyncView`). These components automatically handle loading states, input debouncing, and layout transitions.

```rust
use egui_async::egui::{AsyncButton, AsyncView, StateLayout};

// A button that prevents double-clicks and shows an inline spinner
AsyncButton::new(&mut self.submit_bind, "Submit")
    .pending_text("Processing...")
    .show(ui, perform_submission);

// A declarative container that completely manages the Idle/Pending/Ok/Err lifecycle
AsyncView::new(&mut self.data_bind)
    .state_layout(StateLayout::CenterHorizontal)
    .show(ui, fetch_data, |ui, data| {
        ui.label(format!("Success: {data}"));
    });

```

### 🛠️ Building Custom Widgets

Building your own async-aware widgets is straightforward. Pass a `&mut Bind<T, E>` to your widget, and use its state methods (`is_pending()`, `read()`, etc.) to drive your rendering logic, calling `bind.request()` or `bind.refresh()` when the user interacts.

```rust
pub fn my_custom_widget(
    ui: &mut egui::Ui,
    bind: &mut Bind<String, ()>,
    fetch: impl FnOnce() -> impl Future<Output = Result<String, ()>> + 'static
) {
    if bind.is_pending() {
        ui.spinner();
    } else if ui.button("Fetch Data").clicked() {
        bind.request(fetch());
    }
}

```

### 🧑‍💻 See it in action:

You can find complete, runnable examples for all these patterns in the [`examples/`](https://github.com/xangelix/egui-async/tree/main/examples) directory of the repository:

- [`simple.rs`](examples/simple.rs) – A minimal HTTP fetch example.
- [`login.rs`](examples/login.rs) – The full "State Machine" pattern with forms and validation.
- [`periodic.rs`](examples/periodic.rs) – A dashboard widget that auto-refreshes.
- [`advanced.rs`](examples/advanced.rs) - An online, IP locator tool, with maps.
- [`widgets.rs`](examples/widgets.rs) - A showcase of all included async widgets (`AsyncButton`, `AsyncSearch`, `AsyncView`).

Look at the code before you run it and try to predict what it does and what it will look like!

## ⚙️ Configuration

### Retain Policy

By default (`Bind::default()`), `egui-async` assumes immediate-mode behavior: if you stop calling `poll()` (or `read_*`) on a Bind for a frame, it assumes the UI element is no longer visible and drops the data to save memory and reset the state.

If you want to keep data and state even when the UI component is hidden (e.g., inside a collapsed header or a closed tab), set `retain` to true:

```rust
let mut bind = Bind::new(true); // Retain = true
```

### Native Task Abort

On native targets (non-WASM), you can configure `Bind` to physically abort the Tokio task when the request is cleared or overwritten. This is useful for cancelling heavy computations or large downloads.

```rust
let mut bind = Bind::new(true);
bind.set_abort(true); // Enable physical cancellation (Native only)
```

> **⚠️ WebAssembly Note:**
>
> Due to browser limitations, `set_abort` has no effect on WASM targets. The `Future` will run to completion, but its result will be ignored by the `Bind`.
>
> There are some methods to do this inside the browser if you desire, but you will need to manually implement it.

### 🔧 Under the Hood

How does `egui-async` bridge the gap between Immediate Mode GUI (60fps loop) and Asynchronous Runtimes?

1. **The Plugin**: The `EguiAsyncPlugin` acts as the heartbeat. It synchronizes a global atomic clock with `egui`'s input time. This allows `Bind` instances to measure durations (like "time since finished") without expensive syscalls or mutex locking on every frame.
2. **The Channel**: When you call `request()`, we spawn a task on the runtime (Tokio or Wasm). We give that task a `oneshot::Sender`. The `Bind` struct holds the `oneshot::Receiver`.
3. **Non-Blocking Polling**: On every frame where the UI element is drawn, `Bind::poll()` checks the receiver.

- If the channel is empty, it returns `Pending` (and `egui` continues drawing).
- If the channel has data, it moves the state to `Finished`.
- **Crucially**, if the data arrives between frames, `Bind` _automatically requests a repaint_ from the `Context`, ensuring your UI updates immediately without user interaction.

#### Spawning

On **Native** targets, tasks are spawned onto the `tokio` runtime (specifically using `tokio::spawn`).

On **WASM** targets, tasks are spawned onto the browser's event loop using `wasm_bindgen_futures::spawn_local`. This detection happens automatically at compile time.

### ⚠️ Common Issues

**1. Forgetting the Plugin**
If your UI is stuck in `Pending` forever or your periodic requests aren't triggering, check your `App` implementation. `egui-async` requires its plugin to be registered every frame so it can track time and trigger background repaints.

With `egui` 0.34+, the required place to do this is inside the `logic` function:

```rust
impl eframe::App for MyApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Without this, egui-async has no concept of time!
        ctx.plugin_or_default::<egui_async::EguiAsyncPlugin>();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // ... your UI code ...
    }
}
```

**2. Dropping the Bind**
The `Bind` struct owns the receiving end of the async channel. If you create a `Bind` inside a function scope (local variable) instead of your App struct, it will be dropped at the end of the function, cancelling the specific UI binding.

- **Bad:** `let mut my_bind = Bind::new(false);` inside `update()`.
- **Good:** `self.my_bind` inside `struct MyApp`.

**3. The "Disappearing Data" Mystery**
By default, `Bind` uses `retain = false`. This means if `read()` or `poll()` is **not** called during a specific frame (e.g., the user switched to a different tab in your app), `egui-async` assumes the data is no longer needed and clears it to free memory.

- **Fix:** If you want data to persist while hidden, use `Bind::new(true)`.

#### 🌍 WASM Configuration

Some crates have features that must be enabled, or must be disabled, under WASM or WASM running in a browser runtime. If you're seeing nonsensical errors in your browser console, consider removing dependencies until things work, then slowly adding them back to see what breaks the runtime.

A very common example of this issue is the `getrandom` crate. Be sure to read relevant documentation for how to handle the browser runtime. [See the `getrandom` wasm32 documentation](https://docs.rs/getrandom/0.3.4/getrandom/#webassembly-support).

## 🤝 Contributing

Contributions are more than welcome! If you find a bug or have a feature request, please open an issue. If you want to contribute code, please submit a pull request.

There are many opportunities to create async-native structs on the UiExt trait inside of `src/egui.rs` that would make for great first-contributions!

### Special thank you to our contributors:

- @sectore

## ⚖️ License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](https://spdx.org/licenses/Apache-2.0))
- MIT license ([LICENSE-MIT](https://spdx.org/licenses/MIT))

at your option.

## Note

This is **not** an official `egui` product. Please refer to [https://github.com/emilk/egui](https://github.com/emilk/egui) for official crates and recommendations.
