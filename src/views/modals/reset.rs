//! Reset modal — `git reset {--soft|--mixed|--hard} <oid>`. Pre-filled
//! with the commit that was right-clicked.

use crate::app::{AppState, Modal, ResetMode, ResetModalState, Toast};
use crate::git;
use crate::theme::Theme;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::view::{flex, label, Axis, CrossAxisAlignment, FlexSpacer};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let s = state_get(state).cloned().unwrap_or_default();

    let body = body_view(&s, &theme).boxed();
    let footer = super::ok_cancel_footer(&theme, "Reset", run_reset);

    let short = &s.oid[..s.oid.len().min(7)];
    let subtitle = format!("Move HEAD to {short}");

    super::shell("Reset", &subtitle, body, footer, &theme)
}

fn body_view(s: &ResetModalState, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let mode_label = label("mode")
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM);

    let chips = flex((
        mode_chip("soft",  ResetMode::Soft,  s.mode, theme),
        mode_chip("mixed", ResetMode::Mixed, s.mode, theme),
        mode_chip("hard",  ResetMode::Hard,  s.mode, theme),
    ))
    .direction(Axis::Horizontal)
    .gap(6.0);

    // Per-mode hint that explains what the user is about to do.
    let (hint_text, hint_color) = match s.mode {
        ResetMode::Soft  => (
            "keeps index + working tree — staged changes preserved",
            theme.text_muted,
        ),
        ResetMode::Mixed => (
            "resets index, keeps working tree — changes become unstaged",
            theme.text_muted,
        ),
        ResetMode::Hard  => (
            "resets index AND working tree — uncommitted changes will be lost",
            theme.warn,
        ),
    };
    let hint = label(hint_text.to_string()).brush(hint_color).text_size(11.0);

    let error_view: Box<xilem::AnyWidgetView<AppState>> = match &s.error {
        Some(err) => label(err.clone()).brush(theme.removed).text_size(11.0).boxed(),
        None => label("").boxed(),
    };

    flex((
        mode_label,
        chips,
        FlexSpacer::Fixed(12.0),
        hint,
        FlexSpacer::Fixed(8.0),
        error_view,
    ))
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(4.0)
}

fn mode_chip(
    text: &'static str,
    mode: ResetMode,
    current: ResetMode,
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    let selected = mode == current;
    flat_button(
        xilem::view::label(text)
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
        move |st: &mut AppState| {
            if let Some(rs) = state_mut(st) { rs.mode = mode; rs.error = None; }
        },
    )
}

fn run_reset(st: &mut AppState) {
    let (oid, mode) = match state_get(st) {
        Some(s) => (s.oid.clone(), s.mode),
        None => return,
    };
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(s) = state_mut(st) { s.error = Some("demo mode".into()); }
        return;
    }
    if oid.is_empty() {
        if let Some(s) = state_mut(st) { s.error = Some("no target commit".into()); }
        return;
    }

    match git::ops::reset(&repo_path, &oid, mode) {
        Ok(()) => {
            st.refresh_all();
            let short = &oid[..oid.len().min(7)];
            let mode_str = match mode {
                ResetMode::Soft  => "soft",
                ResetMode::Mixed => "mixed",
                ResetMode::Hard  => "hard",
            };
            st.toast = Some(Toast::info(format!("reset --{mode_str} to {short}")));
            st.modal = None;
        }
        Err(e) => {
            if let Some(s) = state_mut(st) { s.error = Some(format!("{e:#}")); }
        }
    }
}

fn state_get(state: &AppState) -> Option<&ResetModalState> {
    match &state.modal { Some(Modal::Reset(s)) => Some(s), _ => None }
}

fn state_mut(state: &mut AppState) -> Option<&mut ResetModalState> {
    match &mut state.modal { Some(Modal::Reset(s)) => Some(s), _ => None }
}
