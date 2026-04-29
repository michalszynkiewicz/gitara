//! Git adapters. Reads via `gix`, writes/network via `git2`. Each submodule
//! exposes a small async surface the UI calls and awaits progress on.
pub mod diff;
pub mod log;
pub mod ops;
pub mod refs;
pub mod status;

#[cfg(test)]
pub mod test_fixture;
