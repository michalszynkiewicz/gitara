//! 22px statusbar.
//! Left: current branch + ahead/behind. Center: last-fetched. Right: selection summary.

use crate::app::AppState;
use crate::model::repo::HeadState;

pub fn view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    use xilem::view::{flex, label, Axis};

    let branch = match &state.repo.head {
        HeadState::Branch { name } => name.clone(),
        HeadState::Detached { oid } => format!("HEAD @ {}", &oid[..7]),
        HeadState::Unborn => "(unborn)".into(),
    };

    let sel = match state.selection.set.len() {
        0 => String::new(),
        1 => {
            let oid = &state.selection.set[0];
            state.commits.iter()
                .find(|c| &c.oid == oid)
                .map(|c| format!("{} · {}", c.short, c.subject))
                .unwrap_or_default()
        }
        n => format!("{n} commits selected"),
    };

    flex((
        label(branch).brush(state.theme.text_muted),
        xilem::view::FlexSpacer::Flex(1.0),
        label(sel).brush(state.theme.text_dim),
    ))
    .direction(Axis::Horizontal)
}
