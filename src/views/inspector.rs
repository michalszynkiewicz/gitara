//! Inspector: tabs bar + pane body. 420px default, resizable 280..=720.

use crate::app::{AppState, InspectorTab};
use crate::model::commit::Commit;
use crate::theme::Theme;
use crate::ui::label;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::masonry::properties::types::AsUnit as _;
use xilem::style::{Padding, Style as _};
use xilem::view::{flex, portal, sized_box, Axis, CrossAxisAlignment, FlexExt as _, FlexSpacer};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let body = match state.inspector.tab {
        InspectorTab::Changes => changes(state).boxed(),
        InspectorTab::Diff => diff(state).boxed(),
        InspectorTab::Files => files(state).boxed(),
        InspectorTab::Details => details(state).boxed(),
    };

    let theme = state.theme.clone();
    let current = state.inspector.tab;

    flex(
        Axis::Vertical,
        (
            // Tab strip
            sized_box(
                flex(
                    Axis::Vertical,
                    (
                        itab(
                            &theme,
                            "Changes",
                            current == InspectorTab::Changes,
                            |s: &mut AppState| s.inspector.tab = InspectorTab::Changes,
                        ),
                        itab(
                            &theme,
                            "Diff",
                            current == InspectorTab::Diff,
                            |s: &mut AppState| s.inspector.tab = InspectorTab::Diff,
                        ),
                        itab(
                            &theme,
                            "Files",
                            current == InspectorTab::Files,
                            |s: &mut AppState| s.inspector.tab = InspectorTab::Files,
                        ),
                        itab(
                            &theme,
                            "Details",
                            current == InspectorTab::Details,
                            |s: &mut AppState| s.inspector.tab = InspectorTab::Details,
                        ),
                        FlexSpacer::Flex(1.0),
                    ),
                )
                .direction(Axis::Horizontal)
                .gap((2.0_f64).px()),
            )
            .expand_width()
            .padding(Padding::from_vh(6.0, 10.0)),
            // Wrap the body in a Portal so long diffs/file lists scroll within
            // the inspector pane instead of overflowing it.
            portal(body).flex(1.0),
        ),
    )
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap((0.0_f64).px())
}

fn itab<F>(
    theme: &Theme,
    text: &'static str,
    selected: bool,
    cb: F,
) -> impl xilem::WidgetView<AppState>
where
    F: Fn(&mut AppState) + Send + Sync + 'static,
{
    let fg = if selected {
        theme.text
    } else {
        theme.text_muted
    };
    flat_button(
        crate::ui::label(text).text_size(12.0).color(fg),
        FlatStyle {
            idle_bg: None,
            hover_bg: theme.bg_hover,
            active_bg: Some(theme.bg_panel_3),
            radius: 4.0,
            padding_v: 4.0,
            padding_h: 10.0,
        },
        selected,
        move |s: &mut AppState| cb(s),
    )
}

fn selected_commit(state: &AppState) -> Option<&Commit> {
    let primary = state
        .selection
        .primary
        .as_ref()
        .or(state.commits.first().map(|c| &c.oid))?;
    if primary == crate::views::graph::WORKING_TREE_OID {
        return None;
    }
    state.commits.iter().find(|c| &c.oid == primary)
}

/// True when the working-tree pseudo-row is the active selection.
fn working_tree_selected(state: &AppState) -> bool {
    state.selection.primary.as_deref() == Some(crate::views::graph::WORKING_TREE_OID)
}

