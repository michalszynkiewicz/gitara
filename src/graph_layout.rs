//! Lane-assignment for the commit graph.
//!
//! Given an ordered list of commits (newest first, as for `git log`), produce a
//! per-row layout describing which column the commit sits in, which columns
//! have lines passing through, and which terminate at this row.
//!
//! Algorithm outline:
//!   * Maintain `active[col] = Some(expected_oid)` — the commit we're next
//!     expecting to draw in each column. A column is "active" when some child
//!     commit has declared it as a parent.
//!   * For each incoming commit (in order):
//!       1. If its oid matches some active column, reuse that column.
//!          Otherwise, take the lowest-index free column.
//!       2. "Through" columns = all active columns *other* than our own that
//!          are still live at this row — they keep their vertical stroke.
//!       3. "Terminating" columns = columns that were expecting *this* commit
//!          (duplicate claims — collapse into one here).
//!       4. Replace our column with the commit's first parent. Assign any
//!          additional parents to fresh columns (merges).
//!   * Commits we never re-enter (roots or off-screen parents) leave their
//!     column vacated at the end.

use crate::model::commit::Commit;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowLayout {
    /// Column (0-indexed) where this commit's node is drawn.
    pub column: u8,
    /// Columns (other than `column`) that have a vertical line passing through this row.
    pub through: Vec<u8>,
    /// Columns that terminate at this row (half-line at top, no node).
    pub terminating: Vec<u8>,
    /// Commit has more than one parent.
    pub is_merge: bool,
    /// For each of this commit's parents beyond the first: the column they're
    /// assigned to (so the row can draw a short "fork" segment from `column`
    /// to the new column).
    pub extra_parent_columns: Vec<u8>,
    /// True when no earlier row chained into `column` — this commit is a
    /// branch tip (or a sole-parent fork point not visible above).
    /// Renderer: don't paint the upper half of own column's vertical line.
    pub lane_starts_here: bool,
    /// True when this commit has no parents (root). Renderer: don't paint
    /// the lower half of own column's vertical line.
    pub lane_ends_here: bool,
    /// Max column index across the whole batch — lets callers size the graph gutter.
    pub max_column: u8,
}

