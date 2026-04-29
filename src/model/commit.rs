use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Clone)]
pub struct Commit {
    pub oid: String,
    pub short: String,
    pub subject: String,
    pub body: Option<String>,
    pub author: Author,
    pub committer: Author,
    pub date: OffsetDateTime,
    pub parents: Vec<String>,
    pub refs: Vec<RefChip>,
    /// True iff the commit object carries a `gpgsig` (or `gpgsig-sha256`)
    /// header. We don't verify the signature here — that needs
    /// `git verify-commit` and is expensive enough we'd run it on demand.
    #[serde(default)]
    pub signed: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Author {
    pub name: String,
    pub email: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RefChip {
    Branch { name: String, current: bool },
    Remote { name: String },
    Tag { name: String, annotated: bool },
    Head,
}

/// Computed per viewport. Not persisted. Currently unused — graph_layout
/// builds richer RowLayout values directly. Kept around for future cases
/// where a slimmer per-commit assignment is useful.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct LaneAssignment {
    pub column: u8,
    pub lane_color_idx: u8,
    pub is_merge: bool,
}
