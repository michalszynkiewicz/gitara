//! Diff plumbing — tree-to-tree for commits, workdir-vs-HEAD for the
//! Changes tab. Everything runs via `git2` for now; we can port to `gix`
//! later if we want pure-Rust throughout.

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::model::diff::{DiffLine, DiffOrigin, FileChange, FileStatus, Hunk};

/// List the files changed in `oid` compared to its first parent. For a
/// root commit (no parents), lists every file as Added.
pub fn files_for_commit(repo_path: &Path, oid: &str) -> anyhow::Result<Vec<FileChange>> {
    let repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {}", repo_path.display()))?;

    let commit = repo
        .find_commit(git2::Oid::from_str(oid).context("parse oid")?)
        .context("find commit")?;
    let new_tree = commit.tree().context("commit tree")?;
    let old_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3);

    let diff = repo
        .diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))
        .context("diff tree to tree")?;

    let stats = diff.stats().context("diff stats")?;
    // Per-file additions/deletions are available via deltas; stats gives
    // repo-wide totals. Walk deltas and attribute via git2::Patch.
    let _ = stats;

    let mut out: Vec<FileChange> = Vec::new();
    for delta_idx in 0..diff.deltas().len() {
        let delta = diff.get_delta(delta_idx).context("delta")?;
        let (additions, deletions) = per_file_stats(&repo, &diff, delta_idx);
        out.push(file_change_from_delta(&delta, additions, deletions));
    }
    Ok(out)
}

/// Return every hunk in the commit, optionally filtered to the given file
/// path. If `only_path` is `None`, all files are included.
pub fn hunks_for_commit(
    repo_path: &Path,
    oid: &str,
    only_path: Option<&Path>,
) -> anyhow::Result<Vec<(PathBuf, Hunk)>> {
    let repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {}", repo_path.display()))?;
    let commit = repo
        .find_commit(git2::Oid::from_str(oid).context("parse oid")?)
        .context("find commit")?;
    let new_tree = commit.tree().context("commit tree")?;
    let old_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3);
    if let Some(p) = only_path {
        opts.pathspec(p);
    }

    let diff = repo
        .diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut opts))
        .context("diff tree to tree")?;

    collect_hunks(&diff)
}

/// Files in the index that differ from HEAD — i.e. what `git commit` would
/// commit. Powers the Commit modal's staged-list.
pub fn staged_files(repo_path: &Path) -> anyhow::Result<Vec<FileChange>> {
    let repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {}", repo_path.display()))?;

    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let index = repo.index().context("open index")?;

    let mut opts = git2::DiffOptions::new();
    let diff = repo
        .diff_tree_to_index(head_tree.as_ref(), Some(&index), Some(&mut opts))
        .context("diff tree to index")?;

    let mut out: Vec<FileChange> = Vec::new();
    for delta_idx in 0..diff.deltas().len() {
        let delta = diff.get_delta(delta_idx).context("delta")?;
        let (additions, deletions) = per_file_stats(&repo, &diff, delta_idx);
        let mut fc = file_change_from_delta(&delta, additions, deletions);
        fc.staged = true;
        out.push(fc);
    }
    Ok(out)
}

/// Files in the working tree that differ from the index — i.e. what
/// `git diff` (without --cached) would show. Powers the Commit modal's
/// "Unstaged" list.
pub fn unstaged_files(repo_path: &Path) -> anyhow::Result<Vec<FileChange>> {
    let repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {}", repo_path.display()))?;

    let mut opts = git2::DiffOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);

    let diff = repo
        .diff_index_to_workdir(None, Some(&mut opts))
        .context("diff index to workdir")?;

    let mut out: Vec<FileChange> = Vec::new();
    for delta_idx in 0..diff.deltas().len() {
        let delta = diff.get_delta(delta_idx).context("delta")?;
        let (additions, deletions) = per_file_stats(&repo, &diff, delta_idx);
        out.push(file_change_from_delta(&delta, additions, deletions));
    }
    Ok(out)
}

/// Hunks for a single file in the index (staged side). Used by the
/// Commit modal when a staged file is selected.
pub fn hunks_staged_for_path(
    repo_path: &Path,
    path: &Path,
) -> anyhow::Result<Vec<(PathBuf, Hunk)>> {
    let repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {}", repo_path.display()))?;
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let index = repo.index().context("open index")?;

    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3);
    opts.pathspec(path);

    let diff = repo
        .diff_tree_to_index(head_tree.as_ref(), Some(&index), Some(&mut opts))
        .context("diff tree to index")?;

    collect_hunks(&diff)
}

/// Hunks for a single file's unstaged (worktree-vs-index) changes.
/// Used by the Commit modal when an unstaged file is selected.
pub fn hunks_unstaged_for_path(
    repo_path: &Path,
    path: &Path,
) -> anyhow::Result<Vec<(PathBuf, Hunk)>> {
    let repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {}", repo_path.display()))?;

    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3);
    opts.include_untracked(true).recurse_untracked_dirs(true);
    opts.pathspec(path);

    let diff = repo
        .diff_index_to_workdir(None, Some(&mut opts))
        .context("diff index to workdir")?;

    collect_hunks(&diff)
}

