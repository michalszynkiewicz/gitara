//! Rename-branch modal — `git branch -m <old> <new>`.

use crate::app::{AppState, Modal, RenameBranchModalState, Toast};
use crate::git;
use crate::theme::Theme;
use xilem::view::{flex, label, sized_box, textbox, Axis, CrossAxisAlignment, FlexSpacer, Padding};
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
    let new_label = label("new name").brush(theme.text_dim).text_size(10.0).weight(xilem::FontWeight::MEDIUM);
    let new_input = sized_box(
        textbox(s.new_name.clone(), |st: &mut AppState, new| {
            if let Some(rs) = state_mut(st) { rs.new_name = new; rs.error = None; }
        })
        .on_enter(|st: &mut AppState, _| run_rename(st))
        .brush(super::input_text()),
    )
    .expand_width()
    .height(32.0)
    .background(super::input_bg())
    .border(theme.border, 1.0)
    .rounded(4.0)
    .padding(Padding::from_vh(4.0, 8.0));

    let error_view: Box<xilem::AnyWidgetView<AppState>> = match &s.error {
        Some(err) => label(err.clone()).brush(theme.removed).text_size(11.0).boxed(),
        None => label("").boxed(),
    };

    flex((
        new_label,
        new_input,
        FlexSpacer::Fixed(8.0),
        error_view,
    ))
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(4.0)
}

fn run_rename(st: &mut AppState) {
    let (old, new) = match state_get(st) {
        Some(s) => (s.old_name.clone(), s.new_name.trim().to_string()),
        None => return,
    };
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(s) = state_mut(st) { s.error = Some("demo mode".into()); }
        return;
    }
    if new.is_empty() {
        if let Some(s) = state_mut(st) { s.error = Some("new name is empty".into()); }
        return;
    }

    match git::ops::rename_branch(&repo_path, &old, &new) {
        Ok(()) => {
            st.refresh_all();
            st.toast = Some(Toast::info(format!("renamed {old} → {new}")));
            st.modal = None;
        }
        Err(e) => {
            if let Some(s) = state_mut(st) { s.error = Some(format!("{e:#}")); }
        }
    }
}

fn state_get(state: &AppState) -> Option<&RenameBranchModalState> {
    match &state.modal { Some(Modal::RenameBranch(s)) => Some(s), _ => None }
}

fn state_mut(state: &mut AppState) -> Option<&mut RenameBranchModalState> {
    match &mut state.modal { Some(Modal::RenameBranch(s)) => Some(s), _ => None }
}
