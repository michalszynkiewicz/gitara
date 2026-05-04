//! AppState + root view composition.

use crate::model::{commit::Commit, reflog::ReflogEntry, repo::RepoView};
use crate::persist::Settings;
use crate::theme::{Theme, ThemeMode};
use crate::views;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    History,
    Reflog,
}

#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub primary: Option<String>, // oid of last-clicked
    pub set: Vec<String>,        // all selected
    pub anchor: Option<String>,  // for shift-click range
}

#[derive(Clone, Debug)]
pub struct InspectorState {
    pub collapsed: bool,
    pub width: f64, // 280..=720
    pub tab: InspectorTab,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectorTab {
    Changes,
    Diff,
    Files,
    Details,
}

#[derive(Clone, Debug, Default)]
pub struct BranchModalState {
    pub name: String,
    pub checkout: bool,
    /// Optional commit oid to branch from. When None, branches from HEAD.
    /// Set by the commit context menu's "Create branch from here" item.
    pub start_oid: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ResetMode {
    /// Move HEAD only — index and working tree untouched.
    Soft,
    /// Move HEAD + reset index — working tree untouched. Default `git reset`.
    #[default]
    Mixed,
    /// Move HEAD + reset index + working tree. Destructive.
    Hard,
}

#[derive(Clone, Debug, Default)]
pub struct ResetModalState {
    pub oid: String,
    pub mode: ResetMode,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct RenameBranchModalState {
    pub old_name: String,
    pub new_name: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct TagModalState {
    pub name: String,
    /// Optional message — non-empty makes the tag annotated (-a -m),
    /// empty makes it lightweight (a ref-only tag).
    pub message: String,
    /// Optional commit oid to tag. None → tag HEAD.
    pub oid: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct FetchModalState {
    pub remote: String,
    pub prune: bool,
    pub error: Option<String>,
    pub running: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PushModalState {
    pub remote: String,
    /// Local branch being pushed (read from HEAD when the modal opens).
    pub branch: String,
    /// Remote branch name to push *to*. Defaults to `branch` (push to a
    /// same-named branch) but the user can pick from existing remote
    /// branches or type a new name.
    pub target_branch: String,
    pub force_with_lease: bool,
    pub error: Option<String>,
    pub running: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CommitModalState {
    pub message: String,
    pub amend: bool,
    pub error: Option<String>,
    /// Working-tree files that aren't yet staged. Refreshed after every
    /// stage/unstage so the lists stay in sync with the index.
    pub unstaged: Vec<crate::model::diff::FileChange>,
    /// Files in the index that differ from HEAD — what `git commit` would
    /// commit.
    pub staged: Vec<crate::model::diff::FileChange>,
    /// Path of the file whose diff is shown in the right pane. Cleared
    /// when the file goes away (e.g. after staging it).
    pub selected_path: Option<std::path::PathBuf>,
    /// Whether the selected file is in the staged or unstaged side.
    /// Drives whether we read its diff from the index or the worktree.
    pub selected_staged: bool,
    /// Cached hunks for the currently-selected file. Re-read on selection
    /// change and after stage/unstage.
    pub hunks: Vec<(std::path::PathBuf, crate::model::diff::Hunk)>,
    /// Last click on a file row — used to detect a double-click for the
    /// stage/unstage shortcut. Resets on any non-row interaction.
    pub last_click: Option<(std::path::PathBuf, std::time::Instant)>,
    /// Set when a background read (diff, status) fails. The lists keep
    /// their last-known-good contents; this surfaces the failure.
    pub read_error: Option<String>,
}

impl CommitModalState {
    /// Build a fresh modal state with file lists loaded from disk and an
    /// initial selection (first staged, then first unstaged, else none).
    pub fn open(repo_path: &std::path::Path) -> Self {
        let mut s = Self::default();
        s.reload_lists(repo_path);
        // Initial selection — prefer something whose diff pane will
        // actually show content. Untracked files come from
        // diff_index_to_workdir as zero-hunk entries, so try a tracked
        // change first.
        let pick_tracked = |files: &[crate::model::diff::FileChange]| {
            files
                .iter()
                .find(|f| !matches!(f.status, crate::model::diff::FileStatus::Untracked))
                .map(|f| f.path.clone())
        };
        if let Some(p) = pick_tracked(&s.staged) {
            s.selected_path = Some(p);
            s.selected_staged = true;
        } else if let Some(p) = pick_tracked(&s.unstaged) {
            s.selected_path = Some(p);
            s.selected_staged = false;
        } else if let Some(f) = s.staged.first() {
            s.selected_path = Some(f.path.clone());
            s.selected_staged = true;
        } else if let Some(f) = s.unstaged.first() {
            s.selected_path = Some(f.path.clone());
            s.selected_staged = false;
        }
        s.reload_hunks(repo_path);
        s
    }

    /// Re-read both file lists. Keeps the existing selection if the file
    /// still exists on its side; otherwise tries the other side; otherwise
    /// clears.
    ///
    /// On read failure the existing lists are **preserved** (so the UI
    /// never silently shows "no changes") and `read_error` is set.
    pub fn reload_lists(&mut self, repo_path: &std::path::Path) {
        match (
            crate::git::diff::unstaged_files(repo_path),
            crate::git::diff::staged_files(repo_path),
        ) {
            (Ok(unstaged), Ok(staged)) => {
                self.unstaged = unstaged;
                self.staged = staged;
                self.read_error = None;
            }
            (Err(e), _) | (_, Err(e)) => {
                self.read_error = Some(format!("failed to read working tree: {e:#}"));
                return;
            }
        }

        if let Some(p) = &self.selected_path {
            let on_staged = self.staged.iter().any(|f| &f.path == p);
            let on_unstaged = self.unstaged.iter().any(|f| &f.path == p);
            if self.selected_staged && !on_staged && on_unstaged {
                self.selected_staged = false;
            } else if !self.selected_staged && !on_unstaged && on_staged {
                self.selected_staged = true;
            } else if !on_staged && !on_unstaged {
                self.selected_path = None;
            }
        }
        // No selection? Pick the first available one.
        if self.selected_path.is_none() {
            if let Some(f) = self.staged.first() {
                self.selected_path = Some(f.path.clone());
                self.selected_staged = true;
            } else if let Some(f) = self.unstaged.first() {
                self.selected_path = Some(f.path.clone());
                self.selected_staged = false;
            }
        }
    }

    /// Re-read the hunks for the currently-selected file.
    ///
    /// On read failure the existing hunks are preserved and `read_error`
    /// is set.
    pub fn reload_hunks(&mut self, repo_path: &std::path::Path) {
        let result = match &self.selected_path {
            Some(p) if self.selected_staged => {
                crate::git::diff::hunks_staged_for_path(repo_path, p)
            }
            Some(p) => crate::git::diff::hunks_unstaged_for_path(repo_path, p),
            None => {
                self.hunks = Vec::new();
                return;
            }
        };
        match result {
            Ok(hunks) => {
                self.hunks = hunks;
                self.read_error = None;
            }
            Err(e) => {
                self.read_error = Some(format!("failed to read diff: {e:#}"));
            }
        }
    }

    /// Re-read both lists *and* hunks. Call after stage/unstage.
    pub fn refresh(&mut self, repo_path: &std::path::Path) {
        self.reload_lists(repo_path);
        self.reload_hunks(repo_path);
    }
}

#[derive(Clone, Debug, Default)]
pub struct MergeModalState {
    pub branch: String,
    pub no_ff: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct RebaseModalState {
    pub onto: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct AddRemoteModalState {
    pub name: String,
    pub url: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct CherryPickModalState {
    pub oid: String, // pre-filled from selection
    pub no_commit: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Modal {
    Commit(CommitModalState),
    Rebase(RebaseModalState),
    Fetch(FetchModalState),
    Push(PushModalState),
    // Kept variant + state struct so the modal code compiles, but
    // never constructed — see ISSUES.md for the URL-validation gap
    // that needs to land before re-enabling the UI entry.
    #[allow(dead_code)]
    AddRemote(AddRemoteModalState),
    Branch(BranchModalState),
    Merge(MergeModalState),
    CherryPick(CherryPickModalState),
    Reset(ResetModalState),
    RenameBranch(RenameBranchModalState),
    Tag(TagModalState),
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // Branch / Remote / Tag / Stash kinds are wired up as we add their menus.
pub enum CtxMenuKind {
    Commit { oid: String },
    Branch { name: String },
    Remote { name: String },
    Tag { name: String },
    Stash { idx: u32 },
}

#[derive(Clone, Debug)]
pub struct CtxMenu {
    pub x: f64,
    pub y: f64,
    pub kind: CtxMenuKind,
}

/// Returns true when the app is in demo mode — no real repo is loaded and
/// write ops should fail with a helpful message instead of trying to touch
/// a bogus path.
pub fn is_demo_repo(path: &std::path::Path) -> bool {
    path.starts_with("/nonexistent/demo")
}

/// Transient banner shown above the statusbar to surface op results.
#[derive(Clone, Debug)]
pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Error,
}

impl Toast {
    pub fn info(message: String) -> Self {
        Self {
            kind: ToastKind::Info,
            message,
        }
    }
    pub fn error(message: String) -> Self {
        Self {
            kind: ToastKind::Error,
            message,
        }
    }
}

pub struct AppState {
    pub theme_mode: ThemeMode,
    pub theme: Theme,
    pub repo: RepoView,
    pub commits: Vec<Commit>,
    pub reflog: Vec<ReflogEntry>,
    pub working_status: Option<crate::git::status::WorkingStatus>,
    pub selection: Selection,
    pub view: View,
    pub inspector: InspectorState,
    pub sidebar_collapsed: bool,
    pub modal: Option<Modal>,
    pub ctx_menu: Option<CtxMenu>,
    pub toast: Option<Toast>,
    /// gitk-style "show all branches" toggle. When true, the History tab
    /// walks every branch tip + tag in chronological order; when false,
    /// only commits reachable from HEAD.
    pub show_all_refs: bool,
    /// When true, long commit subjects wrap to multiple lines and the
    /// row grows vertically. When false, subjects clip with no
    /// wrapping; row stays compact.
    pub wrap_subjects: bool,
}

impl AppState {
    pub fn boot(settings: Settings) -> anyhow::Result<Self> {
        // Theme override: GITARA_DARK forces dark, GITARA_LIGHT forces light.
        // If neither is set, the persisted setting wins (defaults to Dark).
        let theme_mode = if std::env::var_os("GITARA_DARK").is_some() {
            ThemeMode::Dark
        } else if std::env::var_os("GITARA_LIGHT").is_some() {
            ThemeMode::Light
        } else {
            settings.theme
        };
        let theme = match theme_mode {
            ThemeMode::Light => Theme::light(),
            ThemeMode::Dark => Theme::dark(),
        };

        // Repo-loading preference order:
        //   1. GITARA_REPO env var, if set
        //   2. current working directory, if it's a git repo
        //   3. mock data (demo mode — write ops disabled)
        let candidate = std::env::var_os("GITARA_REPO")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::current_dir().ok());

        let (repo, commits, reflog, working_status) = match candidate {
            Some(path) => match crate::git::refs::load_repo(&path) {
                Ok(repo_view) => {
                    // load_repo may have walked upward — reuse its resolved path.
                    let real_path = repo_view.path.clone();
                    let commits = crate::git::log::load_commits(&real_path, &repo_view, 200, true)
                        .unwrap_or_else(|e| {
                            tracing::warn!("load_commits failed: {e:?}");
                            Vec::new()
                        });
                    let reflog = crate::git::refs::reflog(&real_path).unwrap_or_default();
                    let status = crate::git::status::read(&real_path).ok();
                    (repo_view, commits, reflog, status)
                }
                Err(e) => {
                    tracing::warn!(
                        "load_repo({}) failed — using mock data: {e:?}",
                        path.display()
                    );
                    let (r, c, rl) = crate::mock::seed();
                    (r, c, rl, None)
                }
            },
            None => {
                let (r, c, rl) = crate::mock::seed();
                (r, c, rl, None)
            }
        };

        // Optional pre-selection for screenshot tests — e.g.
        // GITARA_SELECT=a1f3b2c selects the first commit whose oid starts
        // with the given prefix. The literal __working_tree__ sentinel
        // selects the working-tree pseudo-row.
        let mut selection = Selection::default();
        if let Ok(prefix) = std::env::var("GITARA_SELECT") {
            if prefix == crate::views::graph::WORKING_TREE_OID {
                selection.primary = Some(prefix.clone());
                selection.set = vec![prefix.clone()];
                selection.anchor = Some(prefix);
            } else if let Some(c) = commits
                .iter()
                .find(|c| c.oid.starts_with(&prefix) || c.short == prefix)
            {
                selection.primary = Some(c.oid.clone());
                selection.set = vec![c.oid.clone()];
                selection.anchor = Some(c.oid.clone());
            }
        }

        // Compute env-var-driven modal *before* moving repo into Self.
        let modal = std::env::var("GITARA_MODAL").ok().and_then(|v| {
            let default_remote = repo
                .remotes
                .first()
                .map(|r| r.name.clone())
                .unwrap_or_default();
            let current_branch = repo
                .branches
                .iter()
                .find(|b| b.current)
                .map(|b| b.name.clone())
                .unwrap_or_default();
            match v.as_str() {
                "commit" => Some(Modal::Commit(CommitModalState::open(&repo.path))),
                "fetch" => Some(Modal::Fetch(FetchModalState {
                    remote: default_remote,
                    prune: false,
                    error: None,
                    running: false,
                })),
                "push" => Some(Modal::Push(PushModalState {
                    remote: default_remote,
                    target_branch: current_branch.clone(),
                    branch: current_branch,
                    force_with_lease: false,
                    error: None,
                    running: false,
                })),
                "branch" => Some(Modal::Branch(BranchModalState {
                    start_oid: selection
                        .primary
                        .clone()
                        .filter(|p| p != crate::views::graph::WORKING_TREE_OID),
                    ..Default::default()
                })),
                "merge" => Some(Modal::Merge(MergeModalState::default())),
                "rebase" => Some(Modal::Rebase(RebaseModalState::default())),
                // "add_remote" deliberately removed — see ISSUES.md.
                // The current implementation passes user-supplied URLs
                // positionally to `git remote add`, which lets a URL
                // like `-oProxyCommand=...` execute on next fetch
                // (CVE-2017-1000117 class). Disabled until the modal
                // gains scheme-allow-list validation.
                "cherry_pick" => Some(Modal::CherryPick(CherryPickModalState::default())),
                "reset" => Some(Modal::Reset(ResetModalState {
                    oid: selection.primary.clone().unwrap_or_default(),
                    mode: ResetMode::default(),
                    error: None,
                })),
                "rename_branch" => {
                    let cur = repo
                        .branches
                        .iter()
                        .find(|b| b.current)
                        .map(|b| b.name.clone())
                        .unwrap_or_default();
                    Some(Modal::RenameBranch(RenameBranchModalState {
                        old_name: cur.clone(),
                        new_name: cur,
                        error: None,
                    }))
                }
                "tag" => Some(Modal::Tag(TagModalState {
                    oid: selection
                        .primary
                        .clone()
                        .filter(|p| p != crate::views::graph::WORKING_TREE_OID),
                    ..Default::default()
                })),
                _ => None,
            }
        });

        // Optional pre-opened context menu for screenshot tests —
        // GITARA_CTX_MENU=commit pins one onto the currently selected commit.
        let ctx_menu = std::env::var("GITARA_CTX_MENU")
            .ok()
            .and_then(|v| match v.as_str() {
                "commit" => selection.primary.clone().map(|oid| CtxMenu {
                    x: 360.0,
                    y: 180.0,
                    kind: CtxMenuKind::Commit { oid },
                }),
                _ => None,
            });

        Ok(Self {
            theme_mode,
            theme,
            repo,
            commits,
            reflog,
            working_status,
            selection,
            view: View::History,
            inspector: InspectorState {
                collapsed: false,
                width: settings.inspector_w.unwrap_or(420.0),
                tab: std::env::var("GITARA_TAB")
                    .ok()
                    .and_then(|v| match v.to_lowercase().as_str() {
                        "changes" => Some(InspectorTab::Changes),
                        "diff" => Some(InspectorTab::Diff),
                        "files" => Some(InspectorTab::Files),
                        "details" => Some(InspectorTab::Details),
                        _ => None,
                    })
                    .unwrap_or(InspectorTab::Details),
            },
            sidebar_collapsed: settings.sidebar_collapsed,
            modal,
            ctx_menu,
            toast: None,
            show_all_refs: true,
            wrap_subjects: true,
        })
    }

    /// Refresh the working-tree status. Safe to call after any op that
    /// might change index/working-tree state, or after a checkout.
    pub fn reload_working_status(&mut self) {
        if is_demo_repo(&self.repo.path) {
            self.working_status = None;
            return;
        }
        match crate::git::status::read(&self.repo.path) {
            Ok(s) => self.working_status = Some(s),
            Err(_) => {} // preserve last-known-good; caller surfaces via toast
        }
    }

    /// Reload the commit log honouring the current `show_all_refs` flag.
    /// Op handlers call this after any change that might affect history.
    pub fn reload_commits(&mut self) {
        if let Ok(commits) =
            crate::git::log::load_commits(&self.repo.path, &self.repo, 200, self.show_all_refs)
        {
            self.commits = commits;
        }
    }

    /// One-shot full refresh — repo metadata, commits, reflog, and the
    /// working-tree status. Centralised so every op handler (and the
    /// manual Refresh button / F5 binding) goes through the same path,
    /// and so future cache invalidation stays in one place.
    pub fn refresh_all(&mut self) {
        if is_demo_repo(&self.repo.path) {
            return;
        }
        let repo_path = self.repo.path.clone();
        if let Ok(repo) = crate::git::refs::load_repo(&repo_path) {
            self.repo = repo;
        }
        self.reload_commits();
        match crate::git::refs::reflog(&self.repo.path) {
            Ok(rl) => self.reflog = rl,
            Err(e) => {
                self.toast = Some(Toast::error(format!("failed to read reflog: {e:#}")));
            }
        }
        self.reload_working_status();
    }

    pub fn toggle_theme(&mut self) {
        self.theme_mode = match self.theme_mode {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
        };
        self.theme = match self.theme_mode {
            ThemeMode::Light => Theme::light(),
            ThemeMode::Dark => Theme::dark(),
        };
    }
}

/// Root view — titlebar, toolbar, body (sidebar + graph + inspector), statusbar.
/// Modal overlays (when `state.modal` is set) are layered on top via zstack.
pub fn root_view(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    use xilem::masonry::properties::types::AsUnit as _;
    use xilem::style::{Padding, Style as _};
    use xilem::view::{
        flex, sized_box, zstack, Axis, CrossAxisAlignment, FlexExt as _, MainAxisAlignment,
    };
    use xilem::WidgetView as _;

    let theme = state.theme.clone();
    let sidebar_w = 220.0;
    let inspector_w = state.inspector.width;

    let titlebar = sized_box(views::titlebar::view(state))
        .height((30.0_f64).px())
        .expand_width()
        .background_color(theme.bg_titlebar)
        .padding(Padding::horizontal(12.0));

    let toolbar = sized_box(views::toolbar::view(state))
        .height((36.0_f64).px())
        .expand_width()
        .background_color(theme.bg_panel)
        .padding(Padding::horizontal(10.0));

    let sidebar_panel = (!state.sidebar_collapsed).then(|| {
        sized_box(views::sidebar::view(state))
            .width((sidebar_w).px())
            .expand_height()
            .background_color(theme.bg_panel_2)
            .border(theme.border, 1.0)
            .padding(Padding::from_vh(10.0, 10.0))
    });

    let graph_panel = sized_box(views::graph::view(state))
        .expand()
        .background_color(theme.bg_panel);

    let inspector_panel = (!state.inspector.collapsed).then(|| {
        sized_box(views::inspector::view(state))
            .width((inspector_w).px())
            .expand_height()
            .background_color(theme.bg_panel_2)
            .border(theme.border, 1.0)
    });

    let body = flex(
        Axis::Vertical,
        (sidebar_panel, graph_panel.flex(1.0), inspector_panel),
    )
    .direction(Axis::Horizontal)
    .cross_axis_alignment(CrossAxisAlignment::Fill)
    .gap((0.0_f64).px());

    let statusbar = sized_box(views::statusbar::view(state))
        .height((22.0_f64).px())
        .expand_width()
        .background_color(theme.bg_titlebar)
        .padding(Padding::horizontal(10.0));

    let toast: Option<_> = state.toast.as_ref().map(|t| {
        let (fg, bg) = match t.kind {
            ToastKind::Info => (theme.text, theme.accent_tint),
            ToastKind::Error => (theme.accent_fg, theme.removed),
        };
        sized_box(
            flex(
                Axis::Vertical,
                (
                    crate::ui::label(t.message.clone())
                        .text_size(12.0)
                        .weight(xilem::FontWeight::MEDIUM)
                        .color(fg),
                    xilem::view::FlexSpacer::Flex(1.0),
                    crate::widgets::flat_button::flat_button(
                        crate::ui::label("dismiss").text_size(11.0).color(fg),
                        crate::widgets::flat_button::FlatStyle {
                            idle_bg: None,
                            hover_bg: vello::peniko::Color::from_rgba8(255, 255, 255, 40),
                            active_bg: None,
                            radius: 3.0,
                            padding_v: 2.0,
                            padding_h: 8.0,
                        },
                        false,
                        |s: &mut AppState| s.toast = None,
                    ),
                ),
            )
            .direction(Axis::Horizontal)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .gap((8.0_f64).px()),
        )
        .height((28.0_f64).px())
        .expand_width()
        .background_color(bg)
        .padding(Padding::horizontal(12.0))
    });

    let main = sized_box(
        flex(
            Axis::Vertical,
            (titlebar, toolbar, body.flex(1.0), toast, statusbar),
        )
        .direction(Axis::Vertical)
        .cross_axis_alignment(CrossAxisAlignment::Fill)
        .main_axis_alignment(MainAxisAlignment::Start)
        .gap((0.0_f64).px()),
    )
    .expand()
    .background_color(theme.bg);

    // Modal and ctx-menu must share one Option slot — Xilem 0.3's ZStack
    // panics when two sibling Option<View> children change shape in the
    // same render pass (e.g. opening a modal from the ctx-menu transitions
    // modal None→Some AND ctx_menu Some→None at once, and ZStack's child
    // tracking gets confused). Modal wins when both are set; the menu
    // closes itself in every action callback anyway, so this matches our
    // UX without hitting the bug.
    let overlay: Option<Box<xilem::AnyWidgetView<AppState>>> =
        if let Some(m) = views::modals::view(state) {
            Some(m.boxed())
        } else {
            views::context_menu::view(state).map(|m| m.boxed())
        };

    // TopLeft alignment so the ctx-menu's spacer-based positioning is
    // honored verbatim. `main` is full-window so alignment is moot for it,
    // and `modal` wraps its own ZStack with Center alignment.
    zstack((main.boxed(), overlay))
        .alignment(xilem::masonry::properties::types::UnitPoint::TOP_LEFT)
}