/// Hunks for the working tree vs HEAD (incl. untracked). Powers the Diff
/// tab when the working-tree pseudo-row is selected.
pub fn hunks_for_dirty_tree(repo_path: &Path) -> anyhow::Result<Vec<(PathBuf, Hunk)>> {
    let repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {}", repo_path.display()))?;

    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());

    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3);
    opts.include_untracked(true).recurse_untracked_dirs(true);

    let diff = repo
        .diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))
        .context("diff tree to workdir")?;

    collect_hunks(&diff)
}

/// Working-tree vs HEAD, for the inspector's Changes tab.
pub fn dirty_tree(repo_path: &Path) -> anyhow::Result<Vec<FileChange>> {
    let repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {}", repo_path.display()))?;

    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());

    let mut opts = git2::DiffOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);

    let diff = repo
        .diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))
        .context("diff tree to workdir")?;

    let mut out: Vec<FileChange> = Vec::new();
    for delta_idx in 0..diff.deltas().len() {
        let delta = diff.get_delta(delta_idx).context("delta")?;
        let (additions, deletions) = per_file_stats(&repo, &diff, delta_idx);
        out.push(file_change_from_delta(&delta, additions, deletions));
    }
    Ok(out)
}

// ── helpers ─────────────────────────────────────────────────────────────

fn file_change_from_delta(
    delta: &git2::DiffDelta<'_>,
    additions: u32,
    deletions: u32,
) -> FileChange {
    let status = match delta.status() {
        git2::Delta::Added | git2::Delta::Copied => FileStatus::Added,
        git2::Delta::Deleted => FileStatus::Deleted,
        git2::Delta::Modified => FileStatus::Modified,
        git2::Delta::Renamed => FileStatus::Renamed,
        git2::Delta::Typechange => FileStatus::TypeChange,
        git2::Delta::Conflicted => FileStatus::Conflicted,
        git2::Delta::Untracked => FileStatus::Untracked,
        _ => FileStatus::Modified,
    };
    let path = delta
        .new_file()
        .path()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let old_path = delta
        .old_file()
        .path()
        .map(|p| p.to_path_buf())
        .filter(|op| op != &path);

    FileChange {
        path,
        old_path,
        status,
        staged: false,
        additions,
        deletions,
    }
}

fn per_file_stats(repo: &git2::Repository, diff: &git2::Diff<'_>, idx: usize) -> (u32, u32) {
    match git2::Patch::from_diff(diff, idx) {
        Ok(Some(patch)) => match patch.line_stats() {
            Ok((_context, additions, deletions)) => (additions as u32, deletions as u32),
            Err(_) => (0, 0),
        },
        _ => {
            let _ = repo;
            (0, 0)
        }
    }
}

