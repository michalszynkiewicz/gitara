//! UI icon helpers.
//!
//! We use a small subset of Phosphor for the few inline icons (✓ ● ↑ ↓ → × −)
//! and let body text fall back to system fonts. That keeps the binary tiny
//! while still rendering icons consistently across hosts.
//!
//! The font is embedded by `crate::fonts::PHOSPHOR_SUBSET` and registered
//! at startup in `main.rs` via `Xilem::with_font`. fontique reports the
//! family name as "Phosphor", which we name explicitly here.

use std::borrow::Cow;
use xilem::masonry::parley::style::{FontFamily, FontStack};
use xilem::style::Style as _;
use xilem::view::Label;

const PHOSPHOR: FontStack<'static> =
    FontStack::Single(FontFamily::Named(Cow::Borrowed("Phosphor")));

/// Drop-in for `xilem::view::label`. Kept here so existing
/// `use crate::ui::label;` imports remain valid even after we stopped
/// pinning a UI font — body text now flows from the host system's
/// `system-ui`.
pub fn label(text: impl Into<masonry::core::ArcStr>) -> Label {
    xilem::view::label(text)
}

/// Render an icon by name. Unknown names render as an empty label.
pub fn icon(name: &str) -> Label {
    let cp = match name {
        "check" => '\u{e182}',
        "arrow-up" => '\u{e08e}',
        "arrow-down" => '\u{e03e}',
        "arrow-right" => '\u{e06c}',
        "minus" => '\u{e32a}',
        "x" => '\u{e4f6}',
        "dot" => '\u{ecde}',
        _ => ' ',
    };
    xilem::view::label(cp.to_string()).font(PHOSPHOR)
}

/// "Toggle row" — a checkbox-style label used by all our `flat_button`
/// toggles. Renders `[✓] text` when `on`, `text` alone when off, both
/// at the same color and font size so the row width doesn't jump
/// when the user clicks.
pub fn toggle_row<S, A>(
    on: bool,
    text: impl Into<masonry::core::ArcStr>,
    color: vello::peniko::Color,
    size: f32,
) -> impl xilem::WidgetView<S, A>
where
    S: 'static,
    A: 'static,
{
    use xilem::masonry::properties::types::AsUnit as _;
    use xilem::view::{flex, Axis};
    use xilem::WidgetView as _;
    let icon_view: Box<xilem::AnyWidgetView<S, A>> = if on {
        icon("check").text_size(size).color(color).boxed()
    } else {
        label("").text_size(size).boxed()
    };
    flex(
        Axis::Horizontal,
        (icon_view, label(text).text_size(size).color(color)),
    )
    .gap((4.0_f64).px())
}
