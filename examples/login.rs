use std::time::Duration;

use eframe::egui;
use egui_async::{Bind, EguiAsyncPlugin, StateWithData};

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "egui-async Login Example",
        native_options,
        Box::new(|_cc| Ok(Box::new(LoginApp::default()))),
    )
}

struct LoginApp {
    username: String,
    password: String,
    // We bind to <String, String> -> <Success Message, Error Message>
    login: Bind<String, String>,
}

#[allow(clippy::derivable_impls)]
impl Default for LoginApp {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            // We use default() (retain = false) because if the user closes the window
            // or we change views, we probably don't need to keep the login state floating around.
            login: Bind::default(),
        }
    }
}

// A mock async function to simulate a server request
async fn perform_login(username: String, password: String) -> Result<String, String> {
    // Simulate network latency (2 seconds)
    #[cfg(not(target_family = "wasm"))]
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Simple validation logic
    if username.trim().is_empty() {
        return Err("Username cannot be empty.".to_owned());
    }

    if password == "secret" {
        Ok(format!("User '{username}'"))
    } else {
        Err("Invalid password. (Hint: try 'secret')".to_owned())
    }
}

impl eframe::App for LoginApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Register the plugin (Required!)
        ctx.plugin_or_default::<EguiAsyncPlugin>();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical(|ui| {
                ui.heading("Login Portal");
                ui.add_space(20.0);

                // 2. Explicit State Control Pattern
                // We match exhaustively on the state to determine exactly what the UI looks like.
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
            });
        });
    }
}
