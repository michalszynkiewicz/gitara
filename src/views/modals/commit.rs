//! Commit workbench — large modal with two file lists (Unstaged /
//! Staged), a scrollable diff pane for the selected file, and a
//! message + amend + Commit/Cancel footer.

use std::path::PathBuf;

use crate::app::{AppState, CommitModalState, Modal};
use crate::theme::Theme;
use crate::ui::label;
use crate::widgets::clickable_box::{clickable_box, ClickStyle};
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::masonry::properties::types::AsUnit as _;
use xilem::style::{Padding, Style as _};
use xilem::view::{
    flex, portal, sized_box, text_input, Axis, CrossAxisAlignment, FlexExt as _, FlexSpacer,
};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let s = commit_state(state).cloned().unwrap_or_default();

    let body = body_view(&s, &theme).boxed();
    let footer = footer_view(&s, &theme);

    super::shell_large(
        "Commit",
        "Stage files and review the diff",
        body,
        footer,
        &theme,
    )
}

fn body_view(s: &CommitModalState, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let main_row = flex(
        Axis::Vertical,
        (
            sized_box(left_column(s, theme))
                .width((280.0_f64).px())
                .expand_height(),
            FlexSpacer::Fixed((12.0_f64).px()),
            right_pane(s, theme).flex(1.0),
        ),
    )
    .direction(Axis::Horizontal)
    .cross_axis_alignment(CrossAxisAlignment::Fill);

    let msg_label = label("message  (Enter commits — Shift+Enter or Ctrl/Cmd+Enter for newline)")
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM)
        .color(theme.text_dim);

    let msg_box = sized_box(
        text_input(s.message.clone(), |st: &mut AppState, new| {
            if let Some(cs) = commit_state_mut(st) {
                cs.message = new;
                cs.error = None;
            }
        })
        .on_enter(|st: &mut AppState, _| run_commit(st))
        .insert_newline(xilem::InsertNewline::OnShiftEnter)
        .text_color(super::input_text()),
    )
    .expand_width()
    .height((90.0_f64).px())
    .corner_radius(4.0)
    .background_color(super::input_bg())
    .border(theme.border, 1.0)
    .padding(Padding::from_vh(4.0, 8.0));

    let amend_color = if s.amend {
        theme.warn
    } else {
        theme.text_muted
    };
    let amend_btn = flat_button(
        crate::ui::toggle_row(s.amend, "amend last commit", amend_color, 11.0),
        FlatStyle {
            idle_bg: None,
            hover_bg: theme.bg_hover,
            active_bg: Some(super::tinted_warn(theme)),
            radius: 4.0,
            padding_v: 4.0,
            padding_h: 8.0,
        },
        s.amend,
        |st: &mut AppState| {
            if let Some(cs) = commit_state_mut(st) {
                cs.amend = !cs.amend;
                cs.error = None;
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
            msg_label,
            msg_box,
            FlexSpacer::Fixed((6.0_f64).px()),
            amend_btn,
            FlexSpacer::Fixed((4.0_f64).px()),
            error_view,
            FlexSpacer::Fixed((10.0_f64).px()),
            main_row.flex(1.0),
        ),
    )
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .must_fill_major_axis(true)
    .gap((4.0_f64).px())
}

fn left_column(s: &CommitModalState, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let unstaged_header = section_header("Unstaged", s.unstaged.len(), theme);
    let staged_header = section_header("Staged", s.staged.len(), theme);

    let unstaged_rows: Vec<Box<xilem::AnyWidgetView<AppState>>> = if s.unstaged.is_empty() {
        vec![placeholder_row("clean — nothing to stage", theme).boxed()]
    } else {
        s.unstaged
            .iter()
            .map(|f| {
                file_row(
                    f,
                    /*staged=*/ false,
                    is_selected(s, &f.path, false),
                    theme,
                )
                .boxed()
            })
            .collect()
    };
    let staged_rows: Vec<Box<xilem::AnyWidgetView<AppState>>> = if s.staged.is_empty() {
        vec![placeholder_row("nothing staged yet", theme).boxed()]
    } else {
        s.staged
            .iter()
            .map(|f| {
                file_row(
                    f,
                    /*staged=*/ true,
                    is_selected(s, &f.path, true),
                    theme,
                )
                .boxed()
            })
            .collect()
    };

    let unstaged_list = sized_box(
        flex(Axis::Vertical, unstaged_rows)
            .direction(Axis::Vertical)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap((1.0_f64).px()),
    )
    .expand_width();
    let staged_list = sized_box(
        flex(Axis::Vertical, staged_rows)
            .direction(Axis::Vertical)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap((1.0_f64).px()),
    )
    .expand_width();

    // Proportional split: each list claims space in ratio to its row
    // count so the busier side gets more height. Minimum factor of 1
    // keeps an empty side from collapsing entirely (its placeholder
    // row still needs a sliver).
    let unstaged_factor = (s.unstaged.len().max(1)) as f64;
    let staged_factor = (s.staged.len().max(1)) as f64;

    sized_box(
        flex(
            Axis::Vertical,
            (
                unstaged_header,
                // Wrap portals in sized_box(...).expand_height() — masonry's
                // Flex passes a *loose* major-axis bc (min=0) and Portal's
                // layout returns content size, so flex factors alone don't
                // stretch the portal. expand_height pins it to bc.max.
                sized_box(portal(unstaged_list))
                    .expand_height()
                    .flex(unstaged_factor),
                FlexSpacer::Fixed((8.0_f64).px()),
                staged_header,
                sized_box(portal(staged_list))
                    .expand_height()
                    .flex(staged_factor),
            ),
        )
        .direction(Axis::Vertical)
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .must_fill_major_axis(true)
        .gap((4.0_f64).px()),
    )
    .expand()
    .corner_radius(4.0)
    .padding(Padding::from(4.0))
    .background_color(theme.bg)
    .border(theme.border, 1.0)
}

