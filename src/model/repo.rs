use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Clone)]
pub struct RepoView {
    pub path: PathBuf,
    pub name: String,
    pub head: HeadState,
    pub branches: Vec<Branch>,
    pub remotes: Vec<Remote>,
    pub tags: Vec<Tag>,
    pub stashes: Vec<Stash>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HeadState {
    Branch { name: String },
    Detached { oid: String },
    Unborn,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Branch {
    pub name: String,
    pub current: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub tip_oid: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Remote {
    pub name: String,
    pub url: String,
    pub branches: Vec<RemoteBranch>,
    pub last_fetched: Option<OffsetDateTime>,
}

/// Tracking branch for a remote — name like "origin/main" plus the tip oid
/// of the local mirror (refs/remotes/origin/main).
#[derive(Serialize, Deserialize, Clone)]
pub struct RemoteBranch {
    pub name: String,    // e.g. "origin/main"
    pub tip_oid: String, // 40-char hex
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Tag {
    pub name: String,
    pub oid: String,
    pub annotated: bool,
    pub date: OffsetDateTime,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Stash {
    pub idx: u32,
    pub message: String,
    pub date: OffsetDateTime,
    pub on_branch: String,
}
