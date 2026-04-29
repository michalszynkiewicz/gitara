//! Add Remote modal — `git remote add <name> <url>`.

use crate::app::{AddRemoteModalState, AppState, Modal, Toast};
use crate::git;
use crate::theme::Theme;
use xilem::view::{flex, label, sized_box, textbox, Axis, CrossAxisAlignment, FlexSpacer, Padding};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let s = state_get(state).cloned().unwrap_or_default();

    let body = body_view(&s, &theme).boxed();
    let footer = super::ok_cancel_footer(&theme, "Add", run_add);

    super::shell(
        "Add remote",
        "Register a new git remote",
        body,
        footer,
        &theme,
    )
}

fn body_view(s: &AddRemoteModalState, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let name_label = label("name")
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM);
    let name_input = sized_box(
        textbox(s.name.clone(), |st: &mut AppState, new| {
            if let Some(rs) = state_mut(st) {
                rs.name = new;
                rs.error = None;
            }
        })
        .on_enter(|st: &mut AppState, _| run_add(st))
        .brush(super::input_text()),
    )
    .expand_width()
    .height(32.0)
    .background(super::input_bg())
    .border(theme.border, 1.0)
    .rounded(4.0)
    .padding(Padding::from_vh(4.0, 8.0));

    let url_label = label("url")
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM);
    let url_input = sized_box(
        textbox(s.url.clone(), |st: &mut AppState, new| {
            if let Some(rs) = state_mut(st) {
                rs.url = new;
                rs.error = None;
            }
        })
        .on_enter(|st: &mut AppState, _| run_add(st))
        .brush(super::input_text()),
    )
    .expand_width()
    .height(32.0)
    .background(super::input_bg())
    .border(theme.border, 1.0)
    .rounded(4.0)
    .padding(Padding::from_vh(4.0, 8.0));

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
        FlexSpacer::Fixed(10.0),
        url_label,
        url_input,
        FlexSpacer::Fixed(10.0),
        error_view,
    ))
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(4.0)
}

fn run_add(st: &mut AppState) {
    let (name, url) = match state_get(st) {
        Some(s) => (s.name.trim().to_string(), s.url.trim().to_string()),
        None => return,
    };
    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(s) = state_mut(st) {
            s.error = Some("demo mode".into());
        }
        return;
    }

    match git::ops::add_remote(&repo_path, &name, &url) {
        Ok(()) => {
            st.refresh_all();
            st.toast = Some(Toast::info(format!("added remote {name}")));
            st.modal = None;
        }
        Err(e) => {
            if let Some(s) = state_mut(st) {
                s.error = Some(format!("{e:#}"));
            }
        }
    }
}

fn state_get(state: &AppState) -> Option<&AddRemoteModalState> {
    match &state.modal {
        Some(Modal::AddRemote(s)) => Some(s),
        _ => None,
    }
}

fn state_mut(state: &mut AppState) -> Option<&mut AddRemoteModalState> {
    match &mut state.modal {
        Some(Modal::AddRemote(s)) => Some(s),
        _ => None,
    }
}
