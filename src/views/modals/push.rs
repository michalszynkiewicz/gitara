//! Push modal — `git push [--force-with-lease] <remote> <branch>`.

use crate::app::{AppState, Modal, PushModalState};
use crate::git;
use crate::theme::Theme;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::view::{flex, label, Axis, CrossAxisAlignment, FlexSpacer};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let s = push_state(state).cloned().unwrap_or_default();

    let body = body_view(&s, &theme).boxed();
    let footer = super::ok_cancel_footer(&theme, "Push", run_push);

    super::shell("Push", "Push the current branch", body, footer, &theme)
}

fn body_view(s: &PushModalState, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let summary = if s.branch.is_empty() {
        "(no current branch)".to_string()
    } else if s.remote.is_empty() {
        format!("{}  →  (no remote configured)", s.branch)
    } else {
        format!("{}  →  {}/{}", s.branch, s.remote, s.branch)
    };

    let summary_view = label(summary)
        .brush(theme.text)
        .text_size(13.0)
        .weight(xilem::FontWeight::MEDIUM);

    let force_label = if s.force_with_lease {
        "✓ force with lease"
    } else {
        "force with lease"
    };
    let force_btn = flat_button(
        xilem::view::label(force_label)
            .brush(if s.force_with_lease { theme.warn } else { theme.text_muted })
            .text_size(11.0),
        FlatStyle {
            idle_bg: None,
            hover_bg: theme.bg_hover,
            active_bg: Some(super::tinted_warn(theme)),
            radius: 4.0,
            padding_v: 4.0,
            padding_h: 8.0,
        },
        s.force_with_lease,
        |st: &mut AppState| {
            if let Some(ps) = push_state_mut(st) { ps.force_with_lease = !ps.force_with_lease; }
        },
    );

    let force_help = label(
        "rejects the push if the remote has commits we haven't seen — \
         safer than --force, still a rewrite",
    )
    .brush(theme.text_dim)
    .text_size(10.0);

    let error_view: Box<xilem::AnyWidgetView<AppState>> = match (&s.error, s.running) {
        (Some(err), _) => label(err.clone()).brush(theme.removed).text_size(11.0).boxed(),
        (_, true)      => label("pushing…").brush(theme.text_muted).text_size(11.0).boxed(),
        _              => label("").boxed(),
    };

    flex((
        summary_view,
        FlexSpacer::Fixed(14.0),
        force_btn,
        force_help,
        FlexSpacer::Fixed(8.0),
        error_view,
    ))
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(4.0)
}

fn run_push(st: &mut AppState) {
    let (remote, branch, force) = match push_state(st) {
        Some(s) => (s.remote.clone(), s.branch.clone(), s.force_with_lease),
        None => return,
    };
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(s) = push_state_mut(st) {
            s.error = Some("demo mode — set GITARA_REPO=<path>".into());
        }
        return;
    }
    if branch.is_empty() {
        if let Some(s) = push_state_mut(st) { s.error = Some("no current branch".into()); }
        return;
    }
    if remote.is_empty() {
        if let Some(s) = push_state_mut(st) { s.error = Some("no remote configured".into()); }
        return;
    }

    match git::ops::push(&repo_path, &remote, &branch, force) {
        Ok(()) => {
            st.refresh_all();
            st.modal = None;
        }
        Err(e) => {
            if let Some(s) = push_state_mut(st) { s.error = Some(format!("{e:#}")); }
        }
    }
}

fn push_state(state: &AppState) -> Option<&PushModalState> {
    match &state.modal { Some(Modal::Push(s)) => Some(s), _ => None }
}

fn push_state_mut(state: &mut AppState) -> Option<&mut PushModalState> {
    match &mut state.modal { Some(Modal::Push(s)) => Some(s), _ => None }
}