fn details(state: &AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let Some(c) = selected_commit(state) else {
        return placeholder("No commit selected", &theme).boxed();
    };

    let parents = if c.parents.is_empty() {
        "(root)".to_string()
    } else {
        c.parents
            .iter()
            .map(|p| &p[..p.len().min(7)])
            .collect::<Vec<_>>()
            .join("   ")
    };

    let tags: Vec<String> = c
        .refs
        .iter()
        .filter_map(|r| match r {
            crate::model::commit::RefChip::Tag { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    let tags_row: Box<xilem::AnyWidgetView<AppState>> = if tags.is_empty() {
        label("").boxed()
    } else {
        kv("tags", tags.join("   "), &theme).boxed()
    };

    let (sig_text, sig_color) = if c.signed {
        ("✓ signed", theme.added)
    } else {
        ("not signed", theme.text_muted)
    };

    sized_box(
        flex(
            Axis::Vertical,
            (
                // Subject first — it's the headline.
                label(c.subject.clone())
                    .text_size(14.0)
                    .weight(xilem::FontWeight::MEDIUM)
                    .color(theme.text),
                FlexSpacer::Fixed((6.0_f64).px()),
                // Author byline — emails muted, name regular.
                label(format!("{} · {}", c.author.name, c.author.email))
                    .text_size(11.0)
                    .color(theme.text_muted),
                FlexSpacer::Fixed((14.0_f64).px()),
                divider(&theme),
                FlexSpacer::Fixed((12.0_f64).px()),
                kv("commit", c.oid.clone(), &theme),
                kv("parents", parents, &theme),
                tags_row,
                kv_brushed("signed", sig_text.to_string(), sig_color, &theme),
                FlexSpacer::Fixed((14.0_f64).px()),
                // Body, if any. WordWrap so long lines wrap inside the
                // inspector pane and \n in the message renders as a hard
                // break (full multi-line commit message is visible, not
                // just the first line).
                label(c.body.clone().unwrap_or_default())
                    .text_size(12.0)
                    .color(theme.text_muted)
                    .line_break_mode(xilem::masonry::properties::LineBreaking::WordWrap),
            ),
        )
        .direction(Axis::Vertical)
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap((0.0_f64).px()),
    )
    .expand_width()
    .padding(Padding::from_vh(14.0, 16.0))
    .boxed()
}

fn kv_brushed(
    key: &'static str,
    value: String,
    value_brush: vello::peniko::Color,
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    flex(
        Axis::Vertical,
        (
            sized_box(
                label(key)
                    .text_size(10.0)
                    .weight(xilem::FontWeight::MEDIUM)
                    .color(theme.text_dim),
            )
            .width((60.0_f64).px()),
            label(value).text_size(12.0).color(value_brush),
        ),
    )
    .direction(Axis::Horizontal)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap((8.0_f64).px())
}

fn kv(key: &'static str, value: String, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    flex(
        Axis::Vertical,
        (
            sized_box(
                label(key)
                    .text_size(10.0)
                    .weight(xilem::FontWeight::MEDIUM)
                    .color(theme.text_dim),
            )
            .width((60.0_f64).px()),
            label(value).text_size(12.0).color(theme.text),
        ),
    )
    .direction(Axis::Horizontal)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap((8.0_f64).px())
}

fn divider(theme: &Theme) -> impl xilem::WidgetView<AppState> {
    sized_box(flex(Axis::Vertical, ()))
        .expand_width()
        .height((1.0_f64).px())
        .background_color(theme.border_faint)
}

fn placeholder(text: &'static str, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    sized_box(label(text).text_size(12.0).color(theme.text_dim))
        .expand()
        .padding(Padding::from_vh(14.0, 16.0))
}

// ── Changes tab ────────────────────────────────────────────────────────
//
// Context-aware: working-tree diff vs HEAD when the working-tree row is
// selected, else the file list for the selected commit. Mirrors what
// `git status` / `git show --stat` would show for the chosen entity.

fn changes(state: &AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    if crate::app::is_demo_repo(&state.repo.path) {
        return placeholder("demo mode — changes are only shown for real repos", &theme).boxed();
    }
    if working_tree_selected(state) || state.selection.primary.is_none() {
        return match crate::git::diff::dirty_tree(&state.repo.path) {
            Ok(files) if files.is_empty() => placeholder("No uncommitted changes.", &theme).boxed(),
            Ok(files) => files_list(&files, &theme).boxed(),
            Err(e) => placeholder_text(&format!("error: {e:#}"), &theme).boxed(),
        };
    }
    let Some(c) = selected_commit(state) else {
        return placeholder("Select a commit or the working-tree row.", &theme).boxed();
    };
    match crate::git::diff::files_for_commit(&state.repo.path, &c.oid) {
        Ok(files) if files.is_empty() => placeholder("No files changed.", &theme).boxed(),
        Ok(files) => files_list(&files, &theme).boxed(),
        Err(e) => placeholder_text(&format!("error: {e:#}"), &theme).boxed(),
    }
}

// ── Files tab: changed-files list for the selected commit ──────────────

fn files(state: &AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    if working_tree_selected(state) {
        return placeholder("Working-tree files — see the Changes tab.", &theme).boxed();
    }
    let Some(c) = selected_commit(state) else {
        return placeholder("Select a commit to see its files.", &theme).boxed();
    };
    if crate::app::is_demo_repo(&state.repo.path) {
        return placeholder("demo mode — files are only shown for real repos", &theme).boxed();
    }
    match crate::git::diff::files_for_commit(&state.repo.path, &c.oid) {
        Ok(files) if files.is_empty() => placeholder("No files changed.", &theme).boxed(),
        Ok(files) => files_list(&files, &theme).boxed(),
        Err(e) => placeholder_text(&format!("error: {e:#}"), &theme).boxed(),
    }
}

fn files_list(
    files: &[crate::model::diff::FileChange],
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    let rows: Vec<_> = files.iter().map(|f| file_row(f, theme).boxed()).collect();
    sized_box(
        flex(Axis::Vertical, rows)
            .direction(Axis::Vertical)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .gap((2.0_f64).px()),
    )
    .expand_width()
    .padding(Padding::from_vh(10.0, 14.0))
}

fn file_row(f: &crate::model::diff::FileChange, theme: &Theme) -> impl xilem::WidgetView<AppState> {
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

    let path_str = f.path.display().to_string();
    let counts = if f.additions > 0 || f.deletions > 0 {
        format!("+{} −{}", f.additions, f.deletions)
    } else {
        String::new()
    };

    flex(
        Axis::Vertical,
        (
            sized_box(
                label(letter.to_string())
                    .text_size(11.0)
                    .weight(xilem::FontWeight::MEDIUM)
                    .color(color),
            )
            .width((16.0_f64).px()),
            label(path_str).text_size(12.0).color(theme.text),
            FlexSpacer::Flex(1.0),
            label(counts).text_size(11.0).color(theme.text_muted),
        ),
    )
    .direction(Axis::Horizontal)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .gap((8.0_f64).px())
}

// ── Diff tab: hunks for the selected commit ────────────────────────────

fn diff(state: &AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    if crate::app::is_demo_repo(&state.repo.path) {
        return placeholder("demo mode — diffs are only shown for real repos", &theme).boxed();
    }
    if working_tree_selected(state) {
        return match crate::git::diff::hunks_for_dirty_tree(&state.repo.path) {
            Ok(hunks) if hunks.is_empty() => placeholder("No uncommitted changes.", &theme).boxed(),
            Ok(hunks) => crate::views::diff_view::render_hunks(hunks, &theme).boxed(),
            Err(e) => placeholder_text(&format!("error: {e:#}"), &theme).boxed(),
        };
    }
    let Some(c) = selected_commit(state) else {
        return placeholder("Select a commit to see its diff.", &theme).boxed();
    };
    match crate::git::diff::hunks_for_commit(&state.repo.path, &c.oid, None) {
        Ok(hunks) if hunks.is_empty() => placeholder("No changes in this commit.", &theme).boxed(),
        Ok(hunks) => crate::views::diff_view::render_hunks(hunks, &theme).boxed(),
        Err(e) => placeholder_text(&format!("error: {e:#}"), &theme).boxed(),
    }
}

fn placeholder_text(text: &str, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    sized_box(
        label(text.to_string())
            .text_size(12.0)
            .color(theme.text_dim),
    )
    .expand()
    .padding(Padding::from_vh(14.0, 16.0))
}
