//! Toolbar: 36px row of flat buttons.
//! Primary action (Commit when dirty, else Push when ahead) is accent-filled.

use crate::app::{AppState, Modal};
use crate::theme::Theme;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use vello::peniko::Color;
use xilem::view::{flex, Axis, CrossAxisAlignment, FlexSpacer};

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = &state.theme;
    // Primary action heuristic: if main is ahead of upstream → push; else commit.
    let primary_is_push = state
        .repo
        .branches
        .iter()
        .find(|b| b.current)
        .map(|b| b.ahead > 0)
        .unwrap_or(false);

    flex((
        tb(theme, "Commit", primary_is_push == false, |s: &mut AppState| {
            let st = crate::app::CommitModalState::open(&s.repo.path);
            s.modal = Some(Modal::Commit(st));
        }),
        tb(theme, "Fetch",  false, |s: &mut AppState| {
            // Default to first remote in the repo (usually "origin").
            let remote = s.repo.remotes.first().map(|r| r.name.clone()).unwrap_or_default();
            s.modal = Some(Modal::Fetch(crate::app::FetchModalState {
                remote, prune: false, error: None, running: false,
            }));
        }),
        tb(theme, "Pull",   false, |s: &mut AppState| run_pull(s)),
        tb(theme, "Push",   primary_is_push, |s: &mut AppState| {
            let remote = s.repo.remotes.first().map(|r| r.name.clone()).unwrap_or_default();
            let branch = s.repo.branches.iter().find(|b| b.current).map(|b| b.name.clone()).unwrap_or_default();
            s.modal = Some(Modal::Push(crate::app::PushModalState {
                remote,
                target_branch: branch.clone(),
                branch,
                force_with_lease: false, error: None, running: false,
            }));
        }),
        FlexSpacer::Fixed(12.0),
        tb(theme, "Branch", false, |s: &mut AppState| {
            s.modal = Some(Modal::Branch(crate::app::BranchModalState::default()));
        }),
        tb(theme, "Merge",  false, |s: &mut AppState| {
            s.modal = Some(Modal::Merge(crate::app::MergeModalState::default()));
        }),
        tb(theme, "Rebase", false, |s: &mut AppState| {
            s.modal = Some(Modal::Rebase(crate::app::RebaseModalState::default()));
        }),
        FlexSpacer::Flex(1.0),
        tb(theme, "Refresh", false, |s: &mut AppState| {
            s.refresh_all();
            s.toast = Some(crate::app::Toast::info("refreshed".into()));
        }),
    ))
    .direction(Axis::Horizontal)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap(2.0)
}

fn tb<F>(theme: &Theme, text: &'static str, primary: bool, cb: F)
    -> impl xilem::WidgetView<AppState>
where
    F: Fn(&mut AppState) + Send + Sync + 'static,
{
    let fg = if primary { theme.accent_fg } else { theme.text };
    let lbl = xilem::view::label(text).brush(fg).text_size(12.0);
    flat_button(
        lbl,
        FlatStyle {
            idle_bg: if primary { Some(theme.accent) } else { None },
            hover_bg: if primary { theme.accent_hover } else { theme.bg_hover },
            active_bg: None,
            radius: 4.0,
            padding_v: 5.0,
            padding_h: 10.0,
        },
        false,
        move |s: &mut AppState| cb(s),
    )
}

// Suppress unused-import warning when this module recompiles.
#[allow(dead_code)]
fn _mute_color(_: Color) {}

fn run_pull(st: &mut AppState) {
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        st.toast = Some(crate::app::Toast::error(
            "demo mode — set GITARA_REPO=<path>".into(),
        ));
        return;
    }
    match crate::git::ops::pull(&repo_path) {
        Ok(()) => {
            st.refresh_all();
            st.toast = Some(crate::app::Toast::info("pulled".into()));
        }
        Err(e) => {
            st.toast = Some(crate::app::Toast::error(format!("pull failed: {e:#}")));
        }
    }
}
