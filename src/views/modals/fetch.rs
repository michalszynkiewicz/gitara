//! Fetch modal — picks a remote, runs `git fetch [--prune] <remote>`.

use crate::app::{AppState, FetchModalState, Modal};
use crate::git;
use crate::theme::Theme;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::view::{flex, label, Axis, CrossAxisAlignment, FlexSpacer};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let s = fetch_state(state).cloned().unwrap_or_default();
    let remotes: Vec<String> = state.repo.remotes.iter().map(|r| r.name.clone()).collect();

    let body = body_view(&s, &remotes, &theme).boxed();
    let footer = super::ok_cancel_footer(&theme, "Fetch", run_fetch);

    super::shell("Fetch", "Fetch refs from a remote", body, footer, &theme)
}

fn body_view(
    s: &FetchModalState,
    remotes: &[String],
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    let header = label("remote")
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM);

    let chips: Vec<_> = if remotes.is_empty() {
        vec![label("(no remotes configured)")
            .brush(theme.text_muted).text_size(12.0).boxed()]
    } else {
        remotes
            .iter()
            .map(|name| remote_chip(name, s.remote == *name, theme).boxed())
            .collect()
    };

    let prune_label = if s.prune { "✓ prune deleted remote refs" } else { "prune deleted remote refs" };
    let prune_btn = flat_button(
        xilem::view::label(prune_label)
            .brush(if s.prune { theme.accent } else { theme.text_muted })
            .text_size(11.0),
        FlatStyle {
            idle_bg: None,
            hover_bg: theme.bg_hover,
            active_bg: Some(theme.accent_tint),
            radius: 4.0,
            padding_v: 4.0,
            padding_h: 8.0,
        },
        s.prune,
        |st: &mut AppState| {
            if let Some(fs) = fetch_state_mut(st) { fs.prune = !fs.prune; }
        },
    );

    let error_view: Box<xilem::AnyWidgetView<AppState>> = match (&s.error, s.running) {
        (Some(err), _) => label(err.clone()).brush(theme.removed).text_size(11.0).boxed(),
        (_, true)      => label("fetching…").brush(theme.text_muted).text_size(11.0).boxed(),
        _              => label("").boxed(),
    };

    flex((
        header,
        flex(chips).direction(Axis::Horizontal).gap(6.0),
        FlexSpacer::Fixed(12.0),
        prune_btn,
        FlexSpacer::Fixed(8.0),
        error_view,
    ))
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(4.0)
}

fn remote_chip(name: &str, selected: bool, theme: &Theme) -> impl xilem::WidgetView<AppState> {
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
        move |st: &mut AppState| {
            if let Some(fs) = fetch_state_mut(st) { fs.remote = owned.clone(); }
        },
    )
}

fn run_fetch(st: &mut AppState) {
    let (remote, prune) = match fetch_state(st) {
        Some(s) => (s.remote.clone(), s.prune),
        None => return,
    };
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(s) = fetch_state_mut(st) {
            s.error = Some(
                "demo mode — start gitara from a real repo or set GITARA_REPO=<path>".into(),
            );
        }
        return;
    }
    if remote.trim().is_empty() {
        if let Some(s) = fetch_state_mut(st) {
            s.error = Some("pick a remote first".into());
        }
        return;
    }

    match git::ops::fetch(&repo_path, &remote, prune) {
        Ok(()) => {
            st.refresh_all();
            st.modal = None;
        }
        Err(e) => {
            if let Some(s) = fetch_state_mut(st) {
                s.error = Some(format!("{e:#}"));
            }
        }
    }
}

fn fetch_state(state: &AppState) -> Option<&FetchModalState> {
    match &state.modal { Some(Modal::Fetch(s)) => Some(s), _ => None }
}

fn fetch_state_mut(state: &mut AppState) -> Option<&mut FetchModalState> {
    match &mut state.modal { Some(Modal::Fetch(s)) => Some(s), _ => None }
}