/// Walk every delta in `diff` and collect hunks with their lines.
fn collect_hunks(diff: &git2::Diff<'_>) -> anyhow::Result<Vec<(PathBuf, Hunk)>> {
    let mut out: Vec<(PathBuf, Hunk)> = Vec::new();
    for delta_idx in 0..diff.deltas().len() {
        let patch = match git2::Patch::from_diff(diff, delta_idx).context("build patch")? {
            Some(p) => p,
            None => continue,
        };
        let path = patch
            .delta()
            .new_file()
            .path()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();

        for h in 0..patch.num_hunks() {
            let (raw, line_count) = patch.hunk(h).context("hunk")?;
            let mut hunk = Hunk {
                old_start: raw.old_start(),
                old_len: raw.old_lines(),
                new_start: raw.new_start(),
                new_len: raw.new_lines(),
                header: String::from_utf8_lossy(raw.header()).trim_end().to_string(),
                lines: Vec::with_capacity(line_count),
            };
            for l in 0..line_count {
                let line = patch.line_in_hunk(h, l).context("line")?;
                let origin = match line.origin() {
                    '+' => DiffOrigin::Added,
                    '-' => DiffOrigin::Removed,
                    _ => DiffOrigin::Context,
                };
                hunk.lines.push(DiffLine {
                    origin,
                    old_lineno: line.old_lineno(),
                    new_lineno: line.new_lineno(),
                    content: String::from_utf8_lossy(line.content())
                        .trim_end_matches('\n')
                        .to_string(),
                });
            }
            out.push((path.clone(), hunk));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_fixture::{fixture, seed_commits};
    use std::fs;

    fn head_oid(path: &Path) -> String {
        git2::Repository::open(path)
            .unwrap()
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id()
            .to_string()
    }

    #[test]
    fn files_for_commit_root() {
        let repo = fixture("diff_files_root");
        seed_commits(&repo, &["first"]);
        let oid = head_oid(&repo);
        let files = files_for_commit(&repo, &oid).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("0.txt"));
        assert!(matches!(files[0].status, FileStatus::Added));
    }

    #[test]
    fn files_for_commit_modified() {
        let repo = fixture("diff_files_mod");
        seed_commits(&repo, &["a"]);
        fs::write(repo.join("0.txt"), "replaced\nmore\n").unwrap();

        let r = git2::Repository::open(&repo).unwrap();
        let mut idx = r.index().unwrap();
        idx.add_path(Path::new("0.txt")).unwrap();
        idx.write().unwrap();
        let tree_id = idx.write_tree().unwrap();
        let tree = r.find_tree(tree_id).unwrap();
        let sig = r.signature().unwrap();
        let parent = r.head().unwrap().peel_to_commit().unwrap();
        r.commit(Some("HEAD"), &sig, &sig, "mod", &tree, &[&parent])
            .unwrap();

        let files = files_for_commit(&repo, &head_oid(&repo)).unwrap();
        assert_eq!(files.len(), 1);
        assert!(matches!(files[0].status, FileStatus::Modified));
        assert!(files[0].additions >= 1);
    }

    #[test]
    fn hunks_for_commit_returns_diff_lines() {
        let repo = fixture("diff_hunks");
        seed_commits(&repo, &["first"]);
        // Modify the file and commit.
        fs::write(repo.join("0.txt"), "first\nadded line\n").unwrap();
        let r = git2::Repository::open(&repo).unwrap();
        let mut idx = r.index().unwrap();
        idx.add_path(Path::new("0.txt")).unwrap();
        idx.write().unwrap();
        let tree_id = idx.write_tree().unwrap();
        let tree = r.find_tree(tree_id).unwrap();
        let sig = r.signature().unwrap();
        let parent = r.head().unwrap().peel_to_commit().unwrap();
        r.commit(Some("HEAD"), &sig, &sig, "mod", &tree, &[&parent])
            .unwrap();

        let hunks = hunks_for_commit(&repo, &head_oid(&repo), None).unwrap();
        assert!(!hunks.is_empty());
        assert!(hunks.iter().any(|(_, h)| h
            .lines
            .iter()
            .any(|l| matches!(l.origin, DiffOrigin::Added))));
    }

    #[test]
    fn staged_files_lists_only_index_changes() {
        let repo = fixture("diff_staged");
        seed_commits(&repo, &["a"]);

        // Modify 0.txt in the working tree (not staged).
        fs::write(repo.join("0.txt"), "modified-not-staged").unwrap();
        // Add a new file and stage it.
        fs::write(repo.join("new.txt"), "staged content").unwrap();
        let g = git2::Repository::open(&repo).unwrap();
        let mut idx = g.index().unwrap();
        idx.add_path(Path::new("new.txt")).unwrap();
        idx.write().unwrap();

        let staged = staged_files(&repo).unwrap();
        assert_eq!(staged.len(), 1, "expected just new.txt staged");
        assert_eq!(staged[0].path, PathBuf::from("new.txt"));
        assert!(matches!(staged[0].status, FileStatus::Added));
        assert!(staged[0].staged);
    }

    #[test]
    fn unstaged_files_excludes_staged_changes() {
        let repo = fixture("diff_unstaged");
        seed_commits(&repo, &["a"]);

        // Staged: new.txt added to the index.
        fs::write(repo.join("new.txt"), "staged content").unwrap();
        let g = git2::Repository::open(&repo).unwrap();
        let mut idx = g.index().unwrap();
        idx.add_path(Path::new("new.txt")).unwrap();
        idx.write().unwrap();

        // Unstaged: 0.txt modified in the working tree only.
        fs::write(repo.join("0.txt"), "modified").unwrap();

        let unstaged = unstaged_files(&repo).unwrap();
        // new.txt should NOT appear (it's fully staged); 0.txt should.
        assert!(unstaged.iter().any(|f| f.path == Path::new("0.txt")));
        assert!(!unstaged.iter().any(|f| f.path == Path::new("new.txt")));
    }

    #[test]
    fn hunks_unstaged_for_path_returns_only_workdir_diff() {
        let repo = fixture("diff_hunks_unstaged");
        seed_commits(&repo, &["a"]);
        fs::write(repo.join("0.txt"), "first\nadded by hand\n").unwrap();
        let hunks = hunks_unstaged_for_path(&repo, Path::new("0.txt")).unwrap();
        assert!(!hunks.is_empty());
        assert!(hunks.iter().any(|(_, h)| h
            .lines
            .iter()
            .any(|l| matches!(l.origin, DiffOrigin::Added))));
    }

    #[test]
    fn dirty_tree_shows_modified_and_untracked() {
        let repo = fixture("diff_dirty");
        seed_commits(&repo, &["a"]);
        fs::write(repo.join("0.txt"), "modified").unwrap();
        fs::write(repo.join("new.txt"), "untracked").unwrap();

        let changes = dirty_tree(&repo).unwrap();
        let mods: Vec<_> = changes
            .iter()
            .filter(|c| matches!(c.status, FileStatus::Modified))
            .collect();
        let news: Vec<_> = changes
            .iter()
            .filter(|c| matches!(c.status, FileStatus::Untracked))
            .collect();
        assert_eq!(mods.len(), 1);
        assert_eq!(news.len(), 1);
    }
}
