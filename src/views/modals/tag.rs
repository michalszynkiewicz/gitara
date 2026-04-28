//! Tag modal — create a tag (lightweight or annotated) at HEAD or a
//! chosen commit. An empty message produces a lightweight ref-only tag;
//! a non-empty message produces an annotated tag (`-a -m`) and goes
//! through the user's signing/hooks/config.

use crate::app::{AppState, Modal, TagModalState};
use crate::git;
use crate::theme::Theme;
use xilem::view::{flex, label, sized_box, textbox, Axis, CrossAxisAlignment, FlexSpacer, Padding};
use xilem::WidgetView as _;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let theme = state.theme.clone();
    let s = tag_state(state).cloned().unwrap_or_default();

    let body: Box<xilem::AnyWidgetView<AppState>> = body_view(&s, &theme).boxed();
    let footer = super::ok_cancel_footer(&theme, "Create", run_create);

    let subtitle = match &s.oid {
        Some(oid) => {
            let short = &oid[..oid.len().min(7)];
            format!("Tag commit {short}")
        }
        None => "Tag the current HEAD".to_string(),
    };

    super::shell("New tag", &subtitle, body, footer, &theme)
}

fn body_view(s: &TagModalState, theme: &Theme) -> impl xilem::WidgetView<AppState> {
    let name_label = label("name")
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM);

    let name_input = sized_box(
        textbox(s.name.clone(), |st: &mut AppState, new| {
            if let Some(ts) = tag_state_mut(st) {
                ts.name = new;
                ts.error = None;
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

    let msg_label = label("message (optional — leave empty for lightweight tag)")
        .brush(theme.text_dim)
        .text_size(10.0)
        .weight(xilem::FontWeight::MEDIUM);

    let msg_input = sized_box(
        textbox(s.message.clone(), |st: &mut AppState, new| {
            if let Some(ts) = tag_state_mut(st) {
                ts.message = new;
                ts.error = None;
            }
        })
        .brush(super::input_text()),
    )
    .expand_width()
    .height(32.0)
    .background(super::input_bg())
    .border(theme.border, 1.0)
    .rounded(4.0)
    .padding(Padding::from_vh(4.0, 8.0));

    let error_view: Box<xilem::AnyWidgetView<AppState>> = match &s.error {
        Some(err) => label(err.clone()).brush(theme.removed).text_size(11.0).boxed(),
        None => label("").boxed(),
    };

    flex((
        name_label,
        name_input,
        FlexSpacer::Fixed(12.0),
        msg_label,
        msg_input,
        FlexSpacer::Fixed(8.0),
        error_view,
    ))
    .direction(Axis::Vertical)
    .cross_axis_alignment(CrossAxisAlignment::Start)
    .gap(4.0)
}

fn run_create(st: &mut AppState) {
    let (name, message, oid) = match tag_state(st) {
        Some(ts) => (ts.name.trim().to_string(), ts.message.clone(), ts.oid.clone()),
        None => return,
    };

    let repo_path = st.repo.path.clone();
    if crate::app::is_demo_repo(&repo_path) {
        if let Some(ts) = tag_state_mut(st) {
            ts.error = Some(
                "demo mode — start gitara from a real repo or set GITARA_REPO=<path>".into(),
            );
        }
        return;
    }
    match git::ops::create_tag(&repo_path, &name, oid.as_deref(), &message) {
        Ok(()) => {
            st.refresh_all();
            st.modal = None;
        }
        Err(e) => {
            if let Some(ts) = tag_state_mut(st) {
                ts.error = Some(format!("{e:#}"));
            }
        }
    }
}

fn tag_state(state: &AppState) -> Option<&TagModalState> {
    match &state.modal {
        Some(Modal::Tag(s)) => Some(s),
        _ => None,
    }
}

fn tag_state_mut(state: &mut AppState) -> Option<&mut TagModalState> {
    match &mut state.modal {
        Some(Modal::Tag(s)) => Some(s),
        _ => None,
    }
}
