//! Rebase modal — picks a base ref for `git rebase <onto>`.

use crate::app::{AppState, Modal, RebaseModalState, Toast};
use crate::git;
use crate::theme::Theme;
use xilem::view::{flex, label, Axis, CrossAxisAlignment, FlexSpacer};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let s = rebase_state(state).cloned().unwrap_or_default();
    let branches: Vec<String> = state
        .repo
        .branches
        .iter()
        .filter(|b| !b.current)
        .map(|b| b.name.clone())
        .collect();

    let body = body_view(&s, &branches, &theme).boxed();
    let footer = super::ok_cancel_footer(&theme, "Rebase", run_rebase);

    super::shell(
        "Rebase",
        "Replay current branch's commits onto another base",
        body,
        footer,
        &theme,
    )
}

fn body_view(
    s: &RebaseModalState,
    branches: &[String],
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    let header = label("onto")
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM);

    let chips: Vec<_> = if branches.is_empty() {
        vec![label("(no other branches)")
            .brush(theme.text_muted)
            .text_size(12.0)
            .boxed()]
    } else {
        branches
            .iter()
            .map(|name| {
                super::ref_chip(name, s.onto == *name, theme, |st, picked| {
                    if let Some(rs) = rebase_state_mut(st) {
                        rs.onto = picked;
                        rs.error = None;
                    }
                })
                .boxed()
            })
            .collect()
    };

    let warning = label("this rewrites your current branch — push will require --force-with-lease")
        .brush(theme.warn)
        .text_size(11.0);

    let error_view: Box<xilem::AnyWidgetView<AppState>> = match &s.error {
        Some(err) => label(err.clone())
            .brush(theme.removed)
            .text_size(11.0)
            .boxed(),
        None => label("").boxed(),
    };

    flex((
        header,
        flex(chips).direction(Axis::Horizontal).gap(6.0),
        FlexSpacer::Fixed(12.0),
        warning,
        FlexSpacer::Fixed(8.0),
        error_view,
    ))
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(4.0)
}

fn run_rebase(st: &mut AppState) {
    let onto = match rebase_state(st) {
        Some(s) => s.onto.clone(),
        None => return,
    };
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(s) = rebase_state_mut(st) {
            s.error = Some("demo mode".into());
        }
        return;
    }
    if onto.is_empty() {
        if let Some(s) = rebase_state_mut(st) {
            s.error = Some("pick a base".into());
        }
        return;
    }

    match git::ops::rebase(&repo_path, &onto) {
        Ok(()) => {
            st.refresh_all();
            st.toast = Some(Toast::info(format!("rebased onto {onto}")));
            st.modal = None;
        }
        Err(e) => {
            if let Some(s) = rebase_state_mut(st) {
                s.error = Some(format!("{e:#}"));
            }
        }
    }
}

fn rebase_state(state: &AppState) -> Option<&RebaseModalState> {
    match &state.modal {
        Some(Modal::Rebase(s)) => Some(s),
        _ => None,
    }
}

fn rebase_state_mut(state: &mut AppState) -> Option<&mut RebaseModalState> {
    match &mut state.modal {
        Some(Modal::Rebase(s)) => Some(s),
        _ => None,
    }
}
