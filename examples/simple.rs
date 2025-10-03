use std::time::Duration;

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

struct MyApp {
    /// The Bind struct holds the state of our async operation.
    my_ip: Bind<String, String>, // Bind<OkType, ErrType>
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            // Initialize with a non-retaining policy. Data is cleared if the UI isn't shown.
            my_ip: Bind::new(false),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // This must be called every frame to update the internal time
        // and drive the polling mechanism.
        ctx.loop_handle(); // <-- REQUIRED

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("egui-async Demo");
            ui.label("This example fetches your public IP address asynchronously.");

            ui.separator();

            // `read_or_request` is a common pattern for data that should be loaded automatically.
            if let Some(res) = self.my_ip.read_or_request(|| async {
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
                ui.spinner();
            }

            // `refresh` immediately clears existing data and starts a new request.
            // Any active futures will be dropped
            if ui.button("Refresh IP with fragile connection").clicked() {
                self.my_ip.refresh(fragile_fetch_data());
            }
        });
    }
}

// This could be a network request, a file operation, etc.
async fn fragile_fetch_data() -> Result<String, String> {
    // Simulate a network delay (remember tokio sleep() won't work on wasm32 targets)
    #[cfg(not(target_family = "wasm"))]
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Simulate a possible error
    if rand::random() {
        reqwest::get("https://icanhazip.com/")
            .await
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())
    } else {
        Err("Failed to fetch data.".to_string())
    }
}
