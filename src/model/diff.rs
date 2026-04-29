use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileChange {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: FileStatus,
    pub staged: bool,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChange,
    Conflicted,
    Untracked,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hunk {
    pub old_start: u32,
    pub old_len: u32,
    pub new_start: u32,
    pub new_len: u32,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiffLine {
    pub origin: DiffOrigin,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum DiffOrigin {
    Context,
    Added,
    Removed,
}
