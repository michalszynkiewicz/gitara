//! Walk the commit log via `gix` — synchronous, first-page.
//!
//! The streaming async version from the prototype is deferred until we actually
//! need it (large repos). For now, a single-shot load returning the first N
//! commits reachable from HEAD is plenty.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;
use time::OffsetDateTime;

use crate::model::commit::{Author, Commit, RefChip};
use crate::model::repo::RepoView;

pub fn load_commits(
    path: &Path,
    repo_view: &RepoView,
    limit: usize,
    all_refs: bool,
) -> anyhow::Result<Vec<Commit>> {
    let repo =
        gix::discover(path).with_context(|| format!("discover git repo at {}", path.display()))?;

    // Index ref chips by oid. Keyed by full 40-char oid so we can match on
    // each commit's own oid exactly.
    let mut chips: BTreeMap<String, Vec<RefChip>> = BTreeMap::new();
    let head_oid: Option<String> = repo.head_id().ok().map(|id| id.to_string());

    for b in &repo_view.branches {
        chips
            .entry(b.tip_oid.clone())
            .or_default()
            .push(RefChip::Branch {
                name: b.name.clone(),
                current: b.current,
            });
    }
    for r in &repo_view.remotes {
        for rb in &r.branches {
            chips
                .entry(rb.tip_oid.clone())
                .or_default()
                .push(RefChip::Remote {
                    name: rb.name.clone(),
                });
        }
    }
    for t in &repo_view.tags {
        chips.entry(t.oid.clone()).or_default().push(RefChip::Tag {
            name: t.name.clone(),
            annotated: t.annotated,
        });
    }
    if let Some(ref h) = head_oid {
        chips.entry(h.clone()).or_default().push(RefChip::Head);
    }

    // Pick the rev-walk start set. With `all_refs` we want gitk-style "all
    // branches", which means starting from every branch tip + HEAD (gix
    // dedupes commits internally). HEAD-only mode keeps the historical
    // behavior — current branch + ancestors only.
    let mut starts: Vec<gix::ObjectId> = Vec::new();
    if all_refs {
        for b in &repo_view.branches {
            if let Ok(id) = gix::ObjectId::from_hex(b.tip_oid.as_bytes()) {
                starts.push(id);
            }
        }
        for r in &repo_view.remotes {
            for rb in &r.branches {
                if let Ok(id) = gix::ObjectId::from_hex(rb.tip_oid.as_bytes()) {
                    starts.push(id);
                }
            }
        }
        for t in &repo_view.tags {
            if let Ok(id) = gix::ObjectId::from_hex(t.oid.as_bytes()) {
                starts.push(id);
            }
        }
    }
    if let Ok(id) = repo.head_id() {
        starts.push(id.detach());
    }
    if starts.is_empty() {
        return Ok(Vec::new()); // unborn / no refs
    }

    // ByCommitTimeNewestFirst gives a single chronological log across all
    // start points — essential when we have multiple branch tips, otherwise
    // commits from different branches would appear in arbitrary order.
    let walker = repo
        .rev_walk(starts)
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
        .context("start rev walk")?;

    let mut out: Vec<Commit> = Vec::new();
    for info in walker {
        if out.len() >= limit {
            break;
        }
        let info = info?;
        let obj = info.object()?; // commit
        let oid_str = info.id.to_string();
        let (subject, body) = split_message(obj.message_raw_sloppy());
        let parents: Vec<String> = obj.parent_ids().map(|id| id.to_string()).collect();

        let (author, date) = extract_author(&obj);

        // Detect a signature on the commit object. Verification (good /
        // bad / unknown) needs `git verify-commit`, which is per-commit
        // and slow — we just record presence here. SSH / X.509 sigs land
        // in the same `gpgsig` header so this catches them all.
        let signed = obj.signature().ok().flatten().is_some();

        let mut commit = Commit {
            oid: oid_str.clone(),
            short: oid_str[..oid_str.len().min(7)].to_string(),
            subject,
            body,
            author: author.clone(),
            committer: author,
            date,
            parents,
            refs: chips.get(&oid_str).cloned().unwrap_or_default(),
            signed,
        };
        // Stable sort within refs: HEAD last so UI can render it after branches.
        commit.refs.sort_by_key(|c| match c {
            RefChip::Branch { current: true, .. } => 0,
            RefChip::Branch { .. } => 1,
            RefChip::Head => 2,
            RefChip::Remote { .. } => 3,
            RefChip::Tag { .. } => 4,
        });
        out.push(commit);
    }
    Ok(out)
}

fn split_message(raw: &[u8]) -> (String, Option<String>) {
    let s = String::from_utf8_lossy(raw).to_string();
    // Prefer the conventional `subject\n\nbody` split; fall back to the
    // first single newline so messages without a blank-line separator
    // still surface their body lines (otherwise everything past line 1
    // would be silently lost from the Details view).
    let (subject, body) = if let Some((sub, body)) = s.split_once("\n\n") {
        (sub.to_string(), Some(body.to_string()))
    } else if let Some((sub, body)) = s.split_once('\n') {
        (sub.to_string(), Some(body.to_string()))
    } else {
        (s.clone(), None)
    };
    let body = body
        .map(|b| b.trim_end_matches('\n').to_string())
        .filter(|b| !b.is_empty());
    (subject, body)
}

fn extract_author(commit: &gix::Commit<'_>) -> (Author, OffsetDateTime) {
    // gix exposes author via commit.author() returning a SignatureRef.
    // In gix 0.83 the raw `time` field is a &BStr (lossless bytes);
    // call `.time()` to parse it into a `gix_date::Time` whose
    // `seconds` field we feed to `OffsetDateTime::from_unix_timestamp`.
    if let Ok(sig) = commit.author() {
        let name = sig.name.to_string();
        let email = sig.email.to_string();
        let date = sig
            .time()
            .ok()
            .and_then(|t| OffsetDateTime::from_unix_timestamp(t.seconds).ok())
            .unwrap_or_else(OffsetDateTime::now_utc);
        (Author { name, email }, date)
    } else {
        (
            Author {
                name: "?".into(),
                email: "?".into(),
            },
            OffsetDateTime::now_utc(),
        )
    }
}

#[cfg(test)]
mod split_message_tests {
    use super::split_message;

    #[test]
    fn blank_line_separator_keeps_body() {
        let (sub, body) = split_message(b"subject line\n\nbody l1\nbody l2\n");
        assert_eq!(sub, "subject line");
        assert_eq!(body.as_deref(), Some("body l1\nbody l2"));
    }

    #[test]
    fn no_blank_line_still_extracts_body() {
        let (sub, body) = split_message(b"subject\nbody l1\nbody l2");
        assert_eq!(sub, "subject");
        assert_eq!(body.as_deref(), Some("body l1\nbody l2"));
    }

    #[test]
    fn single_line_has_no_body() {
        let (sub, body) = split_message(b"only one line");
        assert_eq!(sub, "only one line");
        assert!(body.is_none());
    }

    #[test]
    fn trailing_blank_lines_dropped() {
        let (_sub, body) = split_message(b"subj\n\nbody\n\n\n");
        assert_eq!(body.as_deref(), Some("body"));
    }
}
