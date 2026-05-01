//! Graph pane: view-tabs (History / Reflog) + virtualized table.

use crate::app::{AppState, CtxMenu, CtxMenuKind, View};
use crate::graph_layout::{self, RowLayout};
use crate::model::commit::Commit;
use crate::model::reflog::{ReflogAction, ReflogEntry};
use crate::theme::Theme;
use crate::ui::label;
use crate::widgets::clickable_box::{clickable_box, ClickInfo, ClickStyle};
use crate::widgets::flat_button::{flat_button, FlatStyle};
use crate::widgets::flow::flow;
use crate::widgets::graph_gutter::{graph_gutter as graph_gutter_view, GutterStyle};
use masonry::core::PointerButton;
use xilem::masonry::properties::types::AsUnit as _;
use xilem::style::{Padding, Style as _};
use xilem::view::{flex, portal, sized_box, Axis, FlexExt as _, FlexSpacer};
use xilem::WidgetView as _;

// Graph geometry. Pitch + row height stay here because the row's
// non-graph columns (oid, subject, …) need to know how wide the gutter
// is and how tall the row should sit. LANE_W / NODE_D live in the
// custom GraphGutter widget where painting happens.
const LANE_PITCH: f64 = 14.0; // horizontal spacing between lanes
const ROW_H: f64 = 24.0; // row height

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let body = match state.view {
        View::History => history_table(state).boxed(),
        View::Reflog => reflog_table(state).boxed(),
    };

    let theme = state.theme.clone();
    let is_history = matches!(state.view, View::History);

    let all_refs = state.show_all_refs;
    let wrap = state.wrap_subjects;

    flex(
        Axis::Vertical,
        (
            sized_box(
                flex(
                    Axis::Vertical,
                    (
                        tab(&theme, "History", is_history, |s: &mut AppState| {
                            s.view = View::History
                        }),
                        tab(&theme, "Reflog", !is_history, |s: &mut AppState| {
                            s.view = View::Reflog
                        }),
                        FlexSpacer::Flex(1.0),
                        wrap_toggle(&theme, wrap),
                        all_refs_toggle(&theme, all_refs),
                    ),
                )
                .direction(Axis::Horizontal)
                .gap((2.0_f64).px()),
            )
            .expand_width()
            .padding(Padding::from_vh(6.0, 10.0)),
            // Portal wraps the History/Reflog body so it scrolls within the
            // panel instead of overflowing. .flex(1.0) makes it consume the
            // remaining vertical space below the tab strip.
            portal(body).flex(1.0),
        ),
    )
    .direction(Axis::Vertical)
    .gap((0.0_f64).px())
}

