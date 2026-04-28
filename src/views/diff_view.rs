//! Reusable diff renderer — the unified hunk-list view shared by the
//! Inspector's Diff tab and the Commit workbench modal.
//!
//! Returns a flex column whose intrinsic width is the longest line's
//! width. Wrap it in a `portal()` to get both vertical and horizontal
//! scrollbars (masonry's Portal turns the horizontal scrollbar on
//! whenever the content is wider than the viewport).

use std::path::PathBuf;

use xilem::view::{flex, label, sized_box, Axis, CrossAxisAlignment, Padding};
use xilem::WidgetView as _;

use crate::app::AppState;
use crate::model::diff::Hunk;
use crate::theme::Theme;

/// Render a list of `(path, hunk)` pairs with file headers between paths.
///
/// Crucially does **not** call `expand_width` on the inner column — so
/// long lines push out the natural width and a wrapping `portal` shows
/// a horizontal scrollbar.
pub fn render_hunks(
    hunks: Vec<(PathBuf, Hunk)>,
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    let mono: masonry::parley::FontStack<'static> = masonry::parley::FontStack::Source(
        std::borrow::Cow::Borrowed("ui-monospace, SFMono-Regular, Menlo, monospace"),
    );

    let mut rows: Vec<Box<xilem::AnyWidgetView<AppState>>> = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    for (path, hunk) in hunks {
        if current_path.as_ref() != Some(&path) {
            rows.push(
                label(path.display().to_string())
                    .brush(theme.accent)
                    .text_size(12.0)
                    .weight(xilem::FontWeight::MEDIUM)
                    .boxed(),
            );
            current_path = Some(path);
        }
        rows.push(
            label(hunk.header.clone())
                .brush(theme.text_dim)
                .text_size(11.0)
                .font(mono.clone())
                .boxed(),
        );
        for line in hunk.lines {
            use crate::model::diff::DiffOrigin as O;
            let (prefix, color) = match line.origin {
                O::Added => ("+", theme.added),
                O::Removed => ("−", theme.removed),
                O::Context => (" ", theme.text),
            };
            rows.push(
                label(format!("{prefix} {}", line.content))
                    .brush(color)
                    .text_size(11.0)
                    .font(mono.clone())
                    .boxed(),
            );
        }
        rows.push(sized_box(flex(())).height(6.0).boxed());
    }

    sized_box(
        flex(rows)
            .direction(Axis::Vertical)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap(0.0),
    )
    .padding(Padding::from_vh(10.0, 14.0))
}
