//! Sidebar: Branches / Remotes / Tags / Stashes. 220px default.
//! Clicking a branch performs a checkout (if tree is clean); right-click
//! opens a branch context menu (Checkout / Delete / …).

use crate::app::{AppState, CtxMenu, CtxMenuKind};
use crate::theme::Theme;
use crate::widgets::clickable_box::{clickable_box, ClickInfo, ClickStyle};
use masonry::core::PointerButton;
use xilem::view::{
    flex, label, sized_box, Axis, CrossAxisAlignment, FlexSpacer, Label, MainAxisAlignment, Padding,
};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();

    flex((
        section_header("BRANCHES", &theme),
        flex(branch_rows(state, &theme))
            .direction(Axis::Vertical)
            .gap(0.0),
        FlexSpacer::Fixed(12.0),
        section_header("REMOTES", &theme),
        flex(remote_rows(state)).direction(Axis::Vertical).gap(2.0),
        FlexSpacer::Fixed(12.0),
        section_header("TAGS", &theme),
        flex(tag_rows(state)).direction(Axis::Vertical).gap(2.0),
        FlexSpacer::Fixed(12.0),
        section_header("STASHES", &theme),
        flex(stash_rows(state)).direction(Axis::Vertical).gap(2.0),
    ))
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .main_axis_alignment(MainAxisAlignment::Start)
    .gap(4.0)
}

fn section_header(title: &'static str, theme: &Theme) -> Label {
    label(title)
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM)
}

fn branch_rows(state: &AppState, theme: &Theme) -> Vec<Box<xilem::AnyWidgetView<AppState>>> {
    state
        .repo
        .branches
        .iter()
        .map(|b| {
            let name = b.name.clone();
            let current = b.current;
            let ahead_behind = if b.ahead > 0 || b.behind > 0 {
                format!("  ↑{} ↓{}", b.ahead, b.behind)
            } else {
                String::new()
            };
            let prefix = if current { "●  " } else { "   " };
            let text = format!("{prefix}{name}{ahead_behind}");
            let brush = if current { theme.accent } else { theme.text };
            let mut lbl = label(text).brush(brush).text_size(12.0);
            if current {
                lbl = lbl.weight(xilem::FontWeight::MEDIUM);
            }
            let row_inner = sized_box(lbl)
                .expand_width()
                .padding(Padding::from_vh(3.0, 4.0));

            let name_for_cb = name.clone();
            clickable_box(
                row_inner,
                ClickStyle {
                    idle_bg: None,
                    hover_bg: Some(theme.bg_hover),
                    selected_bg: None,
                    radius: 3.0,
                },
                false,
                move |st: &mut AppState, info: ClickInfo| {
                    if matches!(info.button, Some(PointerButton::Secondary)) {
                        st.ctx_menu = Some(CtxMenu {
                            x: info.x,
                            y: info.y,
                            kind: CtxMenuKind::Branch {
                                name: name_for_cb.clone(),
                            },
                        });
                    } else {
                        // Primary (or any non-Secondary) click → checkout.
                        st.ctx_menu = None;
                        checkout_branch(st, &name_for_cb);
                    }
                },
            )
            .boxed()
        })
        .collect()
}

fn checkout_branch(st: &mut AppState, name: &str) {
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        tracing::warn!("checkout {name}: demo mode, ignoring");
        return;
    }
    // Don't check out a branch that's already current.
    if st.repo.branches.iter().any(|b| b.current && b.name == name) {
        return;
    }

    match crate::git::ops::checkout(&repo_path, name) {
        Ok(()) => {
            st.refresh_all();
            // Clear the per-commit selection — indexes may be different now.
            st.selection = Default::default();
        }
        Err(e) => {
            st.toast = Some(crate::app::Toast::error(format!("checkout {name}: {e:#}")));
        }
    }
}

fn remote_rows(state: &AppState) -> Vec<Box<xilem::AnyWidgetView<AppState>>> {
    let theme = state.theme.clone();
    state
        .repo
        .remotes
        .iter()
        .map(|r| {
            let name = r.name.clone();
            let lbl = label(format!("   {}", r.name))
                .brush(theme.text)
                .text_size(12.0);
            clickable_box_for_ctx_menu(
                lbl,
                &theme,
                move |_info: ClickInfo| CtxMenuKind::Remote { name: name.clone() },
                None::<fn(&mut AppState)>,
            )
        })
        .collect()
}

fn tag_rows(state: &AppState) -> Vec<Box<xilem::AnyWidgetView<AppState>>> {
    let theme = state.theme.clone();
    state
        .repo
        .tags
        .iter()
        .map(|t| {
            let name = t.name.clone();
            let lbl = label(format!("   {}", t.name))
                .brush(theme.text)
                .text_size(12.0);
            clickable_box_for_ctx_menu(
                lbl,
                &theme,
                move |_info: ClickInfo| CtxMenuKind::Tag { name: name.clone() },
                None::<fn(&mut AppState)>,
            )
        })
        .collect()
}

fn stash_rows(state: &AppState) -> Vec<Box<xilem::AnyWidgetView<AppState>>> {
    let theme = state.theme.clone();
    state
        .repo
        .stashes
        .iter()
        .map(|s| {
            let idx = s.idx;
            let lbl = label(format!("   stash@{{{}}} · {}", s.idx, s.message))
                .brush(theme.text_muted)
                .text_size(12.0);
            clickable_box_for_ctx_menu(
                lbl,
                &theme,
                move |_info: ClickInfo| CtxMenuKind::Stash { idx },
                None::<fn(&mut AppState)>,
            )
        })
        .collect()
}

/// Helper: wrap `inner` in a ClickableBox where right-click opens a
/// ctx_menu of the given kind and left-click optionally runs `on_left`.
/// Returns a boxed AnyWidgetView so the caller can collect heterogeneous
/// rows into a single Vec.
fn clickable_box_for_ctx_menu<V, FKind, FLeft>(
    inner: V,
    theme: &Theme,
    kind_fn: FKind,
    on_left: Option<FLeft>,
) -> Box<xilem::AnyWidgetView<AppState>>
where
    V: xilem::WidgetView<AppState> + 'static,
    FKind: Fn(ClickInfo) -> CtxMenuKind + Send + Sync + 'static,
    FLeft: Fn(&mut AppState) + Send + Sync + 'static,
{
    let row_inner = sized_box(inner)
        .expand_width()
        .padding(Padding::from_vh(3.0, 4.0));
    clickable_box(
        row_inner,
        ClickStyle {
            idle_bg: None,
            hover_bg: Some(theme.bg_hover),
            selected_bg: None,
            radius: 3.0,
        },
        false,
        move |st: &mut AppState, info: ClickInfo| {
            if matches!(info.button, Some(PointerButton::Secondary)) {
                let kind = kind_fn(info.clone());
                st.ctx_menu = Some(CtxMenu {
                    x: info.x,
                    y: info.y,
                    kind,
                });
            } else {
                st.ctx_menu = None;
                if let Some(f) = on_left.as_ref() {
                    f(st);
                }
            }
        },
    )
    .boxed()
}