fn right_pane(s: &CommitModalState, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let body: Box<xilem::AnyWidgetView<AppState>> = if s.selected_path.is_none() {
        sized_box(
            label("Select a file to view its diff.")
                .text_size(12.0)
                .color(theme.text_dim),
        )
        .padding(Padding::from_vh(14.0, 16.0))
        .boxed()
    } else if s.hunks.is_empty() {
        sized_box(
            label("No diff for this file.")
                .text_size(12.0)
                .color(theme.text_dim),
        )
        .padding(Padding::from_vh(14.0, 16.0))
        .boxed()
    } else {
        crate::views::diff_view::render_hunks(s.hunks.clone(), theme).boxed()
    };

    sized_box(portal(body))
        .expand()
        .corner_radius(4.0)
        .background_color(theme.bg)
        .border(theme.border, 1.0)
}

fn section_header(
    text: &'static str,
    count: usize,
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    let count_str = format!("{count}");
    flex(
        Axis::Vertical,
        (
            label(text)
                .text_size(10.0)
                .weight(xilem::FontWeight::MEDIUM)
                .color(theme.text_dim),
            FlexSpacer::Flex(1.0),
            label(count_str).text_size(10.0).color(theme.text_muted),
        ),
    )
    .direction(Axis::Horizontal)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap((6.0_f64).px())
}

fn placeholder_row(text: &'static str, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    sized_box(label(text).text_size(11.0).color(theme.text_muted))
        .padding(Padding::from_vh(4.0, 6.0))
}

fn file_row(
    f: &crate::model::diff::FileChange,
    staged: bool,
    selected: bool,
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    use crate::model::diff::FileStatus;
    let (letter, color) = match f.status {
        FileStatus::Added => ("A", theme.added),
        FileStatus::Deleted => ("D", theme.removed),
        FileStatus::Modified => ("M", theme.warn),
        FileStatus::Renamed => ("R", theme.info),
        FileStatus::Copied => ("C", theme.info),
        FileStatus::TypeChange => ("T", theme.warn),
        FileStatus::Conflicted => ("!", theme.removed),
        FileStatus::Untracked => ("?", theme.text_dim),
    };

    let path = f.path.clone();
    let path_for_action = path.clone();
    let path_for_select = path.clone();
    let path_str = path.display().to_string();

    let action_label = if staged { "−" } else { "+" };
    let action_btn = flat_button(
        crate::ui::label(action_label.to_string())
            .text_size(13.0)
            .weight(xilem::FontWeight::MEDIUM)
            .color(if staged { theme.removed } else { theme.added }),
        FlatStyle {
            idle_bg: None,
            hover_bg: theme.bg_hover,
            active_bg: None,
            radius: 4.0,
            padding_v: 0.0,
            padding_h: 6.0,
        },
        false,
        move |st: &mut AppState| {
            let path = path_for_action.clone();
            run_toggle(st, &path, staged);
        },
    );

    // Path label takes a flex slot + Clip so a long path truncates
    // inside the row's remaining width instead of pushing the action
    // button off-screen. Without this, e.g. a deeply nested file path
    // hides the [+] / [−] button and the row is unactionable.
    let path_label = label(path_str)
        .text_size(11.0)
        .color(theme.text)
        .line_break_mode(xilem::masonry::properties::LineBreaking::Clip);

    let row_inner = flex(
        Axis::Vertical,
        (
            sized_box(
                label(letter.to_string())
                    .text_size(11.0)
                    .weight(xilem::FontWeight::MEDIUM)
                    .color(color),
            )
            .width((14.0_f64).px()),
            path_label.flex(1.0),
            action_btn,
        ),
    )
    .direction(Axis::Horizontal)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap((6.0_f64).px());

    let row = sized_box(row_inner)
        .expand_width()
        .padding(Padding::from_vh(3.0, 6.0));

    clickable_box(
        row,
        ClickStyle {
            idle_bg: None,
            hover_bg: Some(theme.bg_hover),
            selected_bg: Some(theme.bg_panel_3),
            radius: 4.0,
        },
        selected,
        move |st: &mut AppState, _info| {
            let path = path_for_select.clone();
            run_select(st, &path, staged);
        },
    )
}

