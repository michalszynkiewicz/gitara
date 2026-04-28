//! Modals — each is a centered card over a translucent backdrop.
//! Shared `shell()` renders chrome + backdrop; each per-type modal renders
//! its own body + footer.

pub mod commit;
pub mod rebase;
pub mod fetch;
pub mod push;
// add_remote is disabled — see ISSUES.md (unvalidated URL → CVE-2017-
// 1000117-class arg injection on next fetch). The module is kept under
// dead_code so re-enabling it is one line; the dispatch arm below
// renders a "feature disabled" notice instead of the form.
#[allow(dead_code)]
pub mod add_remote;
pub mod branch;
pub mod merge;
pub mod cherry_pick;
pub mod reset;
pub mod rename_branch;
pub mod tag;

use crate::app::{AppState, Modal};
use crate::theme::Theme;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use vello::peniko::Color;
use xilem::view::{flex, label, sized_box, Axis, CrossAxisAlignment, FlexExt as _, FlexSpacer, Padding};
use xilem::WidgetView as _;

/// Top-level modal dispatch. Returns `None` when no modal is open.
pub fn view(state: &mut AppState) -> Option<impl xilem::WidgetView<AppState>> {
    let modal = state.modal.clone()?;
    Some(match modal {
        Modal::Branch(_)     => branch::view(state).boxed(),
        Modal::Fetch(_)      => fetch::view(state).boxed(),
        Modal::Push(_)       => push::view(state).boxed(),
        Modal::Commit(_)     => commit::view(state).boxed(),
        Modal::Merge(_)      => merge::view(state).boxed(),
        Modal::Rebase(_)     => rebase::view(state).boxed(),
        Modal::AddRemote(_)  => disabled_view(
            "Add remote — disabled",
            "This action is temporarily disabled in gitara — it can pass\n\
             unvalidated URLs to `git remote add`. Use the CLI instead:\n\n\
             git remote add <name> <url>",
            &state.theme.clone(),
        ).boxed(),
        Modal::CherryPick(_)   => cherry_pick::view(state).boxed(),
        Modal::Reset(_)        => reset::view(state).boxed(),
        Modal::RenameBranch(_) => rename_branch::view(state).boxed(),
        Modal::Tag(_)          => tag::view(state).boxed(),
    })
}

/// Modal body shown for actions that are intentionally disabled.
/// Renders the title + subtitle + a Close-only footer using the
/// shared shell.
fn disabled_view(
    title: &'static str,
    body_text: &'static str,
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    let body: Box<xilem::AnyWidgetView<AppState>> = label(body_text.to_string())
        .brush(theme.text_muted)
        .text_size(12.0)
        .boxed();
    let footer: Box<xilem::AnyWidgetView<AppState>> = flex((
        FlexSpacer::Flex(1.0),
        flat_button(
            xilem::view::label("Close").brush(theme.text).text_size(12.0),
            FlatStyle {
                idle_bg: None,
                hover_bg: theme.bg_hover,
                active_bg: None,
                radius: 4.0,
                padding_v: 6.0,
                padding_h: 14.0,
            },
            false,
            |s: &mut AppState| s.modal = None,
        ),
    ))
    .direction(Axis::Horizontal)
    .gap(8.0)
    .boxed();
    shell(title, "", body, footer, theme)
}

/// Shared chrome: backdrop + centered card with title/subtitle, a body, and a footer.
pub fn shell(
    title: &str,
    subtitle: &str,
    body: Box<xilem::AnyWidgetView<AppState>>,
    footer: Box<xilem::AnyWidgetView<AppState>>,
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    let backdrop = sized_box(flex(()))
        .expand()
        .background(Color::from_rgba8(0, 0, 0, 102));

    let card = sized_box(
        flex((
            label(title.to_string())
                .brush(theme.text)
                .text_size(18.0)
                .weight(xilem::FontWeight::MEDIUM),
            label(subtitle.to_string()).brush(theme.text_muted).text_size(12.0),
            FlexSpacer::Fixed(14.0),
            body,
            FlexSpacer::Fixed(18.0),
            footer,
        ))
        .direction(Axis::Vertical)
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(6.0),
    )
    .width(520.0)
    .background(theme.bg_panel)
    .border(theme.border, 1.0)
    .rounded(8.0)
    .padding(Padding::from_vh(20.0, 24.0));

    use xilem::view::{zstack, Alignment};
    zstack((backdrop, card)).alignment(Alignment::Center)
}

