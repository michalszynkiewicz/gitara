//! Thin re-exports that pin our embedded fonts onto every label/text-input.
//!
//! xilem 0.4's `Xilem::with_font` registers fonts into masonry's
//! FontContext but does not add them to the Latin script's fallback
//! chain, and does not change masonry's default `GenericFamily::SystemUi`.
//! Result: registered fonts only render when a label explicitly names
//! them via `.font(FontStack::Single(FontFamily::Named(...)))`.
//!
//! Wrapping `label()` here lets every call site keep its existing
//! `label("text")` API while quietly forcing our embedded Inter, so
//! the UI looks identical regardless of what fonts the host system
//! has installed.

use std::borrow::Cow;
use xilem::masonry::parley::style::{FontFamily, FontStack};
use xilem::view::Label;

const INTER: FontStack<'static> = FontStack::Single(FontFamily::Named(Cow::Borrowed("Inter")));

pub fn label(text: impl Into<masonry::core::ArcStr>) -> Label {
    xilem::view::label(text).font(INTER)
}
