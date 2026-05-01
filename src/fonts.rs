//! Embedded UI fonts.
//!
//! Both fonts are SIL Open Font License 1.1; license texts live next to
//! the .ttf files under `assets/fonts/licenses/`. We register them with
//! Xilem at startup so rendering doesn't depend on whatever fonts the
//! user happens to have installed (masonry 0.3 used to bundle Roboto;
//! masonry 0.4 dropped that, which left us with random system-font
//! fallbacks and tofu boxes for ✓ ● ↑ ↓ on bare installs).

pub static INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
pub static INTER_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Inter-Medium.ttf");
pub static JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
