//! Titlebar: app + repo name, right-aligned theme toggle.
//! 30px high, bg_titlebar background.

use crate::app::AppState;
use crate::theme::ThemeMode;
use crate::ui::label;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::masonry::properties::types::AsUnit as _;
use xilem::style::Style as _;
use xilem::view::{flex, Axis, CrossAxisAlignment, FlexSpacer};

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let mode = state.theme_mode;

    // Use a simple character glyph that exists in system fonts: U (sun-ish) /
    // D (dark). We'll label the button with just "Light" / "Dark" so it's
    // unambiguous without relying on symbols our font may not have.
    let (label_text, next_mode_text) = match mode {
        ThemeMode::Light => ("Light", "Dark"),
        ThemeMode::Dark => ("Dark", "Light"),
    };
    let _ = next_mode_text;

    flex(
        Axis::Vertical,
        (
            label("gitara")
                .text_size(13.0)
                .weight(xilem::FontWeight::MEDIUM)
                .color(theme.text),
            label(state.repo.name.clone())
                .text_size(12.0)
                .color(theme.text_muted),
            FlexSpacer::Flex(1.0),
            flat_button(
                crate::ui::label(label_text)
                    .text_size(11.0)
                    .color(theme.text_muted),
                FlatStyle {
                    idle_bg: None,
                    hover_bg: theme.bg_hover,
                    active_bg: None,
                    radius: 4.0,
                    padding_v: 3.0,
                    padding_h: 10.0,
                },
                false,
                |s: &mut AppState| s.toggle_theme(),
            ),
        ),
    )
    .direction(Axis::Horizontal)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap((8.0_f64).px())
}