/// Pill that toggles per-row subject wrapping. Off = clip at column;
/// on = wrap, row grows vertically.
fn wrap_toggle(theme: &Theme, on: bool) -> impl xilem::WidgetView<AppState> {
    let label_text = if on { "✓ wrap" } else { "wrap" };
    flat_button(
        crate::ui::label(label_text).text_size(11.0).color(if on {
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
        on,
        |s: &mut AppState| {
            s.wrap_subjects = !s.wrap_subjects;
        },
    )
}

/// Pill that toggles show-all-branches (gitk --all). Reloads commits on flip.
fn all_refs_toggle(theme: &Theme, on: bool) -> impl xilem::WidgetView<AppState> {
    let label_text = if on {
        "✓ all branches"
    } else {
        "all branches"
    };
    flat_button(
        crate::ui::label(label_text).text_size(11.0).color(if on {
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
        on,
        |s: &mut AppState| {
            s.show_all_refs = !s.show_all_refs;
            s.reload_commits();
        },
    )
}

fn tab<F>(
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

fn history_table(state: &AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let layouts = graph_layout::compute(&state.commits);
    let lane_count = layouts.first().map(|r| r.max_column + 1).unwrap_or(1) as usize;
    let selected = state.selection.primary.clone();
    let wrap = state.wrap_subjects;

    let mut rows: Vec<Box<xilem::AnyWidgetView<AppState>>> = Vec::new();

    // Working-tree pseudo-row on top when dirty.
    if let Some(status) = state.working_status.as_ref() {
        if status.is_dirty() {
            let wt_selected = selected.as_deref() == Some(WORKING_TREE_OID);
            rows.push(working_tree_row(status, lane_count, &theme, wt_selected).boxed());
        }
    }

    for (c, rl) in state.commits.iter().zip(layouts.iter()) {
        rows.push(commit_row(c, rl, lane_count, &theme, selected.as_deref(), wrap).boxed());
    }

    sized_box(flex(Axis::Vertical, rows).gap((0.0_f64).px()))
        .expand_width()
        .padding(Padding::from_vh(0.0, 8.0))
}

/// A pseudo-row showing the current working-tree dirty state. Rendered
/// above the topmost commit. Clicking switches the inspector to Changes.
fn working_tree_row(
    status: &crate::git::status::WorkingStatus,
    lane_count: usize,
    theme: &Theme,
    is_selected: bool,
) -> impl xilem::WidgetView<AppState> {
    use xilem::view::{flex, Axis, CrossAxisAlignment};

    let gutter = sized_box(flex(Axis::Vertical, ()))
        .width((lane_count as f64 * LANE_PITCH).px())
        .height((ROW_H).px());
    let summary = status.summary();

    let row_inner = sized_box(
        flex(
            Axis::Vertical,
            (
                gutter,
                sized_box(
                    label("●  working tree")
                        .text_size(12.0)
                        .weight(xilem::FontWeight::MEDIUM)
                        .color(theme.warn),
                )
                .width((160.0_f64).px()),
                label(summary).text_size(12.0).color(theme.text),
                FlexSpacer::Flex(1.0),
            ),
        )
        .direction(Axis::Horizontal)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap((8.0_f64).px()),
    )
    .expand_width()
    .padding(Padding::from_vh(4.0, 6.0));

    // Move the green "dirty" tint up to ClickableBox so selected_bg can
    // override it without being painted under the inner sized_box.
    clickable_box(
        row_inner,
        ClickStyle {
            idle_bg: Some(theme.added_bg),
            hover_bg: Some(theme.bg_hover),
            selected_bg: Some(theme.bg_selected),
            radius: 0.0,
        },
        is_selected,
        |st: &mut AppState, _info: ClickInfo| {
            st.selection.primary = Some(WORKING_TREE_OID.into());
            st.selection.set = vec![WORKING_TREE_OID.into()];
            st.selection.anchor = Some(WORKING_TREE_OID.into());
            st.inspector.tab = crate::app::InspectorTab::Changes;
            st.ctx_menu = None;
        },
    )
}

/// Sentinel "oid" used by `selection.primary` when the working-tree
/// pseudo-row is selected.
pub const WORKING_TREE_OID: &str = "__working_tree__";

fn commit_row(
    c: &Commit,
    row: &RowLayout,
    lane_count: usize,
    theme: &Theme,
    selected_oid: Option<&str>,
    wrap: bool,
) -> impl xilem::WidgetView<AppState> {
    use xilem::masonry::properties::LineBreaking;
    use xilem::view::{flex, Axis, CrossAxisAlignment, FlexExt as _};

    let oid_for_chip_select = c.oid.clone();
    let chip_views: Vec<_> = c
        .refs
        .iter()
        .map(|r| ref_chip(r, theme, oid_for_chip_select.clone()).boxed())
        .collect();
    let is_selected = selected_oid == Some(c.oid.as_str());
    let subject_color = if is_selected {
        theme.accent
    } else {
        theme.text
    };

    // CrossAxisAlignment::Fill stretches every child to the row's full
    // height. The graph_gutter widget reads ctx.size().height in paint, so
    // its lane lines extend cleanly across whatever the row ends up being
    // tall (driven by the wrapping subject).
    let row_inner = sized_box(
        flex(
            Axis::Vertical,
            (
                graph_gutter(row, lane_count, theme),
                // Fixed-width chips column so subjects/oid/author align
                // across rows. Inside, chips use a flow/wrap layout so a
                // long branch name pushes subsequent chips onto a new line
                // (growing the row taller) instead of overflowing the
                // column and shoving other columns sideways.
                sized_box(flow(chip_views, 4.0, 2.0)).width((180.0_f64).px()),
                // Same flow trick as the author column: enforce a 74px
                // column even if the short oid renders narrower naturally,
                // so subject's flex(1.0) gets a stable allocation.
                sized_box(flow(
                    vec![label(c.short.clone())
                        .text_size(12.0)
                        .color(theme.text_dim)
                        .boxed()],
                    0.0,
                    0.0,
                ))
                .width((74.0_f64).px())
                .padding(Padding::from_vh(0.0, 6.0)),
                // Subject takes the remaining horizontal space via flex(1.0).
                // Wrapped in flow because flex hands us a loose bc (max =
                // allocated slot) — and label, like sized_box, returns its
                // natural width, which would let the next column (author)
                // slide leftward as the window widens. flow expands to bc
                // max when finite, pinning the author column to the right.
                flow(
                    vec![label(c.subject.clone())
                        .text_size(13.0)
                        .color(subject_color)
                        .line_break_mode(if wrap {
                            LineBreaking::WordWrap
                        } else {
                            LineBreaking::Clip
                        })
                        .boxed()],
                    0.0,
                    0.0,
                )
                .flex(1.0),
                // Author column at the right end. Wrap the label in `flow`
                // which (unlike sized_box) respects the bc.min width — so
                // a short author name still leaves the column at its full
                // 200px rather than collapsing to the natural text width
                // and letting the subject's flex steal that space, which
                // would shift the author's left edge across rows.
                sized_box(flow(
                    vec![label(if c.author.email.is_empty() {
                        c.author.name.clone()
                    } else {
                        format!("{}  <{}>", c.author.name, c.author.email)
                    })
                    .text_size(12.0)
                    .color(theme.text_muted)
                    .line_break_mode(LineBreaking::Clip)
                    .boxed()],
                    0.0,
                    0.0,
                ))
                .width((200.0_f64).px()),
            ),
        )
        .direction(Axis::Horizontal)
        .cross_axis_alignment(CrossAxisAlignment::Fill)
        .gap((8.0_f64).px()),
    )
    .expand_width()
    .padding(Padding::from_vh(2.0, 6.0));

    let oid_for_click = c.oid.clone();
    clickable_box(
        row_inner,
        ClickStyle {
            idle_bg: None,
            hover_bg: Some(theme.bg_hover),
            selected_bg: Some(theme.bg_selected),
            radius: 0.0,
        },
        is_selected,
        move |st: &mut AppState, info: ClickInfo| {
            // Always select on click — gives consistent feedback regardless of button.
            st.selection.primary = Some(oid_for_click.clone());
            st.selection.set = vec![oid_for_click.clone()];
            st.selection.anchor = Some(oid_for_click.clone());
            // Right click also opens the context menu at the pointer position.
            if matches!(info.button, Some(PointerButton::Secondary)) {
                st.ctx_menu = Some(CtxMenu {
                    x: info.x,
                    y: info.y,
                    kind: CtxMenuKind::Commit {
                        oid: oid_for_click.clone(),
                    },
                });
            } else {
                // Any other click closes a stale menu.
                st.ctx_menu = None;
            }
        },
    )
}

/// A single ref chip — a small pill with per-type coloring. Branch
/// chips are wrapped in a ClickableBox so the user can right-click to
/// open a branch-specific context menu (checkout, delete, …); a left
/// click selects the host commit just like clicking the row.
fn ref_chip(
    chip: &crate::model::commit::RefChip,
    theme: &Theme,
    commit_oid: String,
) -> Box<xilem::AnyWidgetView<AppState>> {
    use crate::model::commit::RefChip as R;
    let (text, fg, bg_tint, border) = match chip {
        R::Head => (
            "HEAD".to_string(),
            theme.accent_fg,
            theme.accent,
            theme.accent_hover,
        ),
        R::Branch { name, current } => {
            if *current {
                (
                    name.clone(),
                    theme.accent_fg,
                    theme.accent,
                    theme.accent_hover,
                )
            } else {
                (name.clone(), theme.text, theme.accent_tint, theme.border)
            }
        }
        R::Remote { name } => (
            name.clone(),
            theme.text_muted,
            theme.bg_panel_3,
            theme.border,
        ),
        R::Tag { name, .. } => (
            format!("tag: {}", name),
            theme.warn,
            blend(theme.warn, theme.bg_panel, 0.85),
            theme.border,
        ),
    };
    // LineBreaking::Clip caps the pill's intrinsic width at the bc max
    // (= chip column width when used in the row). A long branch name
    // therefore truncates inside its pill instead of pushing other
    // chips/columns off-screen.
    let pill = sized_box(
        label(text)
            .text_size(11.0)
            .color(fg)
            .line_break_mode(xilem::masonry::properties::LineBreaking::Clip),
    )
    .corner_radius(4.0)
    .background_color(bg_tint)
    .border(border, 1.0)
    .padding(Padding::from_vh(1.0, 6.0));

    // Branches and tags each get their own click target so right-click
    // opens a kind-specific context menu (tag delete vs branch delete).
    // HEAD/remote chips fall through to the row's outer ClickableBox.
    match chip {
        R::Branch { name, .. } => {
            let branch_name = name.clone();
            let oid_for_select = commit_oid.clone();
            clickable_box(
                pill,
                ClickStyle::default(),
                false,
                move |st: &mut AppState, info: ClickInfo| {
                    st.selection.primary = Some(oid_for_select.clone());
                    st.selection.set = vec![oid_for_select.clone()];
                    st.selection.anchor = Some(oid_for_select.clone());
                    if matches!(info.button, Some(PointerButton::Secondary)) {
                        st.ctx_menu = Some(crate::app::CtxMenu {
                            x: info.x,
                            y: info.y,
                            kind: CtxMenuKind::Branch {
                                name: branch_name.clone(),
                            },
                        });
                    } else {
                        st.ctx_menu = None;
                    }
                },
            )
            .boxed()
        }
        R::Tag { name, .. } => {
            let tag_name = name.clone();
            let oid_for_select = commit_oid.clone();
            clickable_box(
                pill,
                ClickStyle::default(),
                false,
                move |st: &mut AppState, info: ClickInfo| {
                    st.selection.primary = Some(oid_for_select.clone());
                    st.selection.set = vec![oid_for_select.clone()];
                    st.selection.anchor = Some(oid_for_select.clone());
                    if matches!(info.button, Some(PointerButton::Secondary)) {
                        st.ctx_menu = Some(crate::app::CtxMenu {
                            x: info.x,
                            y: info.y,
                            kind: CtxMenuKind::Tag {
                                name: tag_name.clone(),
                            },
                        });
                    } else {
                        st.ctx_menu = None;
                    }
                },
            )
            .boxed()
        }
        _ => pill.boxed(),
    }
}

/// Blend two colors: returns `(1-t)*a + t*b`, so t=0 returns a, t=1 returns b.
fn blend(a: vello::peniko::Color, b: vello::peniko::Color, t: f32) -> vello::peniko::Color {
    let [ar, ag, ab, _] = a.components;
    let [br, bg, bb, _] = b.components;
    vello::peniko::Color::new([
        ar * (1.0 - t) + br * t,
        ag * (1.0 - t) + bg * t,
        ab * (1.0 - t) + bb * t,
        1.0,
    ])
}

/// Render the row gutter via a custom widget that paints lane lines
/// and diagonal branch / merge connectors directly to the Vello scene.
fn graph_gutter(
    row: &RowLayout,
    lane_count: usize,
    theme: &Theme,
) -> impl xilem::WidgetView<AppState> {
    graph_gutter_view::<AppState, ()>(GutterStyle {
        row: row.clone(),
        lane_count: lane_count as u8,
        lanes: theme.lanes.to_vec(),
        bg: theme.bg_panel,
    })
}

fn reflog_table(state: &AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let rows: Vec<_> = state.reflog.iter().map(|e| reflog_row(e, &theme)).collect();
    sized_box(flex(Axis::Vertical, rows).gap((0.0_f64).px()))
        .expand_width()
        .padding(Padding::from_vh(0.0, 8.0))
}

fn reflog_row(e: &ReflogEntry, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    use xilem::view::{flex, Axis};
    let action = match &e.action {
        ReflogAction::Commit => "commit",
        ReflogAction::Merge => "merge",
        ReflogAction::Checkout => "checkout",
        ReflogAction::Rebase => "rebase",
        ReflogAction::Reset => "reset",
        ReflogAction::Pull => "pull",
        ReflogAction::Push => "push",
        ReflogAction::Clone => "clone",
        ReflogAction::CherryPick => "cherry-pick",
        ReflogAction::Amend => "amend",
        ReflogAction::Other => "other",
    };
    sized_box(
        flex(
            Axis::Vertical,
            (
                sized_box(label(e.short.clone()).text_size(12.0).color(theme.text_dim))
                    .width((80.0_f64).px()),
                sized_box(label(action.to_string()).text_size(12.0).color(theme.info))
                    .width((100.0_f64).px()),
                label(e.subject.clone()).text_size(13.0).color(theme.text),
                FlexSpacer::Flex(1.0),
            ),
        )
        .direction(Axis::Horizontal)
        .gap((8.0_f64).px()),
    )
    .expand_width()
    .padding(Padding::from_vh(4.0, 6.0))
}
