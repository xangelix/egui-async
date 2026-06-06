//! A text input widget that automatically triggers async fetches as the user types.

use crate::bind::{Bind, CURR_FRAME, MaybeSend, State};

#[derive(Clone)]
struct AsyncSearchState {
    last_typed: f64,
    last_query: String,
}

/// A text edit widget that debounces input and automatically triggers an asynchronous search.
///
/// `AsyncSearch` listens to changes in a query string and waits for a specified debounce
/// threshold before launching the background task. It shows a spinner while the search
/// is in flight and displays results in a floating dropdown portal.
#[must_use = "You should call .show() on this widget to render it"]
pub struct AsyncSearch<'a, T, E> {
    bind: &'a mut Bind<Vec<T>, E>,
    query: &'a mut String,
    debounce_secs: f64,
    hint_text: String,
    retain_previous_results: bool,
    wrap_results: bool,
    width: Option<f32>,
    popup_width: Option<f32>,
}

impl<'a, T, E> AsyncSearch<'a, T, E> {
    /// Creates a new `AsyncSearch` bound to the provided query string and results bind.
    pub fn new(bind: &'a mut Bind<Vec<T>, E>, query: &'a mut String) -> Self {
        Self {
            bind,
            query,
            debounce_secs: 0.5,
            hint_text: "Search...".to_string(),
            retain_previous_results: true,
            wrap_results: false,
            width: None,
            popup_width: None,
        }
    }

    /// Sets the debounce timer threshold (in seconds) before making an async search call.
    pub const fn debounce_secs(mut self, secs: f64) -> Self {
        self.debounce_secs = secs;
        self
    }

    /// Sets the placeholder text for the search box.
    pub fn hint_text(mut self, text: impl Into<String>) -> Self {
        self.hint_text = text.into();
        self
    }

    /// If set to `true` (default), the widget will display the results of the previous
    /// successful search while the user is typing the next query and while the next
    /// query is pending.
    pub const fn retain_previous_results(mut self, retain: bool) -> Self {
        self.retain_previous_results = retain;
        self
    }

    /// Determines if the returned text inside the result rows should wrap or truncate.
    /// Default is `false` (truncate).
    pub const fn wrap_results(mut self, wrap: bool) -> Self {
        self.wrap_results = wrap;
        self
    }

    /// Explicitly forces the width of the input box.
    pub const fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Sets a fixed width for the search results popup.
    /// If not provided, it will automatically match the width of the text input.
    pub const fn popup_width(mut self, width: f32) -> Self {
        self.popup_width = Some(width);
        self
    }

    /// Renders the search box and processes background fetch logic.
    ///
    /// Returns the text edit response, and an `Option<T>` containing the selected
    /// item if the user just clicked one.
    pub fn show<Fut>(
        self,
        ui: &mut egui::Ui,
        fetch: impl FnOnce(String) -> Fut,
    ) -> (egui::Response, Option<T>)
    where
        Fut: Future<Output = Result<Vec<T>, E>> + MaybeSend + 'static,
        T: MaybeSend + Clone + std::fmt::Display + 'static,
        E: MaybeSend + 'static,
    {
        let AsyncSearch {
            bind,
            query,
            debounce_secs,
            hint_text,
            retain_previous_results,
            wrap_results,
            width,
            popup_width,
        } = self;

        let id = ui.id().with("async_search_state");

        let mut state = ui.data_mut(|d| {
            d.get_temp::<AsyncSearchState>(id)
                .unwrap_or_else(|| AsyncSearchState {
                    last_typed: 0.0,
                    last_query: query.clone(),
                })
        });

        let mut text_edit = egui::TextEdit::singleline(query).hint_text(&hint_text);
        if let Some(w) = width {
            text_edit = text_edit.desired_width(w);
        }

        let resp = ui.add(text_edit);
        let curr_time = CURR_FRAME.load(std::sync::atomic::Ordering::Relaxed);

        if resp.changed() {
            state.last_typed = curr_time;
            if !retain_previous_results {
                bind.clear();
            }
        }

        let text_trimmed = query.trim().to_string();
        let is_debouncing = !text_trimmed.is_empty()
            && curr_time - state.last_typed < debounce_secs
            && state.last_query != text_trimmed;

        if !text_trimmed.is_empty() && !is_debouncing && state.last_query != text_trimmed {
            state.last_query.clone_from(&text_trimmed);

            if retain_previous_results {
                bind.request(fetch(text_trimmed));
            } else {
                bind.refresh(fetch(text_trimmed));
            }
        } else if state.last_query != text_trimmed {
            ui.ctx().request_repaint(); // Stay responsive during debounce window
        }

        ui.data_mut(|d| d.insert_temp(id, state.clone()));

        // Early return for empty queries removes deep nesting.
        if query.is_empty() {
            if !bind.is_idle() {
                bind.clear();
            }
            return (resp, None);
        }

        let bind_state = bind.get_state();
        let data = bind.read();
        let bind_data_is_some = data.is_some();

        let should_show_popup = bind_state != State::Idle
            || is_debouncing
            || (retain_previous_results && bind_data_is_some);

        let mut selected_item = None;

        if should_show_popup {
            let popup_resp = Self::draw_popup(
                ui,
                &resp,
                bind_state,
                data.as_ref(),
                query,
                popup_width,
                wrap_results,
                &mut state,
                &mut selected_item,
                id,
                is_debouncing,
            );

            if Self::handle_click_away(ui, &resp, &popup_resp.response) {
                bind.clear();
            }
        }

        (resp, selected_item)
    }

