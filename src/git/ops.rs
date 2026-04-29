//! Writes and network ops — these shell out to `git` rather than using
//! `git2` directly. Why:
//!   * the user's git config (gpg.signingkey, commit.gpgsign,
//!     core.hooksPath, includeIf, credential helpers, signing keys)
//!     just works — we don't have to reimplement any of it.
//!   * pre-commit / pre-push hooks fire as the user expects.
//!   * SSH agent / smart-card auth is git's problem.
//!   * the operation is byte-for-byte what the user would type.
//!   * we never have to chase a libgit2 bug for a write op.
//!
//! Reads (log, refs, diff, status) stay on the libraries — they're
//! independent of user config and we already pay for parsing the object
//! database in-process.

use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};

use anyhow::Context;

/// Run `git <args>` in `repo_path`. Returns stdout on success; bails with
/// stderr (or stdout if stderr is empty) on non-zero exit.
fn git<I, S>(repo_path: &Path, args: I) -> anyhow::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args: Vec<_> = args.into_iter().collect();
    let argv: Vec<String> = args
        .iter()
        .map(|s| s.as_ref().to_string_lossy().into_owned())
        .collect();

    let output: Output = Command::new("git")
        .current_dir(repo_path)
        .args(&args)
        .output()
        .with_context(|| format!("spawn `git {}` in {}", argv.join(" "), repo_path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let msg = if !stderr.is_empty() { stderr } else { stdout };
        anyhow::bail!("git {}: {msg}", argv.join(" "));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

// ── operations ──────────────────────────────────────────────────────────

/// Create a new local branch. `from` is any rev-spec git accepts; defaults
/// to HEAD. Returns the resolved tip oid.
pub fn create_branch(
    repo_path: &Path,
    name: &str,
    from: Option<&str>,
    checkout: bool,
) -> anyhow::Result<String> {
    if name.trim().is_empty() {
        anyhow::bail!("branch name is empty");
    }
    // `--end-of-options` (git 2.24+) tells git's argument parser that
    // every following arg is positional, even if it starts with `-`.
    // Without it a hostile branch / tag / ref name could be reinterpreted
    // as a flag (CVE-2017-1000117 class).
    let mut args: Vec<&str> = vec!["branch", "--end-of-options", name];
    if let Some(rev) = from {
        args.push(rev);
    }
    git(repo_path, &args)?;
    if checkout {
        git(repo_path, &["checkout", "--end-of-options", name])?;
    }
    // rev-parse arg is `refs/heads/<name>`, prefixed with literal text
    // so the resolved arg never starts with `-` regardless of `name`.
    let oid = git(repo_path, &["rev-parse", &format!("refs/heads/{name}")])?;
    Ok(oid.trim().to_string())
}

/// `git branch -d <name>` (safe) or `-D` (force). Safe delete refuses
/// when the branch isn't fully merged into HEAD; force deletes anyway.
pub fn delete_branch(repo_path: &Path, name: &str, force: bool) -> anyhow::Result<()> {
    if name.trim().is_empty() {
        anyhow::bail!("branch name is empty");
    }
    let flag = if force { "-D" } else { "-d" };
    git(repo_path, &["branch", flag, "--end-of-options", name])?;
    Ok(())
}

/// `git branch -m <old> <new>`.
pub fn rename_branch(repo_path: &Path, old: &str, new: &str) -> anyhow::Result<()> {
    if old.trim().is_empty() || new.trim().is_empty() {
        anyhow::bail!("rename: empty name");
    }
    git(repo_path, &["branch", "-m", "--end-of-options", old, new])?;
    Ok(())
}

/// `git remote remove <name>`.
pub fn remove_remote(repo_path: &Path, name: &str) -> anyhow::Result<()> {
    if name.trim().is_empty() {
        anyhow::bail!("remote name is empty");
    }
    git(repo_path, &["remote", "remove", "--end-of-options", name])?;
    Ok(())
}

/// `git tag -d <name>`.
pub fn delete_tag(repo_path: &Path, name: &str) -> anyhow::Result<()> {
    if name.trim().is_empty() {
        anyhow::bail!("tag name is empty");
    }
    git(repo_path, &["tag", "-d", "--end-of-options", name])?;
    Ok(())
}

/// Create a tag. When `message` is non-empty, creates an annotated tag
/// (`git tag -a -m`) — that goes through the user's signing/hooks/config
/// just like a commit. When empty, creates a lightweight tag (just a ref
/// pointing to the commit). `oid` defaults to HEAD.
pub fn create_tag(
    repo_path: &Path,
    name: &str,
    oid: Option<&str>,
    message: &str,
) -> anyhow::Result<()> {
    if name.trim().is_empty() {
        anyhow::bail!("tag name is empty");
    }
    let mut args: Vec<&str> = vec!["tag"];
    if !message.trim().is_empty() {
        args.push("-a");
        args.push("-m");
        args.push(message);
    }
    args.push("--end-of-options");
    args.push(name);
    if let Some(rev) = oid {
        args.push(rev);
    }
    git(repo_path, &args)?;
    Ok(())
}

/// `git stash apply stash@{idx}` — applies the stash but keeps it.
pub fn stash_apply(repo_path: &Path, idx: u32) -> anyhow::Result<()> {
    git(repo_path, &["stash", "apply", &format!("stash@{{{idx}}}")])?;
    Ok(())
}

/// `git stash pop stash@{idx}` — applies and drops on success.
pub fn stash_pop(repo_path: &Path, idx: u32) -> anyhow::Result<()> {
    git(repo_path, &["stash", "pop", &format!("stash@{{{idx}}}")])?;
    Ok(())
}

/// `git stash drop stash@{idx}`.
pub fn stash_drop(repo_path: &Path, idx: u32) -> anyhow::Result<()> {
    git(repo_path, &["stash", "drop", &format!("stash@{{{idx}}}")])?;
    Ok(())
}

/// Check out a branch, ref, or commit. Defers to git's own logic about
/// whether the working tree is too dirty to switch — when git refuses,
/// the error surfaces with its full stderr.
pub fn checkout(repo_path: &Path, refspec: &str) -> anyhow::Result<()> {
    if refspec.trim().is_empty() {
        anyhow::bail!("checkout target is empty");
    }
    git(repo_path, &["checkout", "--end-of-options", refspec])?;
    Ok(())
}

/// Stage a list of paths.
#[allow(dead_code)] // exposed for future Changes-tab actions.
pub fn stage(repo_path: &Path, paths: &[&Path]) -> anyhow::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut args: Vec<&OsStr> = vec![OsStr::new("add"), OsStr::new("--")];
    args.extend(paths.iter().map(|p| p.as_os_str()));
    git(repo_path, &args)?;
    Ok(())
}

/// Unstage (reset HEAD --) a list of paths.
#[allow(dead_code)] // exposed for future Changes-tab actions.
pub fn unstage(repo_path: &Path, paths: &[&Path]) -> anyhow::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut args: Vec<&OsStr> = vec![OsStr::new("reset"), OsStr::new("HEAD"), OsStr::new("--")];
    args.extend(paths.iter().map(|p| p.as_os_str()));
    git(repo_path, &args)?;
    Ok(())
}

