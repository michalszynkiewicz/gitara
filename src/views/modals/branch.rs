//! Branch modal — creates a new local branch from HEAD (or a chosen
//! starting point) and optionally checks it out.

use crate::app::{AppState, BranchModalState, Modal};
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

    super::shell("New branch", &subtitle, body, footer, &theme)
}

fn body_view(s: &BranchModalState, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let name_label = label("name")
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM)
        .color(theme.text_dim);

    let name_input = sized_box(
        text_input(s.name.clone(), |st: &mut AppState, new| {
            if let Some(bs) = branch_state_mut(st) {
                bs.name = new;
                bs.error = None;
            }
        })
        .on_enter(|st: &mut AppState, _| run_create(st))
        .text_color(super::input_text()),
    )
    .expand_width()
    .height((32.0_f64).px())
    .corner_radius(4.0)
    .background_color(super::input_bg())
    .border(theme.border, 1.0)
    .padding(Padding::from_vh(4.0, 8.0));

    // Checkout toggle — a small pill button that reflects current state.
    let checkout_color = if s.checkout {
        theme.accent
    } else {
        theme.text_muted
    };
    let checkout_btn = flat_button(
        crate::ui::toggle_row(s.checkout, "check out after creating", checkout_color, 11.0),
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
            .text_size(11.0)
            .color(theme.removed)
            .boxed(),
        None => label("").boxed(),
    };

    flex(
        Axis::Vertical,
        (
            name_label,
            name_input,
            FlexSpacer::Fixed((12.0_f64).px()),
            checkout_btn,
            FlexSpacer::Fixed((8.0_f64).px()),
            error_view,
        ),
    )
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap((4.0_f64).px())
}

fn footer_view(theme: &Theme) -> Box<xilem::AnyWidgetView<AppState>> {
    super::ok_cancel_footer(theme, "Create", run_create)
}

fn run_create(st: &mut AppState) {
    let (name, checkout, start_oid) = match branch_state(st) {
        Some(bs) => (
            bs.name.trim().to_string(),
            bs.checkout,
            bs.start_oid.clone(),
        ),
        None => return,
    };

    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(bs) = branch_state_mut(st) {
            bs.error =
                Some("demo mode — start gitara from a real repo or set GITARA_REPO=<path>".into());
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
