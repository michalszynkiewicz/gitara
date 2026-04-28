//! Titlebar: app + repo name, right-aligned theme toggle.
//! 30px high, bg_titlebar background.

use crate::app::AppState;
use crate::theme::ThemeMode;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::view::{flex, label, Axis, CrossAxisAlignment, FlexSpacer};

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let mode = state.theme_mode;

    // Use a simple character glyph that exists in system fonts: U (sun-ish) /
    // D (dark). We'll label the button with just "Light" / "Dark" so it's
    // unambiguous without relying on symbols our font may not have.
    let (label_text, next_mode_text) = match mode {
        ThemeMode::Light => ("Light",  "Dark"),
        ThemeMode::Dark  => ("Dark",   "Light"),
    };
    let _ = next_mode_text;

    flex((
        label("gitara")
            .brush(theme.text)
            .text_size(13.0)
            .weight(xilem::FontWeight::MEDIUM),
        label(state.repo.name.clone())
            .brush(theme.text_muted)
            .text_size(12.0),
        FlexSpacer::Flex(1.0),
        flat_button(
            xilem::view::label(label_text)
                .brush(theme.text_muted)
                .text_size(11.0),
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
    ))
    .direction(Axis::Horizontal)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(8.0)
}
