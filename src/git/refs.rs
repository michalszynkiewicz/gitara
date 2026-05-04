//! Branches, remotes, tags via `gix`.

use anyhow::Context;
use std::path::Path;
use time::OffsetDateTime;

use crate::model::{
    reflog::ReflogEntry,
    repo::{Branch, HeadState, Remote, RepoView, Stash, Tag},
};

pub fn load_repo(path: &Path) -> anyhow::Result<RepoView> {
    // Walk upwards for a `.git` — so running gitara from a subdirectory
    // (e.g. ./gitara/ inside the gitl repo) still finds the repo root.
    let repo =
        gix::discover(path).with_context(|| format!("discover git repo at {}", path.display()))?;

    // After discovery, the "real" repo path is the work_dir, not the
    // starting path — reflect that in state so write ops target the right
    // directory.
    let repo_path: std::path::PathBuf = repo
        .workdir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| path.to_path_buf());

    let name = repo
        .workdir()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| repo_path.display().to_string());

    let head = head_state(&repo)?;
    let branches = local_branches(&repo)?;
    let remotes = remotes(&repo)?;
    let tags = tags(&repo_path)?;
    // gix doesn't expose stashes; git2 does, via stash_foreach.
    let stashes = load_stashes(&repo_path).unwrap_or_else(|e| {
        tracing::warn!("load_stashes failed: {e:#}");
        Vec::new()
    });

    Ok(RepoView {
        path: repo_path,
        name,
        head,
        branches,
        remotes,
        tags,
        stashes,
    })
}

fn head_state(repo: &gix::Repository) -> anyhow::Result<HeadState> {
    let head = repo.head().context("read HEAD")?;
    match head.kind {
        gix::head::Kind::Symbolic(r) => {
            let name = r.name.shorten().to_string();
            Ok(HeadState::Branch { name })
        }
        gix::head::Kind::Detached { target, .. } => Ok(HeadState::Detached {
            oid: target.to_string(),
        }),
        gix::head::Kind::Unborn(_) => Ok(HeadState::Unborn),
    }
}

fn local_branches(repo: &gix::Repository) -> anyhow::Result<Vec<Branch>> {
    let platform = repo.references().context("open references")?;
    let mut out = Vec::new();
    let head_short = match repo.head_name()? {
        Some(n) => n.shorten().to_string(),
        None => String::new(),
    };

    for r in platform.local_branches()? {
        let mut r = match r {
            Ok(r) => r,
            Err(_) => continue,
        };
        let short = r.name().shorten().to_string();
        let tip = match r.peel_to_id() {
            Ok(id) => id.to_string(),
            Err(_) => continue,
        };
        // Upstream branch + ahead/behind: gix doesn't expose a one-liner for this,
        // and the config-based lookup is tricky. For now, no upstream info.
        out.push(Branch {
            name: short.clone(),
            current: short == head_short,
            upstream: None,
            ahead: 0,
            behind: 0,
            tip_oid: tip,
        });
    }
    Ok(out)
}

fn remotes(repo: &gix::Repository) -> anyhow::Result<Vec<Remote>> {
    use crate::model::repo::RemoteBranch;
    let mut out = Vec::new();
    // remote_names is a BTreeSet-like iterator of Cow<'_, BStr>.
    let names: Vec<String> = repo
        .remote_names()
        .into_iter()
        .map(|n| n.to_string())
        .collect();

    // Build remote-branch lists per remote from references().remote_branches().
    // We resolve each ref to its tip oid so the log walker can use it as a
    // starting point and the graph can render a chip on the right commit.
    let platform = repo.references()?;
    let mut per_remote: std::collections::BTreeMap<String, Vec<RemoteBranch>> = Default::default();
    for r in platform.remote_branches()? {
        let mut r = match r {
            Ok(r) => r,
            Err(_) => continue,
        };
        let full = r.name().shorten().to_string(); // "origin/main"
        let Some((remote, _)) = full.split_once('/') else {
            continue;
        };
        let remote = remote.to_string();
        let oid = match r.peel_to_id() {
            Ok(id) => id.to_string(),
            Err(_) => continue,
        };
        per_remote.entry(remote).or_default().push(RemoteBranch {
            name: full,
            tip_oid: oid,
        });
    }

    for name in names {
        let url = repo
            .find_remote(name.as_str())
            .ok()
            .and_then(|rm| {
                rm.url(gix::remote::Direction::Fetch)
                    .map(|u| u.to_bstring().to_string())
            })
            .unwrap_or_default();
        let branches = per_remote.remove(&name).unwrap_or_default();
        out.push(Remote {
            name,
            url,
            branches,
            last_fetched: None,
        });
    }
    Ok(out)
}

