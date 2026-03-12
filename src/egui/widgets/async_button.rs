//! An intelligent action button that disables itself and displays an inline spinner while loading.

use std::sync::Arc;

use crate::bind::{Bind, MaybeSend};

/// Contains the exact layout metrics needed to render the button without shifting.
struct ButtonLayout {
    normal_galley: Arc<egui::Galley>,
    pending_galley: Arc<egui::Galley>,
    desired_size: egui::Vec2,
    spinner_size: f32,
    item_spacing: egui::Vec2,
}

/// A button that initiates an asynchronous operation when clicked.
///
/// It automatically disables itself and swaps its display text to a spinner
/// to prevent double-submissions and indicate work is being done.
#[must_use = "You should call .show() on this widget to render it"]
pub struct AsyncButton<'a, T, E> {
    bind: &'a mut Bind<T, E>,
    text: egui::WidgetText,
    pending_text: Option<egui::WidgetText>,
    frame: bool,
    clear_on_click: bool,
}

impl<'a, T, E> AsyncButton<'a, T, E> {
    /// Creates a new `AsyncButton`.
    pub fn new(bind: &'a mut Bind<T, E>, text: impl Into<egui::WidgetText>) -> Self {
        Self {
            bind,
            text: text.into(),
            pending_text: None,
            frame: true,
            clear_on_click: true,
        }
    }

    /// Sets the text to display next to the spinner while the operation is pending.
    pub fn pending_text(mut self, text: impl Into<egui::WidgetText>) -> Self {
        self.pending_text = Some(text.into());
        self
    }

    /// If set to `false`, the button will be rendered as plain text without a background frame,
    /// similar to a clickable hyperlink label.
    pub const fn frame(mut self, frame: bool) -> Self {
        self.frame = frame;
        self
    }

    /// If set to `true` (default), the button will immediately clear the `Bind`'s previous
    /// data upon being clicked. If `false`, the old data will remain accessible via `.read()`
    /// while the new fetch is pending.
    pub const fn clear_on_click(mut self, clear: bool) -> Self {
        self.clear_on_click = clear;
        self
    }

    /// Shows the button in the given UI and triggers the future if clicked.
    pub fn show<Fut>(self, ui: &mut egui::Ui, f: impl FnOnce() -> Fut) -> egui::Response
    where
        Fut: Future<Output = Result<T, E>> + MaybeSend + 'static,
        T: MaybeSend + 'static,
        E: MaybeSend + 'static,
    {
        let is_pending = self.bind.is_pending();
        let layout = self.calculate_layout(ui);

        let sense = if is_pending {
            egui::Sense::hover()
        } else {
            egui::Sense::click()
        };

        let (rect, resp) = ui.allocate_exact_size(layout.desired_size, sense);

        if ui.is_rect_visible(rect) {
            self.paint_visuals(ui, &rect, &resp, &layout, is_pending);
        }

        if resp.clicked() && !is_pending {
            if self.clear_on_click {
                self.bind.refresh(f());
            } else {
                self.bind.request(f());
            }
        }

        if is_pending {
            resp.on_hover_cursor(egui::CursorIcon::Wait)
        } else {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        }
    }

    /// Calculates galleys and maximum necessary bounding boxes to prevent visual shift.
    fn calculate_layout(&self, ui: &egui::Ui) -> ButtonLayout {
        let pending_display = self
            .pending_text
            .clone()
            .unwrap_or_else(|| self.text.clone());

        let normal_galley =
            self.text
                .clone()
                .into_galley(ui, None, f32::INFINITY, egui::FontSelection::Default);
        let pending_galley =
            pending_display.into_galley(ui, None, f32::INFINITY, egui::FontSelection::Default);

        let button_padding = if self.frame {
            ui.spacing().button_padding
        } else {
            egui::vec2(2.0, 2.0)
        };

        let item_spacing = ui.spacing().item_spacing;
        let spinner_size = ui.text_style_height(&egui::TextStyle::Button);

        let normal_size = normal_galley.size() + 2.0 * button_padding;
        let mut pending_size = pending_galley.size() + 2.0 * button_padding;

        if pending_galley.size().x > 0.0 {
            pending_size.x += item_spacing.x;
        }

        pending_size.x += spinner_size;
        pending_size.y = pending_size
            .y
            .max(2.0f32.mul_add(button_padding.y, spinner_size));

        let desired_size = egui::vec2(
            normal_size.x.max(pending_size.x),
            normal_size.y.max(pending_size.y),
        );

        ButtonLayout {
            normal_galley,
            pending_galley,
            desired_size,
            spinner_size,
            item_spacing,
        }
    }

    /// Handles painting the background, strokes, spinner, and centered text.
    fn paint_visuals(
        &self,
        ui: &mut egui::Ui,
        rect: &egui::Rect,
        resp: &egui::Response,
        layout: &ButtonLayout,
        is_pending: bool,
    ) {
        let visuals = if is_pending {
            ui.style().visuals.widgets.noninteractive
        } else {
            *ui.style().interact(resp)
        };

        // 1. Paint the background
        if self.frame || (resp.hovered() && !is_pending) {
            let (fill, stroke) = if is_pending && self.frame {
                (
                    ui.style().visuals.widgets.inactive.bg_fill,
                    ui.style().visuals.widgets.inactive.bg_stroke,
                )
            } else if self.frame || resp.hovered() {
                (visuals.bg_fill, visuals.bg_stroke)
            } else {
                (egui::Color32::TRANSPARENT, egui::Stroke::NONE)
            };

            let expansion = if resp.hovered() && !is_pending && self.frame {
                visuals.expansion
            } else if !self.frame && resp.hovered() && !is_pending {
                ui.spacing().item_spacing.x * 0.5
            } else {
                0.0
            };

            ui.painter().rect(
                rect.expand(expansion),
                visuals.corner_radius,
                fill,
                stroke,
                egui::StrokeKind::Middle,
            );
        }

        // 2. Center content inside the fixed rect
        let current_galley = if is_pending {
            &layout.pending_galley
        } else {
            &layout.normal_galley
        };

        let content_width = if is_pending {
            if current_galley.size().x > 0.0 {
                current_galley.size().x + layout.item_spacing.x + layout.spinner_size
            } else {
                layout.spinner_size
            }
        } else {
            current_galley.size().x
        };

        let mut cursor_x = rect.center().x - content_width / 2.0;

        // 3. Paint the spinner (if pending)
        if is_pending {
            let spinner_rect = egui::Rect::from_min_size(
                egui::pos2(cursor_x, rect.center().y - layout.spinner_size / 2.0),
                egui::vec2(layout.spinner_size, layout.spinner_size),
            );

            ui.put(
                spinner_rect,
                egui::Spinner::new()
                    .size(layout.spinner_size)
                    .color(visuals.text_color()),
            );

            cursor_x += layout.spinner_size;
            if current_galley.size().x > 0.0 {
                cursor_x += layout.item_spacing.x;
            }
        }

        // 4. Paint the text
        if current_galley.size().x > 0.0 {
            let text_color = visuals.text_color();
            let text_pos = egui::pos2(cursor_x, rect.center().y - current_galley.size().y / 2.0);

            ui.painter()
                .galley(text_pos, current_galley.clone(), text_color);
        }
    }
}
