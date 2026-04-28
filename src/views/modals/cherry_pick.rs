//! Cherry-pick modal — pre-fills with the currently selected commit.

use crate::app::{AppState, CherryPickModalState, Modal, Toast};
use crate::git;
use crate::theme::Theme;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::view::{flex, label, sized_box, textbox, Axis, CrossAxisAlignment, FlexSpacer, Padding};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let mut s = state_get(state).cloned().unwrap_or_default();

    // Default the oid from the current selection on first render.
    // Skip the working-tree sentinel — it isn't a real commit.
    if s.oid.is_empty() {
        if let Some(sel) = &state.selection.primary {
            if sel != crate::views::graph::WORKING_TREE_OID {
                s.oid = sel.clone();
                if let Some(cs) = state_mut(state) { cs.oid = s.oid.clone(); }
            }
        }
    }

    let body = body_view(&s, &theme).boxed();
    let footer = super::ok_cancel_footer(&theme, "Cherry-pick", run_cherry_pick);

    super::shell(
        "Cherry-pick",
        "Apply a commit on top of the current branch",
        body,
        footer,
        &theme,
    )
}

fn body_view(s: &CherryPickModalState, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let oid_label = label("commit").brush(theme.text_dim).text_size(10.0).weight(xilem::FontWeight::MEDIUM);
    let oid_input = sized_box(
        textbox(s.oid.clone(), |st: &mut AppState, new| {
            if let Some(cs) = state_mut(st) { cs.oid = new; cs.error = None; }
        })
        .on_enter(|st: &mut AppState, _| run_cherry_pick(st))
        .brush(super::input_text()),
    )
    .expand_width()
    .height(32.0)
    .background(super::input_bg())
    .border(theme.border, 1.0)
    .rounded(4.0)
    .padding(Padding::from_vh(4.0, 8.0));

    let no_commit_label = if s.no_commit { "✓ stage only (no commit)" } else { "stage only (no commit)" };
    let no_commit_btn = flat_button(
        xilem::view::label(no_commit_label)
            .brush(if s.no_commit { theme.accent } else { theme.text_muted })
            .text_size(11.0),
        FlatStyle {
            idle_bg: None,
            hover_bg: theme.bg_hover,
            active_bg: Some(theme.accent_tint),
            radius: 4.0,
            padding_v: 4.0,
            padding_h: 8.0,
        },
        s.no_commit,
        |st: &mut AppState| {
            if let Some(cs) = state_mut(st) { cs.no_commit = !cs.no_commit; }
        },
    );

    let error_view: Box<xilem::AnyWidgetView<AppState>> = match &s.error {
        Some(err) => label(err.clone()).brush(theme.removed).text_size(11.0).boxed(),
        None => label("").boxed(),
    };

    flex((
        oid_label,
        oid_input,
        FlexSpacer::Fixed(12.0),
        no_commit_btn,
        FlexSpacer::Fixed(8.0),
        error_view,
    ))
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(4.0)
}

fn run_cherry_pick(st: &mut AppState) {
    let (oid, no_commit) = match state_get(st) {
        Some(s) => (s.oid.trim().to_string(), s.no_commit),
        None => return,
    };
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(s) = state_mut(st) { s.error = Some("demo mode".into()); }
        return;
    }
    if oid.is_empty() {
        if let Some(s) = state_mut(st) { s.error = Some("no commit selected".into()); }
        return;
    }

    let result = if no_commit {
        std::process::Command::new("git")
            .current_dir(&repo_path)
            .args(["cherry-pick", "--no-commit", &oid])
            .output()
            .map_err(anyhow::Error::from)
            .and_then(|o| {
                if o.status.success() { Ok(()) } else {
                    let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                    anyhow::bail!(err)
                }
            })
    } else {
        git::ops::cherry_pick(&repo_path, &[&oid])
    };

    match result {
        Ok(()) => {
            st.refresh_all();
            let short = &oid[..oid.len().min(7)];
            st.toast = Some(Toast::info(format!("cherry-picked {short}")));
            st.modal = None;
        }
        Err(e) => {
            if let Some(s) = state_mut(st) { s.error = Some(format!("{e:#}")); }
        }
    }
}

fn state_get(state: &AppState) -> Option<&CherryPickModalState> {
    match &state.modal { Some(Modal::CherryPick(s)) => Some(s), _ => None }
}

fn state_mut(state: &mut AppState) -> Option<&mut CherryPickModalState> {
    match &mut state.modal { Some(Modal::CherryPick(s)) => Some(s), _ => None }
}
