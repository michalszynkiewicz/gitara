//! Embedded icon font.
//!
//! We ship a 1.5 KB subset of Phosphor (MIT, license at
//! `assets/fonts/licenses/Phosphor-MIT.txt`) covering only the
//! handful of UI icons we render — see `crate::ui` for the
//! name → codepoint mapping. Body text uses whatever the host's
//! `system-ui` and `monospace` resolve to.

pub static PHOSPHOR_SUBSET: &[u8] = include_bytes!("../assets/fonts/Phosphor-Subset.ttf");
