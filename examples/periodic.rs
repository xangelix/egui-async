use eframe::egui;
use egui_async::{Bind, ContextExt as _};

// Boilerplate to run an eframe app
fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "egui-async example",
        native_options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
    .unwrap();
}

#[derive(Default)]
struct MyApp {
    /// The Bind struct holds the state of our async operation.
    random_user: Bind<String, String>, // Bind<OkType, ErrType>
}

// This function fetches a random user's name.
async fn fetch_random_user() -> Result<String, String> {
    let url = "https://randomuser.me/api/?nat=us";
    let resp = reqwest::get(url).await.map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        let raw_json = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| e.to_string())?;

        let user = raw_json
            .get("results")
            .and_then(serde_json::Value::as_array)
            .and_then(|arr| arr.first())
            .ok_or_else(|| "Missing 'results' field in response.".to_string())?;
        let name = user
            .get("name")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| "Missing 'name' field in response.".to_string())?;

        let first_name = name
            .get("first")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "Missing 'first' field in response.".to_string())?;
        let last_name = name
            .get("last")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "Missing 'last' field in response.".to_string())?;

        Ok(format!("{first_name} {last_name}"))
    } else {
        Err("Failed to fetch random user.".to_string())
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // This must be called every frame to update the internal time
        // and drive the polling mechanism.
        ctx.loop_handle(); // <-- REQUIRED

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("egui-async Periodic Refresh Demo");
            ui.label("This example fetches a random user every 20 seconds.");

            ui.separator();

            let refresh_interval_secs = 10.0;
            let time_until_refresh = self
                .random_user
                .request_every_sec(fetch_random_user, refresh_interval_secs);

            // `read` only shows data if it's already available.
            if let Some(name) = self.random_user.read() {
                match name {
                    Ok(name) => ui.label(format!("Hello, {name}!")),
                    Err(err) => ui.colored_label(
                        egui::Color32::RED,
                        format!("Could not fetch random user.\nError: {err}"),
                    ),
                };

                ui.label(format!(
                    "Requesting a new random user in {time_until_refresh:.2}s...",
                ));

                // We must manually request a repaint to ensure the UI updates to show the
                // countdown. `egui` only repaints on user input (like clicks or drags)
                // or when explicitly requested. `egui-async` automatically requests a repaint
                // upon future *completion*, but not during the `Pending` state.
                ui.ctx().request_repaint();
            } else {
                ui.horizontal(|ui| {
                    ui.label("Fetching random user...");
                    ui.spinner();
                });
            }
        });
    }
}
