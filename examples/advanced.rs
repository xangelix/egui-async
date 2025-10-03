use eframe::egui;
use egui_async::{Bind, ContextExt as _, StateWithData};
use walkers::{HttpTiles, Map, MapMemory, lat_lon, sources::OpenStreetMap};

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
    ip_lookup: Bind<(f64, f64), String>,
    input_ip: String,
    tiles: Option<HttpTiles>,
    map_memory: MapMemory,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            // Initialize with a non-retaining policy. Data is cleared if the UI isn't shown.
            my_ip: Bind::new(false),
            // Bind::default() is also non-retaining.
            ip_lookup: Bind::default(),
            input_ip: "8.8.8.8".to_string(),
            tiles: None,
            map_memory: MapMemory::default(),
        }
    }
}

// This could be a network request, a file operation, etc.
async fn fetch_my_ip() -> Result<String, String> {
    reqwest::get("https://icanhazip.com/")
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())
}

// This function fetches geolocation data for a given IP address.
async fn fetch_ip_location(ip: String) -> Result<(f64, f64), String> {
    let url = format!("http://ip-api.com/json/{ip}");

    let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        let raw_json = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| e.to_string())?;

        let latitude = raw_json
            .get("lat")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(|| "Missing 'lat' field in response.".to_string())?;
        let longitude = raw_json
            .get("lon")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(|| "Missing 'lon' field in response.".to_string())?;

        Ok((latitude, longitude))
    } else {
        Err(format!("Failed to fetch location data for {ip}."))
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // This must be called every frame to update the internal time
        // and drive the polling mechanism.
        ctx.loop_handle(); // <-- REQUIRED

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("egui-async Advanced Demo");
            ui.label("This example fetches your public IP address and looks up its geolocation.");

            ui.separator();

            match self.my_ip.state_or_request(fetch_my_ip) {
                StateWithData::Idle => {}
                StateWithData::Pending => {
                    ui.label("Fetching your IP address...");
                    ui.spinner();
                }
                StateWithData::Finished(ip) => {
                    let ip = ip.trim();
                    ui.horizontal(|ui| {
                        ui.label(format!("Your public IP is: {ip}"));
                        if ui.button("Copy").clicked() {
                            ui.ctx().copy_text(ip.to_string());
                        }
                    });
                }
                StateWithData::Failed(err) => {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("Could not fetch IP.\nError: {err}"),
                    );
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("IP to look up:");
                ui.text_edit_singleline(&mut self.input_ip);

                if ui.button("Lookup").clicked() {
                    // Fresh tile cache for new location
                    self.tiles = Some(HttpTiles::new(OpenStreetMap, ui.ctx().clone()));
                    // Our `egui-async` call
                    self.ip_lookup
                        .refresh(fetch_ip_location(self.input_ip.clone()));
                }

                if self.ip_lookup.is_pending() {
                    ui.spinner();
                    ui.label("Requesting...");
                } else {
                    ui.label(format!("Status: {:?}", self.ip_lookup.get_state()));
                }
            });

            match self.ip_lookup.state() {
                StateWithData::Idle => {
                    ui.label("No lookup data yet. Enter an IP and click 'Lookup'.");
                }
                StateWithData::Pending => {
                    ui.label("Looking up location...");
                    ui.spinner();
                }
                StateWithData::Finished((latitude, longitude)) => {
                    ui.label(format!(
                        "Located at:\nLongitude: {longitude}\nLatitude: {latitude}"
                    ));
                    ui.add(Map::new(
                        Some(self.tiles.as_mut().expect("tiles should be set")),
                        &mut self.map_memory,
                        lat_lon(*latitude, *longitude),
                    ));
                }
                StateWithData::Failed(err) => {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("Could not fetch location data.\nError: {err}"),
                    );
                }
            }
        });
    }
}
