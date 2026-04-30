//! Context menu — floating overlay anchored at (x, y) with the items
//! appropriate to the right-clicked entity. Clicking anywhere outside the
//! menu's card dismisses it.

use crate::app::{
    AppState, BranchModalState, CherryPickModalState, CtxMenu, CtxMenuKind, Modal,
    RenameBranchModalState, ResetModalState, Toast,
};
use crate::theme::Theme;
use crate::widgets::clickable_box::{clickable_box, ClickInfo, ClickStyle};
use crate::widgets::flat_button::{flat_button, FlatStyle};
use vello::peniko::Color;
use xilem::masonry::properties::types::AsUnit as _;
use xilem::masonry::properties::types::UnitPoint;
use xilem::style::{Padding, Style as _};
use xilem::view::{flex, label, sized_box, zstack, Axis, CrossAxisAlignment, FlexSpacer};
use xilem::WidgetView as _;

const MENU_WIDTH: f64 = 200.0;

pub fn view(state: &AppState) -> Option<impl xilem::WidgetView<AppState>> {
    let menu = state.ctx_menu.clone()?;
    let theme = state.theme.clone();

    let menu_card = build_menu_card(&menu, &theme);

    // Position the card at (x, y) using nested flex spacers. Wrap the whole
    // thing in a backdrop ClickableBox so any click outside the card (the
    // spacer regions) closes the menu. Inner FlatButtons / ClickableBoxes
    // call set_handled() on Down, so item clicks don't reach the backdrop.
    let positioned = sized_box(
        flex(
            Axis::Vertical,
            (
                FlexSpacer::Fixed((menu.y).px()),
                sized_box(
                    flex(
                        Axis::Vertical,
                        (FlexSpacer::Fixed((menu.x).px()), menu_card),
                    )
                    .direction(Axis::Horizontal)
                    .cross_axis_alignment(CrossAxisAlignment::Start),
                ),
            ),
        )
        .direction(Axis::Vertical)
        .cross_axis_alignment(CrossAxisAlignment::Start),
    )
    .expand();

    let dismiss_layer = clickable_box(
        positioned,
        ClickStyle {
            idle_bg: Some(Color::TRANSPARENT),
            hover_bg: None,
            selected_bg: None,
            radius: 0.0,
        },
        false,
        |s: &mut AppState, _info: ClickInfo| {
            s.ctx_menu = None;
        },
    );

    Some(zstack((dismiss_layer,)).alignment(UnitPoint::TOP_LEFT))
}

fn build_menu_card(menu: &CtxMenu, theme: &Theme) -> Box<xilem::AnyWidgetView<AppState>> {
    let items: Box<xilem::AnyWidgetView<AppState>> = match &menu.kind {
        CtxMenuKind::Commit { oid } => commit_items(oid.clone(), theme).boxed(),
        CtxMenuKind::Branch { name } => branch_items(name.clone(), theme).boxed(),
        CtxMenuKind::Remote { name } => remote_items(name.clone(), theme).boxed(),
        CtxMenuKind::Tag { name } => tag_items(name.clone(), theme).boxed(),
        CtxMenuKind::Stash { idx } => stash_items(*idx, theme).boxed(),
    };

    sized_box(items)
        .width((MENU_WIDTH).px())
        .corner_radius(6.0)
        .background_color(theme.bg_panel)
        .border(theme.border, 1.0)
        .padding(Padding::from_vh(4.0, 0.0))
        .boxed()
}

fn commit_items(oid: String, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let oid_for_branch = oid.clone();
    let oid_for_tag = oid.clone();
    let oid_for_reset = oid.clone();
    let oid_for_pick = oid.clone();
    let short = oid.chars().take(7).collect::<String>();

    flex(
        Axis::Vertical,
        (
            menu_label(&format!("commit  {short}"), theme),
            menu_separator(theme),
            menu_item("Create branch from here", theme, move |s: &mut AppState| {
                s.modal = Some(Modal::Branch(BranchModalState {
                    name: String::new(),
                    checkout: false,
                    start_oid: Some(oid_for_branch.clone()),
                    error: None,
                }));
                s.ctx_menu = None;
            }),
            menu_item("Tag commit…", theme, move |s: &mut AppState| {
                s.modal = Some(Modal::Tag(crate::app::TagModalState {
                    oid: Some(oid_for_tag.clone()),
                    ..Default::default()
                }));
                s.ctx_menu = None;
            }),
            menu_item("Reset to here…", theme, move |s: &mut AppState| {
                s.modal = Some(Modal::Reset(ResetModalState {
                    oid: oid_for_reset.clone(),
                    mode: crate::app::ResetMode::default(),
                    error: None,
                }));
                s.ctx_menu = None;
            }),
            menu_item("Cherry-pick", theme, move |s: &mut AppState| {
                s.modal = Some(Modal::CherryPick(CherryPickModalState {
                    oid: oid_for_pick.clone(),
                    no_commit: false,
                    error: None,
                }));
                s.ctx_menu = None;
            }),
        ),
    )
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap((0.0_f64).px())
}