/// Discard working-tree changes for the given paths. Destructive — the
/// caller should confirm.
#[allow(dead_code)] // exposed for future Changes-tab actions.
pub fn discard(repo_path: &Path, paths: &[&Path]) -> anyhow::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut args: Vec<&OsStr> = vec![OsStr::new("checkout"), OsStr::new("--")];
    args.extend(paths.iter().map(|p| p.as_os_str()));
    git(repo_path, &args)?;
    Ok(())
}

/// Create a commit on HEAD. `message` is passed via -m. When `amend`,
/// rewrites the previous commit. User's signing/hooks/config apply.
pub fn commit(repo_path: &Path, message: &str, amend: bool) -> anyhow::Result<String> {
    if message.trim().is_empty() && !amend {
        anyhow::bail!("commit message is empty");
    }
    let mut args: Vec<&str> = vec!["commit", "-m", message];
    if amend {
        args.push("--amend");
    }
    git(repo_path, &args)?;
    let oid = git(repo_path, &["rev-parse", "HEAD"])?;
    Ok(oid.trim().to_string())
}

pub fn fetch(repo_path: &Path, remote: &str, prune: bool) -> anyhow::Result<()> {
    let mut args: Vec<&str> = vec!["fetch"];
    if prune {
        args.push("--prune");
    }
    args.push("--end-of-options");
    if !remote.is_empty() {
        args.push(remote);
    }
    git(repo_path, &args)?;
    Ok(())
}