pub fn compute(commits: &[Commit]) -> Vec<RowLayout> {
    // active[col] = Some(oid) means "next commit expected in this column".
    let mut active: Vec<Option<String>> = Vec::new();
    let mut out: Vec<RowLayout> = Vec::with_capacity(commits.len());

    for c in commits {
        // 1. Find or allocate a column for this commit.
        let mut column_idx: Option<usize> = None;
        let mut terminating: Vec<u8> = Vec::new();
        for (i, slot) in active.iter_mut().enumerate() {
            if slot.as_deref() == Some(c.oid.as_str()) {
                match column_idx {
                    None => {
                        column_idx = Some(i);
                    }
                    Some(_) => {
                        // Another column also expected us — it terminates here.
                        terminating.push(i as u8);
                    }
                }
                *slot = None; // vacate; we'll re-populate for parents below.
            }
        }
        let column = column_idx.unwrap_or_else(|| {
            if let Some(free) = active.iter().position(|s| s.is_none()) {
                active[free] = None;
                free
            } else {
                active.push(None);
                active.len() - 1
            }
        });

        // 2. Through columns = all currently active columns (non-terminating,
        //    non-this-commit's-column) that still hold an expected oid.
        let through: Vec<u8> = active
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| {
                if i == column || terminating.contains(&(i as u8)) {
                    return None;
                }
                slot.as_ref().map(|_| i as u8)
            })
            .collect();

        // 3. Assign parents.
        let mut extra_parent_columns: Vec<u8> = Vec::new();
        if let Some(first_parent) = c.parents.first() {
            // Re-use this commit's column for the first parent.
            active[column] = Some(first_parent.clone());
        }
        for p in c.parents.iter().skip(1) {
            // Fresh column for each additional parent.
            let free = active
                .iter()
                .position(|s| s.is_none())
                .unwrap_or_else(|| {
                    active.push(None);
                    active.len() - 1
                });
            active[free] = Some(p.clone());
            extra_parent_columns.push(free as u8);
        }

        out.push(RowLayout {
            column: column as u8,
            through,
            terminating,
            is_merge: c.parents.len() > 1,
            extra_parent_columns,
            lane_starts_here: column_idx.is_none(),
            lane_ends_here: c.parents.is_empty(),
            max_column: 0, // filled in below
        });
    }

    // Second pass — max_column across all rows.
    let max_col = out
        .iter()
        .map(|r| {
            r.column
                .max(r.through.iter().copied().max().unwrap_or(0))
                .max(r.extra_parent_columns.iter().copied().max().unwrap_or(0))
        })
        .max()
        .unwrap_or(0);
    for r in &mut out {
        r.max_column = max_col;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::commit::{Author, Commit};
    use time::OffsetDateTime;

    fn mk(oid: &str, parents: &[&str]) -> Commit {
        let now = OffsetDateTime::now_utc();
        let a = Author { name: "t".into(), email: "t@t".into() };
        Commit {
            oid: oid.into(),
            short: oid[..oid.len().min(7)].into(),
            subject: "".into(),
            body: None,
            author: a.clone(),
            committer: a,
            date: now,
            parents: parents.iter().map(|s| (*s).to_string()).collect(),
            refs: vec![],
            signed: false,
        }
    }

    #[test]
    fn single_line_history() {
        // a -> b -> c (linear)
        let commits = vec![mk("a", &["b"]), mk("b", &["c"]), mk("c", &[])];
        let rows = compute(&commits);
        assert_eq!(rows[0].column, 0);
        assert_eq!(rows[1].column, 0);
        assert_eq!(rows[2].column, 0);
        for r in &rows { assert!(r.through.is_empty()); }
    }

    #[test]
    fn simple_merge() {
        // a (merge) -> b, c
        // b -> d
        // c -> d
        // d -> (root)
        let commits = vec![
            mk("a", &["b", "c"]),
            mk("b", &["d"]),
            mk("c", &["d"]),
            mk("d", &[]),
        ];
        let rows = compute(&commits);
        assert!(rows[0].is_merge);
        assert_eq!(rows[0].column, 0);
        assert_eq!(rows[0].extra_parent_columns, vec![1u8]);
        // Row b: column 0, column 1 (for c) should be through.
        assert_eq!(rows[1].column, 0);
        assert_eq!(rows[1].through, vec![1u8]);
        // Row c: column 1.
        assert_eq!(rows[2].column, 1);
        // Row d: should be the collapse point — column 0 with column 1 terminating.
        assert_eq!(rows[3].column, 0);
        assert!(rows[3].terminating.contains(&1u8));
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(compute(&[]).is_empty());
    }

    #[test]
    fn root_commit_marks_lane_ends_here() {
        // a -> b (root). Only b has lane_ends_here.
        let commits = vec![mk("a", &["b"]), mk("b", &[])];
        let rows = compute(&commits);
        assert!(!rows[0].lane_ends_here, "a has a parent — lane continues");
        assert!(rows[1].lane_ends_here, "b is a root — lane ends here");
    }

    #[test]
    fn branch_tip_marks_lane_starts_here() {
        // The first commit in any lane should mark lane_starts_here.
        // Linear: a -> b -> c. Only `a` is a tip.
        let commits = vec![mk("a", &["b"]), mk("b", &["c"]), mk("c", &[])];
        let rows = compute(&commits);
        assert!(rows[0].lane_starts_here, "a is a branch tip");
        assert!(!rows[1].lane_starts_here, "b's lane was claimed by a");
        assert!(!rows[2].lane_starts_here, "c's lane was claimed by b");
    }

    #[test]
    fn parallel_lanes_with_offscreen_parents_take_separate_columns() {
        // Two tips whose parents are off-screen (not in the input).
        // Neither lane vacates within the window, so they each hold a
        // distinct column. Documents that lane reuse only happens once
        // a column has actually become free.
        let commits = vec![mk("a", &["b"]), mk("c", &["d"])];
        let rows = compute(&commits);
        assert_eq!(rows[0].column, 0);
        assert_eq!(rows[1].column, 1);
        assert!(rows[0].lane_starts_here);
        assert!(rows[1].lane_starts_here);
    }

    #[test]
    fn octopus_merge_assigns_extra_parent_per_extra_parent() {
        // a (3-parent merge) -> b, c, d
        let commits = vec![
            mk("a", &["b", "c", "d"]),
            mk("b", &[]),
            mk("c", &[]),
            mk("d", &[]),
        ];
        let rows = compute(&commits);
        assert!(rows[0].is_merge);
        // First parent reuses a's column (0); the other two get fresh
        // columns. We don't pin the exact indices, just the count.
        assert_eq!(rows[0].extra_parent_columns.len(), 2);
        // At row b (col 0), c and d are still pending → both pass through.
        assert_eq!(rows[1].column, 0);
        let mut through = rows[1].through.clone();
        through.sort();
        assert_eq!(through, vec![1u8, 2u8]);
    }

    #[test]
    fn vacated_lane_is_reused_by_later_independent_tip() {
        // a merges b, c. Both b and c are roots, so cols 0 and 1 vacate.
        // d is an independent root and should take the lowest free col (0).
        let commits = vec![
            mk("a", &["b", "c"]),
            mk("b", &[]),
            mk("c", &[]),
            mk("d", &[]),
        ];
        let rows = compute(&commits);
        assert_eq!(rows[3].column, 0, "d should reuse col 0 vacated by b");
        assert!(rows[3].lane_starts_here);
        assert!(rows[3].lane_ends_here);
    }

    #[test]
    fn max_column_is_uniform_across_rows() {
        // Octopus merge pushes max_column to 2; every row should report
        // the same max so callers can size the gutter once.
        let commits = vec![
            mk("a", &["b", "c", "d"]),
            mk("b", &[]),
            mk("c", &[]),
            mk("d", &[]),
        ];
        let rows = compute(&commits);
        for r in &rows {
            assert_eq!(r.max_column, 2);
        }
    }

    #[test]
    fn merge_collapse_marks_terminating_not_through() {
        // a merges b, c — both b and c have parent d.
        // At row d (the merge-base), col 1 (c's lane) terminates.
        // It must appear in `terminating`, not in `through`.
        let commits = vec![
            mk("a", &["b", "c"]),
            mk("b", &["d"]),
            mk("c", &["d"]),
            mk("d", &[]),
        ];
        let rows = compute(&commits);
        let d_row = &rows[3];
        assert!(d_row.terminating.contains(&1u8));
        assert!(!d_row.through.contains(&1u8),
            "a terminating column must not also be drawn as a through line");
    }
}
