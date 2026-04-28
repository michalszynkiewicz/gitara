//! Parley font context + embedded font bytes.
//!
//! Ship Inter and JetBrains Mono alongside the binary to avoid depending on
//! what the user has installed.
//!
//! Put TTF/OTF files at:
//!   assets/fonts/Inter-Variable.ttf
//!   assets/fonts/JetBrainsMono-Variable.ttf
//!
//! TODO(xilem-api): the Parley FontContext construction + how you hand it to
//! Masonry has moved across versions. Check `masonry::widget::Label` internals
//! or Xilem's `examples/variable_clock` for the current idiom.

// static INTER:     &[u8] = include_bytes!("../assets/fonts/Inter-Variable.ttf");
// static JB_MONO:   &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Variable.ttf");

#[allow(dead_code)] // Wired up once we ship embedded fonts (Phase 0 leftover).
pub fn install() {
    // TODO(phase-0): register the embedded fonts into parley::FontContext
    // and hand it to Masonry. Pseudocode:
    //
    //   let mut fc = parley::FontContext::new();
    //   fc.collection.register_fonts(INTER.to_vec());
    //   fc.collection.register_fonts(JB_MONO.to_vec());
    //   // then set as the default for the Masonry root.
}
