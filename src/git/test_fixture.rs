//! Helpers for creating disposable git repos in tests.
//!
//! Each `fixture(name)` call creates a fresh repo at
//! `$TMPDIR/gitara-test-<name>-<pid>` with an identity configured. Tests
//! seed commits via `seed_commits` then exercise `git::ops::*` against it.
//! The old directory is removed up-front so re-running tests is idempotent.

#![cfg(test)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

pub fn fixture(name: &str) -> PathBuf {
    let tmp = std::env::temp_dir();
    let path = tmp.join(format!("gitara-test-{name}-{}", process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();

    let repo = git2::Repository::init(&path).expect("init");
    // Identity — required for commits.
    let mut cfg = repo.config().unwrap();
    cfg.set_str("user.name", "Test").unwrap();
    cfg.set_str("user.email", "test@gitara.test").unwrap();
    // Disable signing — if the user has commit.gpgsign=true globally, our
    // shell-out commits would otherwise prompt or fail in CI.
    cfg.set_bool("commit.gpgsign", false).unwrap();
    cfg.set_bool("tag.gpgsign", false).unwrap();
    path
}

/// Create a linear chain of commits with the given subjects, each adding a
/// single file `n.txt` (content = subject). Updates HEAD / master.
pub fn seed_commits(repo_path: &Path, subjects: &[&str]) {
    let repo = git2::Repository::open(repo_path).unwrap();
    let sig = repo.signature().unwrap();

    for (i, subject) in subjects.iter().enumerate() {
        let file = repo_path.join(format!("{i}.txt"));
        fs::File::create(&file).unwrap().write_all(subject.as_bytes()).unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new(&format!("{i}.txt"))).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let parents: Vec<git2::Commit> = match repo.head().ok().and_then(|h| h.peel_to_commit().ok())
        {
            Some(c) => vec![c],
            None => vec![],
        };
        let parents_ref: Vec<&git2::Commit> = parents.iter().collect();

        repo.commit(Some("HEAD"), &sig, &sig, subject, &tree, &parents_ref)
            .unwrap();
    }
}