fn branch_items(name: String, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let n_co = name.clone();
    let n_rn = name.clone();
    let n_del = name.clone();
    let n_force = name.clone();

    flex(
        Axis::Vertical,
        (
            menu_label(&format!("branch  {name}"), theme),
            menu_separator(theme),
            menu_item("Checkout", theme, move |s: &mut AppState| {
                let branch = n_co.clone();
                let repo_path = s.repo.path.clone();
                if crate::app::is_demo_repo(&repo_path) {
                    s.toast = Some(Toast::error("demo mode".into()));
                    s.ctx_menu = None;
                    return;
                }
                match crate::git::ops::checkout(&repo_path, &branch) {
                    Ok(()) => {
                        s.refresh_all();
                        s.toast = Some(Toast::info(format!("checked out {branch}")));
                    }
                    Err(e) => {
                        s.toast = Some(Toast::error(format!("checkout failed: {e:#}")));
                    }
                }
                s.ctx_menu = None;
            }),
            menu_item("Rename…", theme, move |s: &mut AppState| {
                s.modal = Some(Modal::RenameBranch(RenameBranchModalState {
                    old_name: n_rn.clone(),
                    new_name: n_rn.clone(),
                    error: None,
                }));
                s.ctx_menu = None;
            }),
            menu_item("Delete", theme, move |s: &mut AppState| {
                run_delete_branch(s, &n_del, false);
            }),
            menu_item("Force delete", theme, move |s: &mut AppState| {
                run_delete_branch(s, &n_force, true);
            }),
        ),
    )
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap((0.0_f64).px())
}

fn run_delete_branch(s: &mut AppState, branch: &str, force: bool) {
    let repo_path = s.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        s.toast = Some(Toast::error("demo mode".into()));
        s.ctx_menu = None;
        return;
    }
    match crate::git::ops::delete_branch(&repo_path, branch, force) {
        Ok(()) => {
            s.refresh_all();
            s.toast = Some(Toast::info(format!(
                "{} {branch}",
                if force { "force-deleted" } else { "deleted" }
            )));
        }
        Err(e) => {
            // git error usually contains "not fully merged" — that's the cue
            // for the user to try Force delete instead.
            s.toast = Some(Toast::error(format!("delete failed: {e:#}")));
        }
    }
    s.ctx_menu = None;
}

fn remote_items(name: String, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let n_fetch = name.clone();
    let n_remove = name.clone();

    flex(
        Axis::Vertical,
        (
            menu_label(&format!("remote  {name}"), theme),
            menu_separator(theme),
            menu_item("Fetch", theme, move |s: &mut AppState| {
                let remote = n_fetch.clone();
                let repo_path = s.repo.path.clone();
                if crate::app::is_demo_repo(&repo_path) {
                    s.toast = Some(Toast::error("demo mode".into()));
                    s.ctx_menu = None;
                    return;
                }
                match crate::git::ops::fetch(&repo_path, &remote, false) {
                    Ok(()) => {
                        s.refresh_all();
                        s.toast = Some(Toast::info(format!("fetched {remote}")));
                    }
                    Err(e) => {
                        s.toast = Some(Toast::error(format!("fetch failed: {e:#}")));
                    }
                }
                s.ctx_menu = None;
            }),
            menu_item("Remove", theme, move |s: &mut AppState| {
                let remote = n_remove.clone();
                let repo_path = s.repo.path.clone();
                if crate::app::is_demo_repo(&repo_path) {
                    s.toast = Some(Toast::error("demo mode".into()));
                    s.ctx_menu = None;
                    return;
                }
                match crate::git::ops::remove_remote(&repo_path, &remote) {
                    Ok(()) => {
                        s.refresh_all();
                        s.toast = Some(Toast::info(format!("removed remote {remote}")));
                    }
                    Err(e) => {
                        s.toast = Some(Toast::error(format!("remove failed: {e:#}")));
                    }
                }
                s.ctx_menu = None;
            }),
        ),
    )
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap((0.0_f64).px())
}

