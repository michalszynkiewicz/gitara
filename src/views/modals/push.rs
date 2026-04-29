//! Push modal — pick a remote, optionally rename the target branch,
//! and run `git push [--force-with-lease] <remote> <local>:<target>`.

use crate::app::{AppState, Modal, PushModalState};
use crate::git;
use crate::theme::Theme;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::view::{flex, label, sized_box, textbox, Axis, CrossAxisAlignment, FlexSpacer, Padding};
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
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM);
    let source_view = label(if s.branch.is_empty() {
        "(no current branch)".to_string()
    } else {
        s.branch.clone()
    })
    .brush(theme.text)
    .text_size(13.0)
    .weight(xilem::FontWeight::MEDIUM);

    // ── remote chip picker
    let remote_label = label("remote")
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM);
    let remote_chips: Vec<Box<xilem::AnyWidgetView<AppState>>> = if remotes.is_empty() {
        vec![label("(no remotes configured)")
            .brush(theme.text_muted)
            .text_size(12.0)
            .boxed()]
    } else {
        remotes
            .iter()
            .map(|name| remote_chip(name, s.remote == *name, theme).boxed())
            .collect()
    };

    // ── target branch input + suggestion chips for existing remote branches
    let target_label = label("target branch (on the remote)")
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM);
    let target_input = sized_box(
        textbox(s.target_branch.clone(), |st: &mut AppState, new| {
            if let Some(ps) = push_state_mut(st) {
                ps.target_branch = new;
                ps.error = None;
            }
        })
        .on_enter(|st: &mut AppState, _| run_push(st))
        .brush(super::input_text()),
    )
    .expand_width()
    .height(32.0)
    .background(super::input_bg())
    .border(theme.border, 1.0)
    .rounded(4.0)
    .padding(Padding::from_vh(4.0, 8.0));

    let suggestions_view: Box<xilem::AnyWidgetView<AppState>> = if existing_targets.is_empty() {
        label("(no existing branches on this remote)")
            .brush(theme.text_dim)
            .text_size(10.0)
            .boxed()
    } else {
        let suggestion_chips: Vec<Box<xilem::AnyWidgetView<AppState>>> = existing_targets
            .iter()
            .map(|name| target_chip(name, s.target_branch == *name, theme).boxed())
            .collect();
        flex(suggestion_chips)
            .direction(Axis::Horizontal)
            .gap(4.0)
            .boxed()
    };

    // ── force-with-lease toggle (unchanged from before)
    let force_label = if s.force_with_lease {
        "✓ force with lease"
    } else {
        "force with lease"
    };
    let force_btn = flat_button(
        xilem::view::label(force_label)
            .brush(if s.force_with_lease {
                theme.warn
            } else {
                theme.text_muted
            })
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
            if let Some(ps) = push_state_mut(st) {
                ps.force_with_lease = !ps.force_with_lease;
            }
        },
    );
    let force_help = label(
        "rejects the push if the remote has commits we haven't seen — \
         safer than --force, still a rewrite",
    )
    .brush(theme.text_dim)
    .text_size(10.0);

    // ── error / running banner
    let error_view: Box<xilem::AnyWidgetView<AppState>> = match (&s.error, s.running) {
        (Some(err), _) => label(err.clone())
            .brush(theme.removed)
            .text_size(11.0)
            .boxed(),
        (_, true) => label("pushing…")
            .brush(theme.text_muted)
            .text_size(11.0)
            .boxed(),
        _ => label("").boxed(),
    };

    flex((
        source_label,
        source_view,
        FlexSpacer::Fixed(12.0),
        remote_label,
        flex(remote_chips).direction(Axis::Horizontal).gap(6.0),
        FlexSpacer::Fixed(12.0),
        target_label,
        target_input,
        FlexSpacer::Fixed(4.0),
        suggestions_view,
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

fn remote_chip(name: &str, selected: bool, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let owned = name.to_string();
    flat_button(
        xilem::view::label(name.to_string())
            .brush(if selected {
                theme.accent_fg
            } else {
                theme.text
            })
            .text_size(11.0)
            .weight(if selected {
                xilem::FontWeight::MEDIUM
            } else {
                xilem::FontWeight::NORMAL
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
        xilem::view::label(name.to_string())
            .brush(if selected {
                theme.accent_fg
            } else {
                theme.text_muted
            })
            .text_size(11.0),
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
