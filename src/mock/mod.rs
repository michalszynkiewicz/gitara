//! Mock data — same shapes as `design/data.jsx`, enough to render the UI
//! end-to-end before `crate::git::*` is wired up.

use std::path::PathBuf;
use time::OffsetDateTime;

use crate::model::{
    commit::{Author, Commit, RefChip},
    reflog::{ReflogAction, ReflogEntry},
    repo::{Branch, HeadState, RepoView, Remote, Stash, Tag},
};

pub fn seed() -> (RepoView, Vec<Commit>, Vec<ReflogEntry>) {
    let now = OffsetDateTime::now_utc();

    let repo = RepoView {
        // Sentinel path — `crate::app::is_demo_repo` checks for this exact
        // prefix to refuse write ops in demo mode.
        path: PathBuf::from("/nonexistent/demo/ordo"),
        name: "ordo (demo)".into(),
        head: HeadState::Branch { name: "main".into() },
        branches: vec![
            Branch { name: "main".into(), current: true, upstream: Some("origin/main".into()), ahead: 2, behind: 0, tip_oid: "a1f3b2c".into() },
            Branch { name: "feat/commit-graph".into(), current: false, upstream: Some("origin/feat/commit-graph".into()), ahead: 7, behind: 3, tip_oid: "9f8d4e6".into() },
            Branch { name: "fix/diff-whitespace".into(), current: false, upstream: Some("origin/fix/diff-whitespace".into()), ahead: 1, behind: 12, tip_oid: "b6c5e29".into() },
            Branch { name: "chore/bump-deps".into(), current: false, upstream: Some("origin/chore/bump-deps".into()), ahead: 0, behind: 5, tip_oid: "d9b4710".into() },
            Branch { name: "release/2.4".into(), current: false, upstream: Some("origin/release/2.4".into()), ahead: 0, behind: 0, tip_oid: "e8d2f90".into() },
        ],
        remotes: vec![
            Remote {
                name: "origin".into(),
                url: "git@github.com:ordo/ordo.git".into(),
                branches: vec![
                    crate::model::repo::RemoteBranch { name: "origin/main".into(),                tip_oid: "a1f3b2c".into() },
                    crate::model::repo::RemoteBranch { name: "origin/feat/commit-graph".into(),   tip_oid: "f4a2c10".into() },
                    crate::model::repo::RemoteBranch { name: "origin/fix/diff-whitespace".into(), tip_oid: "b6c5e29".into() },
                ],
                last_fetched: Some(now),
            },
            Remote {
                name: "upstream".into(),
                url: "git@github.com:ordo-upstream/ordo.git".into(),
                branches: vec![
                    crate::model::repo::RemoteBranch { name: "upstream/main".into(), tip_oid: "a1f3b2c".into() },
                ],
                last_fetched: Some(now),
            },
        ],
        tags: vec![
            Tag { name: "v2.4.0-rc1".into(), oid: "e8d2f90".into(), annotated: true, date: now },
            Tag { name: "v2.3.2".into(),     oid: "7e9d0a1".into(), annotated: true, date: now },
            Tag { name: "v2.3.1".into(),     oid: "c4b8f20".into(), annotated: true, date: now },
            Tag { name: "v2.3.0".into(),     oid: "91ae44d".into(), annotated: true, date: now },
        ],
        stashes: vec![
            Stash { idx: 0, message: "WIP on feat/commit-graph: lane color work".into(), date: now, on_branch: "feat/commit-graph".into() },
            Stash { idx: 1, message: "experimenting with flat toolbar".into(),           date: now, on_branch: "main".into() },
            Stash { idx: 2, message: "debug prints in rebase modal".into(),              date: now, on_branch: "main".into() },
        ],
    };

    let ana   = Author { name: "Ana Petrova".into(),    email: "ana@ordo.dev".into() };
    let miro  = Author { name: "Miro Tanaka".into(),    email: "miro@ordo.dev".into() };
    let yuki  = Author { name: "Yuki Hoffmann".into(),  email: "yuki@ordo.dev".into() };
    let bot   = Author { name: "release-bot".into(),    email: "bot@ordo.dev".into() };

    let commits = vec![
        cmt("a1f3b2c", &["7d2e9a1"],            &ana,  "Graph: soften lane colors in light theme",
            vec![RefChip::Branch { name: "main".into(), current: true }, RefChip::Head,
                 RefChip::Remote { name: "origin/main".into() }], now),
        cmt("7d2e9a1", &["2b4c8f0"],            &ana,  "Inspector: lazy-load diff hunks over 500 lines", vec![], now),
        cmt("2b4c8f0", &["c3e1a22","9f8d4e6"],  &miro, "Merge branch 'feat/commit-graph' into main", vec![], now),
        cmt("9f8d4e6", &["5a7b1c2"],            &ana,  "Add orthogonal routing for merge edges",
            vec![RefChip::Branch { name: "feat/commit-graph".into(), current: false }], now),
        cmt("c3e1a22", &["e8d2f90"],            &miro, "Docs: describe packfile walker heuristic", vec![], now),
        cmt("5a7b1c2", &["e8d2f90"],            &ana,  "Graph: cache lane assignments per viewport", vec![], now),
        cmt("e8d2f90", &["8c9a6b3"],            &yuki, "Keybinding map rewritten around intents, not keys",
            vec![RefChip::Tag { name: "v2.4.0-rc1".into(), annotated: true }], now),
        cmt("8c9a6b3", &["11fa04d","b6c5e29"],  &miro, "Merge branch 'fix/diff-whitespace'", vec![], now),
        cmt("b6c5e29", &["3d0a7c1"],            &yuki, "Diff: ignore trailing whitespace behind a setting",
            vec![RefChip::Branch { name: "fix/diff-whitespace".into(), current: false },
                 RefChip::Remote { name: "origin/fix/diff-whitespace".into() }], now),
        cmt("11fa04d", &["ff1b9e0"],            &ana,  "Status bar: surface detached HEAD state", vec![], now),
        cmt("3d0a7c1", &["ff1b9e0"],            &yuki, "Normalize line endings in test fixtures", vec![], now),
        cmt("ff1b9e0", &["6a3f20d"],            &miro, "Refactor: split Repository::open into open/discover", vec![], now),
        cmt("6a3f20d", &["4e2c88a"],            &ana,  "Remotes: show last fetched time as relative", vec![], now),
        cmt("4e2c88a", &["d9b4710"],            &yuki, "Fix crash when opening empty repository", vec![], now),
        cmt("d9b4710", &["7e9d0a1"],            &miro, "Bump libgit2 to 1.8.1", vec![], now),
        cmt("7e9d0a1", &["c4b8f20"],            &bot,  "Release 2.3.2",
            vec![RefChip::Tag { name: "v2.3.2".into(), annotated: true }], now),
        cmt("c4b8f20", &["91ae44d"],            &ana,  "Diff: word-wrap toggle (off by default)",
            vec![RefChip::Tag { name: "v2.3.1".into(), annotated: true }], now),
        cmt("91ae44d", &["a02bb15"],            &miro, "Release 2.3.0",
            vec![RefChip::Tag { name: "v2.3.0".into(), annotated: true }], now),
        cmt("a02bb15", &["5fe911c"],            &yuki, "Stashes panel: inline rename", vec![], now),
        cmt("5fe911c", &[],                     &ana,  "Build: reproducible release artifacts", vec![], now),
    ];

    let reflog = vec![
        reflog_entry(0,  "a1f3b2c", ReflogAction::Commit,   "Graph: soften lane colors in light theme", now),
        reflog_entry(1,  "7d2e9a1", ReflogAction::Commit,   "Inspector: lazy-load diff hunks over 500 lines", now),
        reflog_entry(2,  "2b4c8f0", ReflogAction::Merge,    "Merge branch 'feat/commit-graph' into main", now),
        reflog_entry(3,  "c3e1a22", ReflogAction::Checkout, "moving from feat/commit-graph to main", now),
        reflog_entry(4,  "9f8d4e6", ReflogAction::Commit,   "Add orthogonal routing for merge edges", now),
        reflog_entry(5,  "5a7b1c2", ReflogAction::Rebase,   "rebase (finish): returning to refs/heads/feat/commit-graph", now),
        reflog_entry(6,  "5a7b1c2", ReflogAction::Rebase,   "rebase (pick): Graph: cache lane assignments per viewport", now),
        reflog_entry(7,  "e8d2f90", ReflogAction::Reset,    "reset: moving to HEAD~3", now),
        reflog_entry(8,  "e8d2f90", ReflogAction::Pull,     "pull origin main: Fast-forward", now),
        reflog_entry(9,  "8c9a6b3", ReflogAction::Commit,   "Merge branch 'fix/diff-whitespace'", now),
        reflog_entry(10, "b6c5e29", ReflogAction::Checkout, "moving from main to fix/diff-whitespace", now),
        reflog_entry(11, "11fa04d", ReflogAction::CherryPick, "cherry-pick: Status bar: surface detached HEAD state", now),
        reflog_entry(12, "ff1b9e0", ReflogAction::Amend,    "Refactor: split Repository::open into open/discover", now),
        reflog_entry(13, "6a3f20d", ReflogAction::Commit,   "Remotes: show last fetched time as relative", now),
        reflog_entry(14, "5fe911c", ReflogAction::Clone,    "clone: from git@github.com:ordo/ordo.git", now),
    ];

    (repo, commits, reflog)
}

fn cmt(
    oid: &str,
    parents: &[&str],
    author: &Author,
    subject: &str,
    refs: Vec<RefChip>,
    date: OffsetDateTime,
) -> Commit {
    Commit {
        oid: format!("{oid}00000"),
        short: oid.to_string(),
        subject: subject.to_string(),
        body: None,
        author: author.clone(),
        committer: author.clone(),
        date,
        parents: parents.iter().map(|p| format!("{p}00000")).collect(),
        refs,
        signed: false,
    }
}

fn reflog_entry(idx: u32, oid: &str, action: ReflogAction, subject: &str, date: OffsetDateTime) -> ReflogEntry {
    ReflogEntry {
        idx,
        oid: format!("{oid}00000"),
        short: oid.to_string(),
        action,
        subject: subject.to_string(),
        date,
    }
}
