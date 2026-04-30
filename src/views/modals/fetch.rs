//! Fetch modal — picks a remote, runs `git fetch [--prune] <remote>`.

use crate::app::{AppState, FetchModalState, Modal};
use crate::git;
use crate::theme::Theme;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::masonry::properties::types::AsUnit as _;
use xilem::style::Style as _;
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
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM)
        .color(theme.text_dim);

    let chips: Vec<_> = if remotes.is_empty() {
        vec![label("(no remotes configured)")
            .text_size(12.0)
            .color(theme.text_muted)
            .boxed()]
    } else {
        remotes
            .iter()
            .map(|name| remote_chip(name, s.remote == *name, theme).boxed())
            .collect()
    };

    let prune_label = if s.prune {
        "✓ prune deleted remote refs"
    } else {
        "prune deleted remote refs"
    };
    let prune_btn = flat_button(
        xilem::view::label(prune_label)
            .text_size(11.0)
            .color(if s.prune {
                theme.accent
            } else {
                theme.text_muted
            }),
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
            if let Some(fs) = fetch_state_mut(st) {
                fs.prune = !fs.prune;
            }
        },
    );

    let error_view: Box<xilem::AnyWidgetView<AppState>> = match (&s.error, s.running) {
        (Some(err), _) => label(err.clone())
            .text_size(11.0)
            .color(theme.removed)
            .boxed(),
        (_, true) => label("fetching…")
            .text_size(11.0)
            .color(theme.text_muted)
            .boxed(),
        _ => label("").boxed(),
    };

    flex(
        Axis::Vertical,
        (
            header,
            flex(Axis::Horizontal, chips).gap((6.0_f64).px()),
            FlexSpacer::Fixed((12.0_f64).px()),
            prune_btn,
            FlexSpacer::Fixed((8.0_f64).px()),
            error_view,
        ),
    )
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap((4.0_f64).px())
}

fn remote_chip(name: &str, selected: bool, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let owned = name.to_string();
    flat_button(
        xilem::view::label(name.to_string())
            .text_size(11.0)
            .weight(if selected {
                xilem::FontWeight::MEDIUM
            } else {
                xilem::FontWeight::NORMAL
            })
            .color(if selected {
                theme.accent_fg
            } else {
                theme.text
            }),
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
            if let Some(fs) = fetch_state_mut(st) {
                fs.remote = owned.clone();
            }
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
            s.error =
                Some("demo mode — start gitara from a real repo or set GITARA_REPO=<path>".into());
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
    match &state.modal {
        Some(Modal::Fetch(s)) => Some(s),
        _ => None,
    }
}

fn fetch_state_mut(state: &mut AppState) -> Option<&mut FetchModalState> {
    match &mut state.modal {
        Some(Modal::Fetch(s)) => Some(s),
        _ => None,
    }
}
