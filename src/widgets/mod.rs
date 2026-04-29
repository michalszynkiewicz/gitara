//! Custom masonry widgets and their Xilem view wrappers.
//!
//! Why: Xilem 0.3's bundled `Button` widget has fixed "pill" chrome (border,
//! gradient fill, fixed minimum height) with no API for overriding. For a
//! polished UI we need flat, theme-aware buttons — so we drop below Xilem
//! into raw masonry.
pub mod clickable_box;
pub mod flat_button;
pub mod flow;
pub mod graph_gutter;