/// Build the refspec arg for [`push`]. Pulled out so it can be unit
/// tested at the string level without needing a real git repo.
///
/// Returns `None` when `local` is empty (caller should treat this as
/// "no branch to push" and surface an error). Otherwise returns the
/// shorter `<local>` form if `target` is empty or matches `local`,
/// else the explicit `<local>:<target>` refspec.
fn build_push_refspec(local: &str, target: &str) -> Option<String> {
    if local.is_empty() {
        return None;
    }
    if !target.is_empty() && target != local {
        Some(format!("{local}:{target}"))
    } else {
        Some(local.to_string())
    }
}

/// `git push [--force-with-lease] <remote> <local>[:<target>]`.
///
/// `local` is the branch being pushed (typically the current branch).
/// `target` is the remote branch name to push *to*; when empty or
/// equal to `local`, the simpler `<remote> <local>` form is used (git
/// then pushes to a same-named remote branch). When different, gitara
/// builds the explicit `<local>:<target>` refspec.
pub fn push(
    repo_path: &Path,
    remote: &str,
    local: &str,
    target: &str,
    force_with_lease: bool,
) -> anyhow::Result<()> {
    let mut args: Vec<&str> = vec!["push"];
    if force_with_lease {
        args.push("--force-with-lease");
    }
    args.push("--end-of-options");
    if !remote.is_empty() {
        args.push(remote);
    }
    let refspec = build_push_refspec(local, target);
    if let Some(rs) = refspec.as_deref() {
        args.push(rs);
    }
    git(repo_path, &args)?;
    Ok(())
}

pub fn pull(repo_path: &Path) -> anyhow::Result<()> {
    git(repo_path, &["pull"])?;
    Ok(())
}

/// `git merge [--no-ff] <from>`. Centralised so the
/// `--end-of-options` separator (against argument-injection) stays in
/// one place — modals call this rather than building their own
/// `Command::args(...)`.
pub fn merge(repo_path: &Path, from: &str, no_ff: bool) -> anyhow::Result<()> {
    let mut args: Vec<&str> = vec!["merge"];
    if no_ff {
        args.push("--no-ff");
    }
    args.push("--end-of-options");
    args.push(from);
    git(repo_path, &args)?;
    Ok(())
}

pub fn rebase(repo_path: &Path, onto: &str) -> anyhow::Result<()> {
    git(repo_path, &["rebase", "--end-of-options", onto])?;
    Ok(())
}

pub fn add_remote(repo_path: &Path, name: &str, url: &str) -> anyhow::Result<()> {
    if name.trim().is_empty() {
        anyhow::bail!("remote name is empty");
    }
    if url.trim().is_empty() {
        anyhow::bail!("remote URL is empty");
    }
    // NOTE: `--end-of-options` only protects git's own arg parser. URLs
    // like `-oProxyCommand=...` are still interpreted by the underlying
    // transport (ssh/curl). The UI entry point is disabled in mod.rs
    // until URL scheme allow-listing lands — see ISSUES.md.
    git(repo_path, &["remote", "add", "--end-of-options", name, url])?;
    Ok(())
}

/// `git reset {--soft|--mixed|--hard} <oid>`.
pub fn reset(repo_path: &Path, oid: &str, mode: crate::app::ResetMode) -> anyhow::Result<()> {
    if oid.trim().is_empty() {
        anyhow::bail!("reset target is empty");
    }
    let flag = match mode {
        crate::app::ResetMode::Soft => "--soft",
        crate::app::ResetMode::Mixed => "--mixed",
        crate::app::ResetMode::Hard => "--hard",
    };
    git(repo_path, &["reset", flag, "--end-of-options", oid])?;
    Ok(())
}