fn tag_items(name: String, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let n_co = name.clone();
    let n_del = name.clone();

    flex(
        Axis::Vertical,
        (
            menu_label(&format!("tag  {name}"), theme),
            menu_separator(theme),
            menu_item("Checkout (detached)", theme, move |s: &mut AppState| {
                let tag = n_co.clone();
                let repo_path = s.repo.path.clone();
                if crate::app::is_demo_repo(&repo_path) {
                    s.toast = Some(Toast::error("demo mode".into()));
                    s.ctx_menu = None;
                    return;
                }
                match crate::git::ops::checkout(&repo_path, &tag) {
                    Ok(()) => {
                        s.refresh_all();
                        s.toast = Some(Toast::info(format!("checked out {tag}")));
                    }
                    Err(e) => {
                        s.toast = Some(Toast::error(format!("checkout failed: {e:#}")));
                    }
                }
                s.ctx_menu = None;
            }),
            menu_item("Delete tag", theme, move |s: &mut AppState| {
                let tag = n_del.clone();
                let repo_path = s.repo.path.clone();
                if crate::app::is_demo_repo(&repo_path) {
                    s.toast = Some(Toast::error("demo mode".into()));
                    s.ctx_menu = None;
                    return;
                }
                match crate::git::ops::delete_tag(&repo_path, &tag) {
                    Ok(()) => {
                        s.refresh_all();
                        s.toast = Some(Toast::info(format!("deleted tag {tag}")));
                    }
                    Err(e) => {
                        s.toast = Some(Toast::error(format!("delete failed: {e:#}")));
                    }
                }
                s.ctx_menu = None;
            }),
        ),
    )
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap((0.0_f64).px())
}

fn stash_items(idx: u32, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    flex(
        Axis::Vertical,
        (
            menu_label(&format!("stash@{{{idx}}}"), theme),
            menu_separator(theme),
            menu_item("Apply", theme, move |s: &mut AppState| {
                run_stash_op(s, idx, crate::git::ops::stash_apply, "applied");
            }),
            menu_item("Pop", theme, move |s: &mut AppState| {
                run_stash_op(s, idx, crate::git::ops::stash_pop, "popped");
            }),
            menu_item("Drop", theme, move |s: &mut AppState| {
                run_stash_op(s, idx, crate::git::ops::stash_drop, "dropped");
            }),
        ),
    )
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap((0.0_f64).px())
}

fn run_stash_op<F>(s: &mut AppState, idx: u32, op: F, verb: &'static str)
where
    F: FnOnce(&std::path::Path, u32) -> anyhow::Result<()>,
{
    let repo_path = s.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        s.toast = Some(Toast::error("demo mode".into()));
        s.ctx_menu = None;
        return;
    }
    match op(&repo_path, idx) {
        Ok(()) => {
            s.refresh_all();
            s.toast = Some(Toast::info(format!("{verb} stash@{{{idx}}}")));
        }
        Err(e) => {
            s.toast = Some(Toast::error(format!("stash {verb} failed: {e:#}")));
        }
    }
    s.ctx_menu = None;
}

fn menu_label(text: &str, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    sized_box(
        label(text.to_string())
            .text_size(11.0)
            .color(theme.text_muted),
    )
    .expand_width()
    .padding(Padding::from_vh(4.0, 10.0))
}

fn menu_separator(theme: &Theme) -> impl xilem::WidgetView<AppState> {
    sized_box(flex(Axis::Vertical, ()))
        .expand_width()
        .height((1.0_f64).px())
        .background_color(theme.border)
}

fn menu_item<F>(text: &'static str, theme: &Theme, on_click: F) -> impl xilem::WidgetView<AppState>
where
    F: Fn(&mut AppState) + Send + Sync + 'static,
{
    sized_box(flat_button(
        xilem::view::label(text).text_size(12.0).color(theme.text),
        FlatStyle {
            idle_bg: None,
            hover_bg: theme.bg_hover,
            active_bg: None,
            radius: 0.0,
            padding_v: 6.0,
            padding_h: 12.0,
        },
        false,
        move |s: &mut AppState| on_click(s),
    ))
    .expand_width()
}
