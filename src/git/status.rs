//! Working-tree status — summary counts for the uncommitted-changes
//! indicator (pseudo-row in the history list, inspector Changes tab).

use std::path::Path;

use anyhow::Context;

#[derive(Clone, Debug, Default)]
pub struct WorkingStatus {
    /// Staged changes — present in the index but not yet committed.
    pub staged: u32,
    /// Modified / deleted / renamed working-tree changes not in the index.
    pub modified: u32,
    /// Untracked files (not ignored).
    pub untracked: u32,
    /// Files with unresolved merge conflicts.
    pub conflicted: u32,
}

impl WorkingStatus {
    pub fn is_dirty(&self) -> bool {
        self.staged > 0 || self.modified > 0 || self.untracked > 0 || self.conflicted > 0
    }

    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.conflicted > 0 {
            parts.push(format!("{} conflicted", self.conflicted));
        }
        if self.staged > 0 {
            parts.push(format!("{} staged", self.staged));
        }
        if self.modified > 0 {
            parts.push(format!("{} modified", self.modified));
        }
        if self.untracked > 0 {
            parts.push(format!("{} untracked", self.untracked));
        }
        if parts.is_empty() {
            String::from("clean")
        } else {
            parts.join(" · ")
        }
    }
}

pub fn read(repo_path: &Path) -> anyhow::Result<WorkingStatus> {
    let repo = git2::Repository::open(repo_path)
        .with_context(|| format!("open {}", repo_path.display()))?;

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true).include_ignored(false);

    let statuses = repo.statuses(Some(&mut opts)).context("read status")?;

    let mut out = WorkingStatus::default();
    for s in statuses.iter() {
        let st = s.status();
        if st.is_conflicted() {
            out.conflicted += 1;
            continue;
        }
        if st.is_wt_new() {
            out.untracked += 1;
            continue;
        }
        // Staged (index side) ≠ 0 means something is prepared for commit.
        const INDEX_DIRTY: git2::Status = git2::Status::INDEX_NEW
            .union(git2::Status::INDEX_MODIFIED)
            .union(git2::Status::INDEX_DELETED)
            .union(git2::Status::INDEX_RENAMED)
            .union(git2::Status::INDEX_TYPECHANGE);
        const WT_DIRTY: git2::Status = git2::Status::WT_MODIFIED
            .union(git2::Status::WT_DELETED)
            .union(git2::Status::WT_RENAMED)
            .union(git2::Status::WT_TYPECHANGE);
        if st.intersects(INDEX_DIRTY) {
            out.staged += 1;
        }
        if st.intersects(WT_DIRTY) {
            out.modified += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_fixture::{fixture, seed_commits};
    use std::fs;

    #[test]
    fn clean_tree() {
        let repo = fixture("status_clean");
        seed_commits(&repo, &["a"]);
        let s = read(&repo).unwrap();
        assert!(!s.is_dirty());
        assert_eq!(s.summary(), "clean");
    }

    #[test]
    fn untracked_file_counted() {
        let repo = fixture("status_untracked");
        seed_commits(&repo, &["a"]);
        fs::write(repo.join("new.txt"), "hi").unwrap();
        let s = read(&repo).unwrap();
        assert_eq!(s.untracked, 1);
        assert_eq!(s.modified, 0);
        assert_eq!(s.staged, 0);
        assert!(s.is_dirty());
    }

    #[test]
    fn modified_and_staged_counted_separately() {
        let repo = fixture("status_mod_stage");
        seed_commits(&repo, &["a", "b"]);

        // Modify 0.txt in the working tree (not staged).
        fs::write(repo.join("0.txt"), "modified").unwrap();
        // Modify 1.txt and stage it.
        fs::write(repo.join("1.txt"), "staged-too").unwrap();
        let g = git2::Repository::open(&repo).unwrap();
        let mut idx = g.index().unwrap();
        idx.add_path(Path::new("1.txt")).unwrap();
        idx.write().unwrap();

        let s = read(&repo).unwrap();
        assert_eq!(s.staged, 1);
        assert_eq!(s.modified, 1);
    }
}
