//! Merge modal — picks a branch to merge into HEAD; --no-ff toggle.

use crate::app::{AppState, MergeModalState, Modal, Toast};
use crate::git;
use crate::theme::Theme;
use crate::ui::label;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::masonry::properties::types::AsUnit as _;
use xilem::style::Style as _;
use xilem::view::{flex, Axis, CrossAxisAlignment, FlexSpacer};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let s = merge_state(state).cloned().unwrap_or_default();
    let branches: Vec<String> = state
        .repo
        .branches
        .iter()
        .filter(|b| !b.current)
        .map(|b| b.name.clone())
        .collect();

    let body = body_view(&s, &branches, &theme).boxed();
    let footer = super::ok_cancel_footer(&theme, "Merge", run_merge);

    super::shell(
        "Merge",
        "Merge another branch into the current one",
        body,
        footer,
        &theme,
    )
}

fn body_view(
    s: &MergeModalState,
    branches: &[String],
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    let header = label("from branch")
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM)
        .color(theme.text_dim);

    let chips: Vec<_> = if branches.is_empty() {
        vec![label("(no other branches)")
            .text_size(12.0)
            .color(theme.text_muted)
            .boxed()]
    } else {
        branches
            .iter()
            .map(|name| {
                super::ref_chip(name, s.branch == *name, theme, |st, picked| {
                    if let Some(ms) = merge_state_mut(st) {
                        ms.branch = picked;
                        ms.error = None;
                    }
                })
                .boxed()
            })
            .collect()
    };

    let no_ff_label = if s.no_ff {
        "✓ no fast-forward (always create merge commit)"
    } else {
        "no fast-forward (always create merge commit)"
    };
    let no_ff_btn = flat_button(
        crate::ui::label(no_ff_label)
            .text_size(11.0)
            .color(if s.no_ff {
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
        s.no_ff,
        |st: &mut AppState| {
            if let Some(ms) = merge_state_mut(st) {
                ms.no_ff = !ms.no_ff;
            }
        },
    );

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
            header,
            flex(Axis::Horizontal, chips).gap((6.0_f64).px()),
            FlexSpacer::Fixed((12.0_f64).px()),
            no_ff_btn,
            FlexSpacer::Fixed((8.0_f64).px()),
            error_view,
        ),
    )
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap((4.0_f64).px())
}

fn run_merge(st: &mut AppState) {
    let (branch, no_ff) = match merge_state(st) {
        Some(s) => (s.branch.clone(), s.no_ff),
        None => return,
    };
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(s) = merge_state_mut(st) {
            s.error = Some("demo mode".into());
        }
        return;
    }
    if branch.is_empty() {
        if let Some(s) = merge_state_mut(st) {
            s.error = Some("pick a branch to merge".into());
        }
        return;
    }

    let result = git::ops::merge(&repo_path, &branch, no_ff);

    match result {
        Ok(()) => {
            st.refresh_all();
            st.toast = Some(Toast::info(format!("merged {branch}")));
            st.modal = None;
        }
        Err(e) => {
            if let Some(s) = merge_state_mut(st) {
                s.error = Some(format!("{e:#}"));
            }
        }
    }
}

fn merge_state(state: &AppState) -> Option<&MergeModalState> {
    match &state.modal {
        Some(Modal::Merge(s)) => Some(s),
        _ => None,
    }
}

fn merge_state_mut(state: &mut AppState) -> Option<&mut MergeModalState> {
    match &mut state.modal {
        Some(Modal::Merge(s)) => Some(s),
        _ => None,
    }
}