    /// Evaluates if the user clicked away or hit escape to dismiss the widget.
    fn handle_click_away(
        ui: &egui::Ui,
        input_resp: &egui::Response,
        popup_resp: &egui::Response,
    ) -> bool {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            return true;
        }

        if ui.input(|i| i.pointer.any_pressed())
            && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
        {
            let clicked_in_input = input_resp.rect.contains(pos);
            let clicked_in_popup = popup_resp.rect.contains(pos);

            if !clicked_in_input && !clicked_in_popup {
                return true;
            }
        }

        false
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_popup(
        ui: &egui::Ui,
        resp: &egui::Response,
        bind_state: State,
        data: Option<&Result<Vec<T>, E>>,
        query: &mut String,
        popup_width: Option<f32>,
        wrap_results: bool,
        state: &mut AsyncSearchState,
        selected_item: &mut Option<T>,
        id: egui::Id,
        is_debouncing: bool,
    ) -> egui::InnerResponse<()>
    where
        T: MaybeSend + Clone + std::fmt::Display + 'static,
        E: MaybeSend + 'static,
    {
        let area = egui::Area::new(id.with("popup_area"))
            .order(egui::Order::Tooltip)
            .fixed_pos(resp.rect.left_bottom() + egui::vec2(0.0, 4.0));

        area.show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let popup_width = popup_width.unwrap_or_else(|| resp.rect.width());
                ui.set_min_width(popup_width);
                ui.set_max_width(popup_width);

                if bind_state == State::Pending || is_debouncing {
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        ui.spinner();
                        ui.add_space(4.0);
                        ui.label(if is_debouncing {
                            "Waiting to search..."
                        } else {
                            "Searching..."
                        });
                    });
                }

                match data {
                    Some(Ok(results))
                        if results.is_empty() && bind_state != State::Pending && !is_debouncing =>
                    {
                        ui.weak("No results found.");
                    }
                    Some(Ok(results)) => {
                        if bind_state == State::Pending || is_debouncing {
                            ui.separator();
                        }

                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.style_mut().wrap_mode = Some(if wrap_results {
                                    egui::TextWrapMode::Wrap
                                } else {
                                    egui::TextWrapMode::Truncate
                                });

                                ui.with_layout(
                                    egui::Layout::top_down_justified(egui::Align::LEFT),
                                    |ui| {
                                        for item in results {
                                            let text = egui::WidgetText::from(item.to_string());

                                            if ui.selectable_label(false, text).clicked() {
                                                *query = item.to_string();
                                                state.last_query.clone_from(query);
                                                ui.data_mut(|d| d.insert_temp(id, state.clone()));
                                                *selected_item = Some(item.clone());
                                            }
                                        }
                                    },
                                );
                            });
                    }
                    Some(Err(_err)) if bind_state != State::Pending && !is_debouncing => {
                        ui.colored_label(ui.visuals().error_fg_color, "Search failed.");
                    }
                    _ => {}
                }
            });
        })
    }
}
