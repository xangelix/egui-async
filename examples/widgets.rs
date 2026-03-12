use std::time::Duration;

use eframe::egui;
use egui_async::{
    Bind, EguiAsyncPlugin,
    egui::{AsyncButton, AsyncSearch, AsyncView, StateLayout},
};

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "egui-async Widgets",
        native_options,
        Box::new(|_cc| Ok(Box::new(WidgetsApp::default()))),
    )
}

#[derive(Default)]
struct WidgetsApp {
    button_bind: Bind<String, String>,
    view_bind: Bind<String, String>,
    search_bind: Bind<Vec<String>, String>,
    search_query: String,
}

// 1. A heavy compute task mock
async fn fetch_button_data() -> Result<String, String> {
    #[cfg(not(target_family = "wasm"))]
    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok("Background Task Successfully Executed!".to_string())
}

// 2. A sporadic network simulation
async fn fetch_view_data() -> Result<String, String> {
    #[cfg(not(target_family = "wasm"))]
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Simulate sporadic networking failure to demonstrate the internal retry modal
    if rand::random() {
        Ok("Data View Resolved Successfully! Great job!".to_string())
    } else {
        Err("Random simulated failure occurred. Please retry.".to_string())
    }
}

// 3. A typeahead database simulation
async fn fetch_search_results(query: String) -> Result<Vec<String>, String> {
    #[cfg(not(target_family = "wasm"))]
    tokio::time::sleep(Duration::from_millis(500)).await;

    if query.is_empty() {
        return Ok(vec![]);
    }

    Ok(vec![
        format!("{query} - Primary Result"),
        format!("{query} - Secondary Recommendation"),
        format!("{query} - Out of bounds query"),
    ])
}

impl eframe::App for WidgetsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.plugin_or_default::<EguiAsyncPlugin>();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("egui-async Widgets Demo");
            ui.separator();

            // 1. AsyncButton
            ui.label(egui::RichText::new("1. AsyncButton").strong());
            ui.label("Prevents double submissions by disabling itself and showing an inline spinner while running.");
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                AsyncButton::new(&mut self.button_bind, "Submit Task")
                    .pending_text("Processing Task...")
                    .show(ui, fetch_button_data);

                if let Some(Ok(res)) = self.button_bind.read() {
                    ui.label(egui::RichText::new(res).color(egui::Color32::GREEN));
                }
            });

            ui.add_space(20.0);

            // 2. AsyncView
            ui.label(egui::RichText::new("2. AsyncView").strong());
            ui.label("A declarative container that universally traps the visual states of Pending, Ok, and Error.");
            ui.add_space(5.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_height(60.0);
                AsyncView::new(&mut self.view_bind)
                    .loading_text("Fetching extremely complex data...")
                    .state_layout(StateLayout::CenterHorizontal)
                    .show(ui, fetch_view_data, |ui, data| {
                        ui.label(egui::RichText::new(data).color(egui::Color32::GREEN));
                    });
            });

            ui.add_space(20.0);

            // 3. AsyncSearch
            ui.label(egui::RichText::new("3. AsyncSearch (Start Typing!)").strong());
            ui.label("A completely decoupled input field that natively handles debounce timers and search results.");
            ui.add_space(5.0);

            let (_, selected) = AsyncSearch::new(
                &mut self.search_bind,
                &mut self.search_query,
            )
            .debounce_secs(0.5)
            .hint_text("Search for something...")
            .show(ui, fetch_search_results);

            if let Some(item) = selected {
                println!("User selected: {item}");
            }
        });
    }
}
