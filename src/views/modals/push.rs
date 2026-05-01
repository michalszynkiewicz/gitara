//! Push modal — pick a remote, optionally rename the target branch,
//! and run `git push [--force-with-lease] <remote> <local>:<target>`.

use crate::app::{AppState, Modal, PushModalState};
use crate::git;
use crate::theme::Theme;
use crate::ui::label;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::masonry::properties::types::AsUnit as _;
use xilem::style::{Padding, Style as _};
use xilem::view::{flex, sized_box, text_input, Axis, CrossAxisAlignment, FlexSpacer};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let s = push_state(state).cloned().unwrap_or_default();

    let remotes: Vec<String> = state.repo.remotes.iter().map(|r| r.name.clone()).collect();

    // Existing branches on the chosen remote, with the `<remote>/`
    // prefix stripped so we render bare branch names.
    let prefix = format!("{}/", s.remote);
    let mut existing_remote_branches: Vec<String> = state
        .repo
        .remotes
        .iter()
        .find(|r| r.name == s.remote)
        .map(|r| {
            r.branches
                .iter()
                .filter_map(|rb| rb.name.strip_prefix(&prefix).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    existing_remote_branches.sort();
    existing_remote_branches.dedup();

    let body = body_view(&s, &remotes, &existing_remote_branches, &theme).boxed();
    let footer = super::ok_cancel_footer(&theme, "Push", run_push);

    super::shell("Push", "Push the current branch", body, footer, &theme)
}

fn body_view(
    s: &PushModalState,
    remotes: &[String],
    existing_targets: &[String],
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    // ── source line: read-only "this is the local branch we're pushing"
    let source_label = label("source")
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM)
        .color(theme.text_dim);
    let source_view = label(if s.branch.is_empty() {
        "(no current branch)".to_string()
    } else {
        s.branch.clone()
    })
    .text_size(13.0)
    .weight(xilem::FontWeight::MEDIUM)
    .color(theme.text);

    // ── remote chip picker
    let remote_label = label("remote")
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM)
        .color(theme.text_dim);
    let remote_chips: Vec<Box<xilem::AnyWidgetView<AppState>>> = if remotes.is_empty() {
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

    // ── target branch input + suggestion chips for existing remote branches
    let target_label = label("target branch (on the remote)")
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM)
        .color(theme.text_dim);
    let target_input = sized_box(
        text_input(s.target_branch.clone(), |st: &mut AppState, new| {
            if let Some(ps) = push_state_mut(st) {
                ps.target_branch = new;
                ps.error = None;
            }
        })
        .on_enter(|st: &mut AppState, _| run_push(st))
        .text_color(super::input_text()),
    )
    .expand_width()
    .height((32.0_f64).px())
    .corner_radius(4.0)
    .background_color(super::input_bg())
    .border(theme.border, 1.0)
    .padding(Padding::from_vh(4.0, 8.0));

    let suggestions_view: Box<xilem::AnyWidgetView<AppState>> = if existing_targets.is_empty() {
        label("(no existing branches on this remote)")
            .text_size(10.0)
            .color(theme.text_dim)
            .boxed()
    } else {
        let suggestion_chips: Vec<Box<xilem::AnyWidgetView<AppState>>> = existing_targets
            .iter()
            .map(|name| target_chip(name, s.target_branch == *name, theme).boxed())
            .collect();
        flex(Axis::Vertical, suggestion_chips)
            .direction(Axis::Horizontal)
            .gap((4.0_f64).px())
            .boxed()
    };

    // ── force-with-lease toggle (unchanged from before)
    let force_color = if s.force_with_lease {
        theme.warn
    } else {
        theme.text_muted
    };
    let force_btn = flat_button(
        crate::ui::toggle_row(s.force_with_lease, "force with lease", force_color, 11.0),
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
            if let Some(ps) = push_state_mut(st) {
                ps.force_with_lease = !ps.force_with_lease;
            }
        },
    );
    let force_help = label(
        "rejects the push if the remote has commits we haven't seen — \
         safer than --force, still a rewrite",
    )
    .text_size(10.0)
    .color(theme.text_dim);

    // ── error / running banner
    let error_view: Box<xilem::AnyWidgetView<AppState>> = match (&s.error, s.running) {
        (Some(err), _) => label(err.clone())
            .text_size(11.0)
            .color(theme.removed)
            .boxed(),
        (_, true) => label("pushing…")
            .text_size(11.0)
            .color(theme.text_muted)
            .boxed(),
        _ => label("").boxed(),
    };

    flex(
        Axis::Vertical,
        (
            source_label,
            source_view,
            FlexSpacer::Fixed((12.0_f64).px()),
            remote_label,
            flex(Axis::Horizontal, remote_chips).gap((6.0_f64).px()),
            FlexSpacer::Fixed((12.0_f64).px()),
            target_label,
            target_input,
            FlexSpacer::Fixed((4.0_f64).px()),
            suggestions_view,
            FlexSpacer::Fixed((14.0_f64).px()),
            force_btn,
            force_help,
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
        crate::ui::label(name.to_string())
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
            if let Some(ps) = push_state_mut(st) {
                ps.remote = owned.clone();
                ps.error = None;
            }
        },
    )
}

/// Suggestion chip for an existing remote branch — clicking fills the
/// target-branch input with this name. Highlighted when the input
/// currently matches it.
fn target_chip(name: &str, selected: bool, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let owned = name.to_string();
    flat_button(
        crate::ui::label(name.to_string())
            .text_size(11.0)
            .color(if selected {
                theme.accent_fg
            } else {
                theme.text_muted
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
            if let Some(ps) = push_state_mut(st) {
                ps.target_branch = owned.clone();
                ps.error = None;
            }
        },
    )
}

fn run_push(st: &mut AppState) {
    let (remote, branch, target, force) = match push_state(st) {
        Some(s) => (
            s.remote.clone(),
            s.branch.clone(),
            s.target_branch.trim().to_string(),
            s.force_with_lease,
        ),
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
        if let Some(s) = push_state_mut(st) {
            s.error = Some("no current branch".into());
        }
        return;
    }
    if remote.is_empty() {
        if let Some(s) = push_state_mut(st) {
            s.error = Some("no remote configured".into());
        }
        return;
    }
    // Empty target falls back to the source branch name (push to a
    // same-named remote branch — the common case).
    let target = if target.is_empty() {
        branch.clone()
    } else {
        target
    };

    match git::ops::push(&repo_path, &remote, &branch, &target, force) {
        Ok(()) => {
            st.refresh_all();
            st.modal = None;
        }
        Err(e) => {
            if let Some(s) = push_state_mut(st) {
                s.error = Some(format!("{e:#}"));
            }
        }
    }
}

fn push_state(state: &AppState) -> Option<&PushModalState> {
    match &state.modal {
        Some(Modal::Push(s)) => Some(s),
        _ => None,
    }
}

fn push_state_mut(state: &mut AppState) -> Option<&mut PushModalState> {
    match &mut state.modal {
        Some(Modal::Push(s)) => Some(s),
        _ => None,
    }
}