/// `git cherry-pick [--no-commit] <oids>`. Centralised so the
/// `--end-of-options` separator stays in one place.
pub fn cherry_pick(repo_path: &Path, oids: &[&str], no_commit: bool) -> anyhow::Result<()> {
    if oids.is_empty() {
        return Ok(());
    }
    let mut args: Vec<&str> = vec!["cherry-pick"];
    if no_commit {
        args.push("--no-commit");
    }
    args.push("--end-of-options");
    args.extend_from_slice(oids);
    git(repo_path, &args)?;
    Ok(())
}

// ── tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_fixture::{fixture, seed_commits};

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
    fn create_branch_from_head() {
        let repo = fixture("create_branch_head");
        seed_commits(&repo, &["first", "second"]);
        let oid = create_branch(&repo, "feature/x", None, false).unwrap();
        assert_eq!(oid, head_oid(&repo));
    }

    #[test]
    fn create_branch_from_explicit_rev() {
        let repo = fixture("create_branch_rev");
        seed_commits(&repo, &["a", "b", "c"]);

        // Use HEAD~1 — git resolves it via shell-out.
        let oid = create_branch(&repo, "back", Some("HEAD~1"), false).unwrap();
        let g = git2::Repository::open(&repo).unwrap();
        let parent = g
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .parent(0)
            .unwrap();
        assert_eq!(oid, parent.id().to_string());
    }

    #[test]
    fn create_branch_duplicate_fails() {
        let repo = fixture("create_branch_dup");
        seed_commits(&repo, &["only"]);
        create_branch(&repo, "dup", None, false).unwrap();
        let err = create_branch(&repo, "dup", None, false).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("dup") || msg.contains("exists"),
            "unexpected err: {msg}"
        );
    }

    #[test]
    fn create_branch_empty_name_fails() {
        let repo = fixture("create_branch_empty");
        seed_commits(&repo, &["only"]);
        let err = create_branch(&repo, "   ", None, false).unwrap_err();
        assert!(format!("{err:#}").contains("empty"));
    }

    #[test]
    fn create_branch_checkout_moves_head() {
        let repo = fixture("create_branch_checkout");
        seed_commits(&repo, &["x"]);
        create_branch(&repo, "feature/checkout", None, true).unwrap();
        let g = git2::Repository::open(&repo).unwrap();
        assert_eq!(g.head().unwrap().shorthand().unwrap(), "feature/checkout");
    }

    #[test]
    fn checkout_branch_moves_head() {
        let repo = fixture("checkout_branch");
        seed_commits(&repo, &["a", "b"]);
        create_branch(&repo, "side", None, false).unwrap();
        checkout(&repo, "side").unwrap();
        let g = git2::Repository::open(&repo).unwrap();
        assert_eq!(g.head().unwrap().shorthand().unwrap(), "side");
    }

    #[test]
    fn checkout_unknown_ref_fails() {
        let repo = fixture("checkout_unknown");
        seed_commits(&repo, &["only"]);
        let err = checkout(&repo, "nope-never-existed").unwrap_err();
        // git's stderr surfaces — verifies we're actually shelling out.
        let msg = format!("{err:#}");
        assert!(msg.contains("nope-never-existed") || msg.contains("did not match"));
    }

    #[test]
    fn commit_creates_new_oid() {
        use std::fs;
        let repo = fixture("commit_works");
        seed_commits(&repo, &["a"]);
        let before = head_oid(&repo);

        // Stage a new file then commit via the op.
        fs::write(repo.join("new.txt"), "hello").unwrap();
        stage(&repo, &[Path::new("new.txt")]).unwrap();
        let new_oid = commit(&repo, "second", false).unwrap();

        assert_ne!(new_oid, before);
        assert_eq!(new_oid, head_oid(&repo));
    }

    #[test]
    fn commit_empty_message_fails() {
        let repo = fixture("commit_empty");
        seed_commits(&repo, &["a"]);
        let err = commit(&repo, "   ", false).unwrap_err();
        assert!(format!("{err:#}").contains("empty"));
    }

    #[test]
    fn create_lightweight_tag_at_head() {
        let repo = fixture("tag_lightweight");
        seed_commits(&repo, &["a", "b"]);
        create_tag(&repo, "v0.1.0", None, "").unwrap();
        let g = git2::Repository::open(&repo).unwrap();
        let r = g.find_reference("refs/tags/v0.1.0").unwrap();
        assert_eq!(r.target().unwrap().to_string(), head_oid(&repo));
    }

    #[test]
    fn create_annotated_tag_at_specific_commit() {
        let repo = fixture("tag_annotated");
        seed_commits(&repo, &["a", "b", "c"]);
        let g = git2::Repository::open(&repo).unwrap();
        let parent_oid = g
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .parent(0)
            .unwrap()
            .id()
            .to_string();

        create_tag(&repo, "release", Some(&parent_oid), "release notes").unwrap();
        let r = g.find_reference("refs/tags/release").unwrap();
        // Annotated tags resolve via peel.
        let target_oid = r.peel(git2::ObjectType::Commit).unwrap().id().to_string();
        assert_eq!(target_oid, parent_oid);
        // Annotated tags have a tag object — check the message.
        let tag_obj = g.find_tag(r.target().unwrap()).unwrap();
        assert_eq!(tag_obj.message().unwrap().trim(), "release notes");
    }

    #[test]
    fn create_tag_empty_name_fails() {
        let repo = fixture("tag_empty_name");
        seed_commits(&repo, &["a"]);
        let err = create_tag(&repo, "  ", None, "").unwrap_err();
        assert!(format!("{err:#}").contains("empty"));
    }

    /// Regression: a hostile branch / ref name starting with `-` must
    /// be passed as a positional arg, not interpreted as a flag.
    /// `--end-of-options` (after the subcommand) is the load-bearing
    /// piece — without it `git checkout -fhostile` would parse as
    /// `git checkout -f hostile`.
    #[test]
    fn checkout_dash_prefixed_ref_does_not_inject_a_flag() {
        let repo = fixture("checkout_dash_arg");
        seed_commits(&repo, &["a"]);
        // `-fhostile` doesn't exist as a branch, so the checkout will
        // fail. The point: the failure must come from "ref not found"
        // (i.e. git treated the arg as a positional ref) rather than
        // git silently accepting -f as a flag and trying to switch to
        // a branch named `hostile`.
        let err = checkout(&repo, "-fhostile").unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("-fhostile") || msg.contains("did not match") || msg.contains("not a"),
            "unexpected error (suggests the arg was interpreted as a flag): {msg}",
        );
    }

    /// Refspec construction: target equal to local (or empty) ⇒ short
    /// form; otherwise explicit `local:target`.
    #[test]
    fn push_refspec_short_form_when_target_matches_local() {
        assert_eq!(build_push_refspec("main", "main").as_deref(), Some("main"));
        assert_eq!(build_push_refspec("main", "").as_deref(), Some("main"));
        assert_eq!(
            build_push_refspec("feature/x", "feature/x").as_deref(),
            Some("feature/x"),
        );
    }

    #[test]
    fn push_refspec_explicit_when_target_renames() {
        assert_eq!(
            build_push_refspec("feature/x", "pr-123").as_deref(),
            Some("feature/x:pr-123"),
        );
        assert_eq!(
            build_push_refspec("main", "release").as_deref(),
            Some("main:release"),
        );
    }

    #[test]
    fn push_refspec_none_for_empty_local() {
        assert!(build_push_refspec("", "anything").is_none());
        assert!(build_push_refspec("", "").is_none());
    }

    /// Same protection on the branch-creation path.
    #[test]
    fn create_branch_with_dash_prefix_is_not_treated_as_flag() {
        let repo = fixture("branch_dash_arg");
        seed_commits(&repo, &["a"]);
        // git's check-ref-format will reject the name as invalid (it
        // starts with `-`), but it must do so as a *ref name* check,
        // not by silently consuming `-fhostile` as flags. Either an
        // explicit "is not a valid branch name" error, or echoing the
        // exact name in the error, proves the arg crossed the
        // --end-of-options barrier as positional.
        let err = create_branch(&repo, "-fhostile", None, false).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("-fhostile") || msg.contains("not a valid"),
            "unexpected error (suggests the arg was interpreted as a flag): {msg}",
        );
    }
}
