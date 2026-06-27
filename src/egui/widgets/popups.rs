//! Modal-style popup windows designed explicitly for error alerting and user notification.

/// A popup window designed to show errors with a built-in retry mechanism.
#[must_use = "You should call .show() on this widget to render it"]
pub struct ErrorPopup<'a> {
    error: &'a str,
    id_source: egui::Id,
    retry_text: String,
}

impl<'a> ErrorPopup<'a> {
    /// Creates a new error popup with the given error message.
    pub fn new(error: &'a str) -> Self {
        Self {
            error,
            id_source: egui::Id::new("egui_async_error_window"),
            retry_text: "Retry".to_string(),
        }
    }

    /// Overrides the default ID source. Useful if you have multiple distinct error popups.
    pub fn id_source(mut self, id: impl egui::AsId) -> Self {
        self.id_source = egui::Id::new(id);
        self
    }

    /// Renders the error popup onto the context.
    ///
    /// # Returns
    /// `true` if the "Retry" button was clicked this frame.
    #[must_use]
    pub fn show(self, ctx: &egui::Context) -> bool {
        let mut retry_clicked = false;
        let mut open = true;

        egui::Window::new("⚠️ Error")
            .id(self.id_source)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(self.error)
                            .color(ui.visuals().error_fg_color)
                            .size(14.0),
                    );
                    ui.add_space(20.0);

                    ui.separator();
                    ui.add_space(10.0);

                    ui.label("Please retry the request, or contact support if the error persists.");
                    ui.add_space(10.0);

                    if ui.button(&self.retry_text).clicked() {
                        retry_clicked = true;
                    }
                });
            });

        retry_clicked
    }
}

/// A simple informational popup window requiring user acknowledgment.
#[must_use = "You should call .show() on this widget to render it"]
pub struct NotifyPopup<'a> {
    info: &'a str,
    id_source: egui::Id,
    ok_text: String,
}

impl<'a> NotifyPopup<'a> {
    /// Creates a new notification popup with the given message.
    pub fn new(info: &'a str) -> Self {
        Self {
            info,
            id_source: egui::Id::new("egui_async_notify_window"),
            ok_text: "Ok".to_string(),
        }
    }

    /// Renders the notification popup onto the context.
    ///
    /// # Returns
    /// `true` if the acknowledgment button was clicked or the window was closed.
    #[must_use]
    pub fn show(self, ctx: &egui::Context) -> bool {
        let mut ok_clicked = false;
        let mut open = true;

        egui::Window::new("ℹ Information")
            .id(self.id_source)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(self.info);
                    ui.add_space(20.0);

                    ui.separator();
                    ui.add_space(10.0);

                    if ui.button(&self.ok_text).clicked() {
                        ok_clicked = true;
                    }
                });
            });

        ok_clicked || !open
    }
}