fn footer_view(s: &CommitModalState, theme: &Theme) -> Box<xilem::AnyWidgetView<AppState>> {
    let can_commit = !s.staged.is_empty() || s.amend;

    flex(
        Axis::Vertical,
        (
            FlexSpacer::Flex(1.0),
            flat_button(
                crate::ui::label("Cancel").text_size(12.0).color(theme.text),
                FlatStyle {
                    idle_bg: None,
                    hover_bg: theme.bg_hover,
                    active_bg: None,
                    radius: 4.0,
                    padding_v: 6.0,
                    padding_h: 14.0,
                },
                false,
                |s: &mut AppState| s.modal = None,
            ),
            flat_button(
                crate::ui::label(if s.amend { "Amend" } else { "Commit" })
                    .text_size(12.0)
                    .weight(xilem::FontWeight::MEDIUM)
                    .color(if can_commit {
                        theme.accent_fg
                    } else {
                        theme.text_muted
                    }),
                FlatStyle {
                    idle_bg: if can_commit {
                        Some(theme.accent)
                    } else {
                        Some(theme.bg_panel_3)
                    },
                    hover_bg: if can_commit {
                        theme.accent_hover
                    } else {
                        theme.bg_hover
                    },
                    active_bg: None,
                    radius: 4.0,
                    padding_v: 6.0,
                    padding_h: 14.0,
                },
                false,
                run_commit,
            ),
        ),
    )
    .direction(Axis::Horizontal)
    .gap((8.0_f64).px())
    .boxed()
}

// ── helpers ─────────────────────────────────────────────────────────────

fn is_selected(s: &CommitModalState, path: &std::path::Path, staged: bool) -> bool {
    s.selected_staged == staged && s.selected_path.as_deref() == Some(path)
}

fn run_select(st: &mut AppState, path: &std::path::Path, staged: bool) {
    let repo_path = st.repo.path.clone();
    let now = std::time::Instant::now();

    // Double-click detection: a second click on the same path within
    // 400ms also stages/unstages the file — same UX as the +/- button
    // but covers the whole row.
    let is_double = matches!(
        commit_state(st).and_then(|s| s.last_click.as_ref()),
        Some((p, t)) if p == path && now.duration_since(*t) < std::time::Duration::from_millis(400)
    );

    if let Some(cs) = commit_state_mut(st) {
        cs.selected_path = Some(path.to_path_buf());
        cs.selected_staged = staged;
        cs.reload_hunks(&repo_path);
        cs.last_click = Some((path.to_path_buf(), now));
    }

    if is_double {
        run_toggle(st, path, staged);
    }
}

fn run_toggle(st: &mut AppState, path: &std::path::Path, staged: bool) {
    let repo_path = st.repo.path.clone();
    let result = if staged {
        crate::git::ops::unstage(&repo_path, &[path])
    } else {
        crate::git::ops::stage(&repo_path, &[path])
    };
    if let Err(e) = result {
        if let Some(cs) = commit_state_mut(st) {
            cs.error = Some(format!("{e:#}"));
        }
        return;
    }
    if let Some(cs) = commit_state_mut(st) {
        // Move the selection to the same file's new side so the diff
        // pane stays anchored on the file the user just acted on.
        if cs.selected_path.as_deref() == Some(path) {
            cs.selected_staged = !staged;
        }
        cs.refresh(&repo_path);
        cs.error = None;
    }
}

fn run_commit(st: &mut AppState) {
    let (message, amend) = match commit_state(st) {
        Some(s) => (s.message.clone(), s.amend),
        None => return,
    };
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(s) = commit_state_mut(st) {
            s.error = Some("demo mode — set GITARA_REPO=<path>".into());
        }
        return;
    }

    match crate::git::ops::commit(&repo_path, &message, amend) {
        Ok(_oid) => {
            st.refresh_all();
            st.modal = None;
        }
        Err(e) => {
            if let Some(s) = commit_state_mut(st) {
                s.error = Some(format!("{e:#}"));
            }
        }
    }
}

fn commit_state(state: &AppState) -> Option<&CommitModalState> {
    match &state.modal {
        Some(Modal::Commit(s)) => Some(s),
        _ => None,
    }
}

fn commit_state_mut(state: &mut AppState) -> Option<&mut CommitModalState> {
    match &mut state.modal {
        Some(Modal::Commit(s)) => Some(s),
        _ => None,
    }
}

// Keep PathBuf in scope so the type appears in compiler messages cleanly.
const _: fn() = || {
    let _: Option<PathBuf> = None;
};
