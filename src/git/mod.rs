//! Git adapters. Reads via `gix`, writes/network via `git2`. Each submodule
//! exposes a small async surface the UI calls and awaits progress on.
pub mod log;
pub mod diff;
pub mod refs;
pub mod ops;
pub mod status;

#[cfg(test)]
pub mod test_fixture;