/// Workbench-style modal shell — card occupies ~90% × 90% of the
/// window via flex spacers, so the body has room for two columns and
/// big diff scroll regions. Body sits between header and footer; the
/// body is wrapped in `flex(1.0)` so it grows to fill available space.
pub fn shell_large(
    title: &str,
    subtitle: &str,
    body: Box<xilem::AnyWidgetView<AppState>>,
    footer: Box<xilem::AnyWidgetView<AppState>>,
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    let backdrop = sized_box(flex(()))
        .expand()
        .background(Color::from_rgba8(0, 0, 0, 102));

    let card = sized_box(
        flex((
            label(title.to_string())
                .brush(theme.text)
                .text_size(18.0)
                .weight(xilem::FontWeight::MEDIUM),
            label(subtitle.to_string()).brush(theme.text_muted).text_size(12.0),
            FlexSpacer::Fixed(14.0),
            body.flex(1.0),
            FlexSpacer::Fixed(14.0),
            footer,
        ))
        .direction(Axis::Vertical)
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .must_fill_major_axis(true)
        .gap(6.0),
    )
    .expand()
    .background(theme.bg_panel)
    .border(theme.border, 1.0)
    .rounded(8.0)
    .padding(Padding::from_vh(20.0, 24.0));

    // 90% × 90% via flex spacers (1 : 18 : 1 → centre 90%).
    let row = flex((
        FlexSpacer::Flex(1.0),
        card.flex(18.0),
        FlexSpacer::Flex(1.0),
    ))
    .direction(Axis::Horizontal)
    .cross_axis_alignment(CrossAxisAlignment::Fill);

    let column = flex((
        FlexSpacer::Flex(1.0),
        row.flex(18.0),
        FlexSpacer::Flex(1.0),
    ))
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Fill);

    use xilem::view::{zstack, Alignment};
    zstack((backdrop, column)).alignment(Alignment::Center)
}

/// A small selectable pill for picking a branch / remote inside a modal.
/// `on_click` receives the picked name.
pub fn ref_chip<F>(
    name: &str,
    selected: bool,
    theme: &Theme,
    on_click: F,
) -> impl xilem::WidgetView<AppState>
where
    F: Fn(&mut AppState, String) + Send + Sync + 'static,
{
    use crate::widgets::flat_button::{flat_button, FlatStyle};
    let owned = name.to_string();
    flat_button(
        xilem::view::label(name.to_string())
            .brush(if selected { theme.accent_fg } else { theme.text })
            .text_size(11.0)
            .weight(if selected { xilem::FontWeight::MEDIUM } else { xilem::FontWeight::NORMAL }),
        FlatStyle {
            idle_bg: if selected { Some(theme.accent) } else { None },
            hover_bg: theme.bg_hover,
            active_bg: Some(theme.accent),
            radius: 12.0,
            padding_v: 3.0,
            padding_h: 10.0,
        },
        selected,
        move |st: &mut AppState| on_click(st, owned.clone()),
    )
}

/// Background colour for textbox-style inputs across all modals.
///
/// Why not theme-derived: masonry 0.3 hard-codes the text caret colour
/// to `palette::css::WHITE` (TextArea::paint, line ~918). On a light
/// theme a regular white-bg input field has an invisible cursor. Use a
/// fixed dark bg + light text combo for input fields so the caret is
/// always visible regardless of theme.
pub fn input_bg() -> Color {
    Color::from_rgba8(38, 40, 46, 255)
}

/// Text/foreground colour for input fields. Pairs with `input_bg()`.
pub fn input_text() -> Color {
    Color::from_rgba8(232, 234, 240, 255)
}

/// A faint version of theme.warn for "active toggle" backgrounds — used by
/// the Push modal's force-with-lease pill.
pub fn tinted_warn(theme: &Theme) -> Color {
    let [r, g, b, _] = theme.warn.components;
    let t = 0.18;
    Color::new([
        r + (1.0 - r) * (1.0 - t),
        g + (1.0 - g) * (1.0 - t),
        b + (1.0 - b) * (1.0 - t),
        1.0,
    ])
}

/// Footer helpers — shared across modals.
pub fn ok_cancel_footer<Fok>(theme: &Theme, ok_label: &'static str, on_ok: Fok)
    -> Box<xilem::AnyWidgetView<AppState>>
where
    Fok: Fn(&mut AppState) + Send + Sync + 'static,
{
    flex((
        FlexSpacer::Flex(1.0),
        flat_button(
            xilem::view::label("Cancel").brush(theme.text).text_size(12.0),
            FlatStyle {
                idle_bg: None,
                hover_bg: theme.bg_hover,
                active_bg: None,
                radius: 4.0,
                padding_v: 6.0,
                padding_h: 14.0,
            },
            false,
            |s: &mut AppState| s.modal = None,
        ),
        flat_button(
            xilem::view::label(ok_label)
                .brush(theme.accent_fg)
                .text_size(12.0)
                .weight(xilem::FontWeight::MEDIUM),
            FlatStyle {
                idle_bg: Some(theme.accent),
                hover_bg: theme.accent_hover,
                active_bg: None,
                radius: 4.0,
                padding_v: 6.0,
                padding_h: 14.0,
            },
            false,
            move |s: &mut AppState| on_ok(s),
        ),
    ))
    .direction(Axis::Horizontal)
    .gap(8.0)
    .boxed()
}

