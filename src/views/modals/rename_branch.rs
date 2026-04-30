//! Rename-branch modal — `git branch -m <old> <new>`.

use crate::app::{AppState, Modal, RenameBranchModalState, Toast};
use crate::git;
use crate::theme::Theme;
use xilem::masonry::properties::types::AsUnit as _;
use xilem::style::{Padding, Style as _};
use xilem::view::{flex, label, sized_box, text_input, Axis, CrossAxisAlignment, FlexSpacer};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let s = state_get(state).cloned().unwrap_or_default();

    let body = body_view(&s, &theme).boxed();
    let footer = super::ok_cancel_footer(&theme, "Rename", run_rename);

    let subtitle = format!("Rename branch {}", s.old_name);
    super::shell("Rename branch", &subtitle, body, footer, &theme)
}

fn body_view(s: &RenameBranchModalState, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let new_label = label("new name")
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM)
        .color(theme.text_dim);
    let new_input = sized_box(
        text_input(s.new_name.clone(), |st: &mut AppState, new| {
            if let Some(rs) = state_mut(st) {
                rs.new_name = new;
                rs.error = None;
            }
        })
        .on_enter(|st: &mut AppState, _| run_rename(st))
        .text_color(super::input_text()),
    )
    .expand_width()
    .height((32.0_f64).px())
    .corner_radius(4.0)
    .background_color(super::input_bg())
    .border(theme.border, 1.0)
    .padding(Padding::from_vh(4.0, 8.0));

    let error_view: Box<xilem::AnyWidgetView<AppState>> = match &s.error {
        Some(err) => label(err.clone())
            .text_size(11.0)
            .color(theme.removed)
            .boxed(),
        None => label("").boxed(),
    };

    flex(
        Axis::Vertical,
        (
            new_label,
            new_input,
            FlexSpacer::Fixed((8.0_f64).px()),
            error_view,
        ),
    )
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap((4.0_f64).px())
}

fn run_rename(st: &mut AppState) {
    let (old, new) = match state_get(st) {
        Some(s) => (s.old_name.clone(), s.new_name.trim().to_string()),
        None => return,
    };
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(s) = state_mut(st) {
            s.error = Some("demo mode".into());
        }
        return;
    }
    if new.is_empty() {
        if let Some(s) = state_mut(st) {
            s.error = Some("new name is empty".into());
        }
        return;
    }

    match git::ops::rename_branch(&repo_path, &old, &new) {
        Ok(()) => {
            st.refresh_all();
            st.toast = Some(Toast::info(format!("renamed {old} → {new}")));
            st.modal = None;
        }
        Err(e) => {
            if let Some(s) = state_mut(st) {
                s.error = Some(format!("{e:#}"));
            }
        }
    }
}

fn state_get(state: &AppState) -> Option<&RenameBranchModalState> {
    match &state.modal {
        Some(Modal::RenameBranch(s)) => Some(s),
        _ => None,
    }
}

fn state_mut(state: &mut AppState) -> Option<&mut RenameBranchModalState> {
    match &mut state.modal {
        Some(Modal::RenameBranch(s)) => Some(s),
        _ => None,
    }
}