fn tags(repo_path: &Path) -> anyhow::Result<Vec<Tag>> {
    let repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {} for tags", repo_path.display()))?;
    let names = repo.tag_names(None).context("list tags")?;
    let mut out = Vec::new();

    for name in names.iter().flatten() {
        let refname = format!("refs/tags/{name}");
        let r = match repo.find_reference(&refname) {
            Ok(r) => r,
            Err(_) => continue,
        };
        // Peel all the way to a commit to get the target oid shown in the graph.
        let commit = match r.peel(git2::ObjectType::Commit) {
            Ok(obj) => match obj.into_commit() {
                Ok(c) => c,
                Err(_) => continue,
            },
            Err(_) => continue,
        };
        let oid = commit.id().to_string();

        // If the raw reference target differs from the commit id, the ref
        // points at a tag object → annotated.
        let raw_oid = match r.target() {
            Some(id) => id,
            None => continue,
        };
        let (annotated, date) = if raw_oid != commit.id() {
            let date = repo
                .find_tag(raw_oid)
                .ok()
                .and_then(|t| t.tagger().map(|s| git2_time_to_offsetdt(s.when())))
                .unwrap_or_else(|| git2_time_to_offsetdt(commit.committer().when()));
            (true, date)
        } else {
            (false, git2_time_to_offsetdt(commit.committer().when()))
        };

        out.push(Tag { name: name.to_string(), oid, annotated, date });
    }
    Ok(out)
}

fn git2_time_to_offsetdt(t: git2::Time) -> OffsetDateTime {
    let offset = time::UtcOffset::from_whole_seconds(t.offset_minutes() * 60)
        .unwrap_or(time::UtcOffset::UTC);
    OffsetDateTime::from_unix_timestamp(t.seconds())
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .to_offset(offset)
}

/// Walk the stash list for the given repo via git2's stash_foreach
/// (which gix doesn't expose). Newest stash first.
fn load_stashes(repo_path: &Path) -> anyhow::Result<Vec<Stash>> {
    let mut repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {}", repo_path.display()))?;

    let mut out: Vec<Stash> = Vec::new();
    repo.stash_foreach(|idx, message, _oid| {
        // Stash messages look like "WIP on main: 0123abc subject" or
        // "On main: subject". Strip the "WIP on <branch>: " or
        // "On <branch>: " prefix to get a clean message + remember the branch.
        let (on_branch, msg) = parse_stash_message(message);
        out.push(Stash {
            idx: idx as u32,
            message: msg,
            date: OffsetDateTime::now_utc(),
            on_branch,
        });
        true
    })
    .context("walk stash list")?;
    Ok(out)
}

fn parse_stash_message(raw: &str) -> (String, String) {
    let raw = raw.trim();
    if let Some(rest) = raw
        .strip_prefix("WIP on ")
        .or_else(|| raw.strip_prefix("On "))
    {
        if let Some((branch, msg)) = rest.split_once(": ") {
            return (branch.to_string(), msg.to_string());
        }
    }
    (String::new(), raw.to_string())
}

pub fn reflog(repo_path: &Path) -> anyhow::Result<Vec<ReflogEntry>> {
    let repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {}", repo_path.display()))?;
    let log = match repo.reflog("HEAD") {
        Ok(l) => l,
        Err(_) => return Ok(Vec::new()), // no reflog yet
    };

    let mut out: Vec<ReflogEntry> = Vec::with_capacity(log.len());
    for (idx, entry) in log.iter().enumerate() {
        let oid = entry.id_new().to_string();
        let short = oid[..oid.len().min(7)].to_string();
        let raw = entry.message().unwrap_or("").to_string();
        let (action, subject) = parse_reflog_message(&raw);

        // entry.committer().when() gives us a Time {seconds, offset_minutes}.
        let when = entry.committer().when();
        let date = OffsetDateTime::from_unix_timestamp(when.seconds())
            .unwrap_or_else(|_| OffsetDateTime::now_utc());

        out.push(ReflogEntry {
            idx: idx as u32,
            oid,
            short,
            action,
            subject,
            date,
        });
    }
    Ok(out)
}

