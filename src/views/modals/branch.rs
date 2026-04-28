//! Branch modal — creates a new local branch from HEAD (or a chosen
//! starting point) and optionally checks it out.

use crate::app::{AppState, BranchModalState, Modal};
use crate::git;
use crate::theme::Theme;
use crate::widgets::flat_button::{flat_button, FlatStyle};
use xilem::view::{flex, label, sized_box, textbox, Axis, CrossAxisAlignment, FlexSpacer, Padding};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let s = branch_state(state).cloned().unwrap_or_default();

    let body: Box<xilem::AnyWidgetView<AppState>> = body_view(&s, &theme).boxed();
    let footer = footer_view(&theme);

    let subtitle = match &s.start_oid {
        Some(oid) => {
            let short = &oid[..oid.len().min(7)];
            format!("Create a branch starting at {short}")
        }
        None => "Create a branch from the current HEAD".to_string(),
    };

    super::shell(
        "New branch",
        &subtitle,
        body,
        footer,
        &theme,
    )
}

fn body_view(s: &BranchModalState, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let name_label = label("name")
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM);

    let name_input = sized_box(
        textbox(s.name.clone(), |st: &mut AppState, new| {
            if let Some(bs) = branch_state_mut(st) {
                bs.name = new;
                bs.error = None;
            }
        })
        .on_enter(|st: &mut AppState, _| run_create(st))
        .brush(super::input_text()),
    )
    .expand_width()
    .height(32.0)
    .background(super::input_bg())
    .border(theme.border, 1.0)
    .rounded(4.0)
    .padding(Padding::from_vh(4.0, 8.0));

    // Checkout toggle — a small pill button that reflects current state.
    let checkout_label = if s.checkout { "✓ check out after creating" } else { "check out after creating" };
    let checkout_btn = flat_button(
        xilem::view::label(checkout_label)
            .brush(if s.checkout { theme.accent } else { theme.text_muted })
            .text_size(11.0),
        FlatStyle {
            idle_bg: None,
            hover_bg: theme.bg_hover,
            active_bg: Some(theme.accent_tint),
            radius: 4.0,
            padding_v: 4.0,
            padding_h: 8.0,
        },
        s.checkout,
        |st: &mut AppState| {
            if let Some(bs) = branch_state_mut(st) {
                bs.checkout = !bs.checkout;
            }
        },
    );

    let error_view: Box<xilem::AnyWidgetView<AppState>> = match &s.error {
        Some(err) => label(err.clone())
            .brush(theme.removed)
            .text_size(11.0)
            .boxed(),
        None => label("").boxed(),
    };

    flex((
        name_label,
        name_input,
        FlexSpacer::Fixed(12.0),
        checkout_btn,
        FlexSpacer::Fixed(8.0),
        error_view,
    ))
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(4.0)
}

fn footer_view(theme: &Theme) -> Box<xilem::AnyWidgetView<AppState>> {
    super::ok_cancel_footer(theme, "Create", run_create)
}

fn run_create(st: &mut AppState) {
    let (name, checkout, start_oid) = match branch_state(st) {
        Some(bs) => (bs.name.trim().to_string(), bs.checkout, bs.start_oid.clone()),
        None => return,
    };

    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(bs) = branch_state_mut(st) {
            bs.error = Some(
                "demo mode — start gitara from a real repo or set GITARA_REPO=<path>".into(),
            );
        }
        return;
    }
    match git::ops::create_branch(&repo_path, &name, start_oid.as_deref(), checkout) {
        Ok(_) => {
            st.refresh_all();
            st.modal = None;
        }
        Err(e) => {
            if let Some(bs) = branch_state_mut(st) {
                bs.error = Some(format!("{e:#}"));
            }
        }
    }
}

fn branch_state(state: &AppState) -> Option<&BranchModalState> {
    match &state.modal {
        Some(Modal::Branch(s)) => Some(s),
        _ => None,
    }
}

fn branch_state_mut(state: &mut AppState) -> Option<&mut BranchModalState> {
    match &mut state.modal {
        Some(Modal::Branch(s)) => Some(s),
        _ => None,
    }
}
