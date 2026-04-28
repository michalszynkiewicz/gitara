use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Clone)]
pub struct ReflogEntry {
    pub idx: u32,
    pub oid: String,
    pub short: String,
    pub action: ReflogAction,
    pub subject: String,
    pub date: OffsetDateTime,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ReflogAction {
    Commit, Merge, Checkout, Rebase, Reset, Pull, Push, Clone, CherryPick, Amend, Other,
}