/// Reflog messages look like "checkout: moving from X to Y" or
/// "commit: subject" or "merge feat: Fast-forward". Map the leading verb
/// to a ReflogAction; the rest becomes the subject.
fn parse_reflog_message(raw: &str) -> (crate::model::reflog::ReflogAction, String) {
    use crate::model::reflog::ReflogAction as A;
    let raw = raw.trim();
    let (head, rest) = raw.split_once(':').unwrap_or((raw, ""));
    let subject = rest.trim().to_string();
    let action = match head.trim() {
        s if s.starts_with("commit (amend)") => A::Amend,
        s if s.starts_with("commit") => A::Commit,
        s if s.starts_with("merge") => A::Merge,
        s if s.starts_with("checkout") => A::Checkout,
        s if s.starts_with("rebase") => A::Rebase,
        s if s.starts_with("reset") => A::Reset,
        s if s.starts_with("pull") => A::Pull,
        s if s.starts_with("push") => A::Push,
        s if s.starts_with("clone") => A::Clone,
        s if s.starts_with("cherry-pick") => A::CherryPick,
        _ => A::Other,
    };
    let subject = if subject.is_empty() {
        raw.to_string()
    } else {
        subject
    };
    (action, subject)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_fixture::{fixture, seed_commits};

    #[test]
    fn stashes_loaded_from_stash_list() {
        use std::fs;
        let repo = fixture("stashes");
        seed_commits(&repo, &["a"]);

        // Create a working-tree change and stash it.
        fs::write(repo.join("0.txt"), "modified").unwrap();
        let mut g = git2::Repository::open(&repo).unwrap();
        let sig = g.signature().unwrap();
        g.stash_save(&sig, "test stash entry", None).unwrap();

        let stashes = load_stashes(&repo).unwrap();
        assert_eq!(stashes.len(), 1);
        assert_eq!(stashes[0].idx, 0);
        assert!(stashes[0].message.contains("test stash entry"));
    }

    #[test]
    fn lightweight_tag_is_not_annotated_and_has_commit_date() {
        let repo = fixture("tag_meta_lightweight");
        seed_commits(&repo, &["a"]);
        crate::git::ops::create_tag(&repo, "v1.0", None, "").unwrap();

        let result = tags(&repo).unwrap();
        let tag = result.iter().find(|t| t.name == "v1.0").unwrap();
        assert!(!tag.annotated, "lightweight tag should not be annotated");
        // Date should be within a few seconds of now (it's the commit date).
        let delta = (OffsetDateTime::now_utc() - tag.date).whole_seconds().abs();
        assert!(delta < 30, "tag date too far from now: {delta}s");
    }

    #[test]
    fn annotated_tag_is_marked_annotated_and_has_tagger_date() {
        let repo = fixture("tag_meta_annotated");
        seed_commits(&repo, &["a"]);
        crate::git::ops::create_tag(&repo, "v2.0", None, "release notes").unwrap();

        let result = tags(&repo).unwrap();
        let tag = result.iter().find(|t| t.name == "v2.0").unwrap();
        assert!(tag.annotated, "annotated tag should be marked annotated");
        let delta = (OffsetDateTime::now_utc() - tag.date).whole_seconds().abs();
        assert!(delta < 30, "tag date too far from now: {delta}s");
    }

    #[test]
    fn reflog_returns_entries_for_seeded_repo() {
        let repo = fixture("reflog_seeded");
        seed_commits(&repo, &["a", "b", "c"]);
        let entries = reflog(&repo).unwrap();
        assert!(
            entries.len() >= 3,
            "expected at least 3 reflog entries, got {}",
            entries.len()
        );
        // First entry (idx 0) is the most recent action — for our seed, the
        // last commit. Either Commit or some other write action.
        assert!(matches!(
            entries[0].action,
            crate::model::reflog::ReflogAction::Commit | crate::model::reflog::ReflogAction::Other
        ));
    }
}
