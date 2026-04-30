//! Design tokens → typed palette.
//!
//! All colors originate as OKLCH in `DESIGN_TOKENS.md`; they are converted once
//! here to sRGB `vello::peniko::Color` values held by the `Theme` struct.

use vello::peniko::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ThemeMode {
    Light,
    #[default]
    Dark,
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // border_strong / removed_bg are part of the palette but not wired in yet.
pub struct Theme {
    // brand
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_fg: Color,
    pub accent_tint: Color,

    // surfaces
    pub bg: Color,
    pub bg_panel: Color,
    pub bg_panel_2: Color,
    pub bg_panel_3: Color,
    pub bg_hover: Color,
    pub bg_selected: Color,
    pub bg_titlebar: Color,

    // borders
    pub border: Color,
    pub border_strong: Color,
    pub border_faint: Color,

    // text
    pub text: Color,
    pub text_muted: Color,
    pub text_dim: Color,

    // semantic
    pub added: Color,
    pub added_bg: Color,
    pub removed: Color,
    pub removed_bg: Color,
    pub warn: Color,
    pub info: Color,

    // graph lanes (5)
    pub lanes: [Color; 5],
}

impl Theme {
    pub fn light() -> Self {
        Self {
            accent: oklch(0.55, 0.14, 215.0),
            accent_hover: oklch(0.51, 0.14, 215.0),
            accent_fg: Color::from_rgba8(255, 255, 255, 255),
            accent_tint: oklch_a(0.55, 0.14, 215.0, 0.12),

            bg: hex(0xfbfbfc),
            bg_panel: hex(0xffffff),
            bg_panel_2: hex(0xf5f5f7),
            bg_panel_3: hex(0xeeeef1),
            bg_hover: oklch(0.96, 0.002, 260.0),
            bg_selected: oklch_a(0.55, 0.14, 215.0, 0.12),
            bg_titlebar: hex(0xeeeef1),

            border: oklch(0.90, 0.003, 260.0),
            border_strong: oklch(0.84, 0.004, 260.0),
            border_faint: oklch(0.94, 0.003, 260.0),

            text: oklch(0.22, 0.005, 260.0),
            text_muted: oklch(0.50, 0.005, 260.0),
            text_dim: oklch(0.62, 0.004, 260.0),

            added: oklch(0.58, 0.13, 145.0),
            added_bg: oklch(0.95, 0.05, 145.0),
            removed: oklch(0.56, 0.16, 25.0),
            removed_bg: oklch(0.95, 0.04, 25.0),
            warn: oklch(0.72, 0.14, 75.0),
            info: oklch(0.65, 0.10, 230.0),

            lanes: [
                oklch(0.55, 0.10, 260.0),
                oklch(0.58, 0.12, 150.0),
                oklch(0.60, 0.14, 40.0),
                oklch(0.55, 0.13, 320.0),
                oklch(0.60, 0.12, 95.0),
            ],
        }
    }

    pub fn dark() -> Self {
        Self {
            accent: oklch(0.66, 0.14, 215.0),
            accent_hover: oklch(0.62, 0.14, 215.0),
            accent_fg: Color::from_rgba8(255, 255, 255, 255),
            accent_tint: oklch_a(0.66, 0.14, 215.0, 0.20),

            bg: hex(0x17181b),
            bg_panel: hex(0x1d1e22),
            bg_panel_2: hex(0x202125),
            bg_panel_3: hex(0x26272c),
            bg_hover: hex(0x282a2f),
            bg_selected: oklch_a(0.66, 0.14, 215.0, 0.20),
            bg_titlebar: hex(0x1a1b1e),

            border: oklch(0.32, 0.005, 260.0),
            border_strong: oklch(0.42, 0.005, 260.0),
            border_faint: oklch(0.26, 0.005, 260.0),

            // Dark-mode text needs more lift than light-mode's mirrored
            // values: 0.92 looked near-white in isolation but reads dim
            // on bg=0.12. Pushed toward near-white; secondary tiers
            // follow with enough headroom to stay legible.
            text: oklch(0.98, 0.003, 260.0),
            text_muted: oklch(0.82, 0.004, 260.0),
            text_dim: oklch(0.68, 0.005, 260.0),

            added: oklch(0.72, 0.15, 145.0),
            added_bg: oklch_a(0.35, 0.08, 145.0, 0.28),
            removed: oklch(0.72, 0.17, 25.0),
            removed_bg: oklch_a(0.36, 0.10, 25.0, 0.30),
            warn: oklch(0.72, 0.14, 75.0),
            info: oklch(0.65, 0.10, 230.0),

            lanes: [
                oklch(0.72, 0.11, 260.0),
                oklch(0.74, 0.13, 150.0),
                oklch(0.75, 0.14, 40.0),
                oklch(0.72, 0.13, 320.0),
                oklch(0.74, 0.13, 95.0),
            ],
        }
    }

    #[allow(dead_code)] // graph code currently indexes self.lanes directly; keep helper for callers.
    pub fn lane(&self, col: u8) -> Color {
        self.lanes[(col as usize) % self.lanes.len()]
    }
}

// ── conversion helpers ──────────────────────────────────────────────────────

fn hex(rgb: u32) -> Color {
    Color::from_rgba8(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
        255,
    )
}

fn oklch(l: f32, c: f32, h_deg: f32) -> Color {
    oklch_a(l, c, h_deg, 1.0)
}

fn oklch_a(l: f32, c: f32, h_deg: f32, a: f32) -> Color {
    use palette::{IntoColor, Oklch, Srgb};
    let rgb: Srgb = Oklch::new(l, c, h_deg).into_color();
    let clamp = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::from_rgba8(
        clamp(rgb.red),
        clamp(rgb.green),
        clamp(rgb.blue),
        (a.clamp(0.0, 1.0) * 255.0) as u8,
    )
}
