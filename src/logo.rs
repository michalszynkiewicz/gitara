//! gitara window-icon: a "g"-shaped guitar mark.
//!
//! The PNG at `assets/logo.png` is embedded into the binary at compile
//! time. At runtime we decode it once and scale it to whatever size
//! the caller asks for — used for both the `winit` window-icon
//! (X11 _NET_WM_ICON) and for writing the per-size hicolor PNGs that
//! the Wayland compositor matches via `gitara.desktop`.

use tiny_skia::{
    FilterQuality, IntSize, Pixmap, PixmapPaint, PixmapRef, Transform,
};

const LOGO_BYTES: &[u8] = include_bytes!("../assets/logo.png");

/// Default size for the runtime window icon (X11 _NET_WM_ICON).
pub const SIZE: u32 = 256;

/// Sizes installed under `~/.local/share/icons/hicolor/<N>x<N>/apps/`.
/// Hicolor theme picks the closest size to what the compositor asks
/// for; high-DPI panels reach for 512+, so we ship up to that.
pub const INSTALL_SIZES: &[u32] = &[64, 128, 256, 512];

/// Render the icon at the default size. Returns `(rgba_bytes, side_px)`
/// ready for `winit::window::Icon::from_rgba`.
pub fn render() -> (Vec<u8>, u32) {
    let pixmap = render_pixmap_at(SIZE);
    let rgba = pixmap.take();
    (rgba, SIZE)
}

/// Render at a specific size. Decodes the embedded PNG and rescales
/// it onto a square `Pixmap` of the requested side length using a
/// bilinear filter.
pub fn render_pixmap_at(size_px: u32) -> Pixmap {
    let src = decode_logo();
    let src_w = src.width() as f32;
    let src_h = src.height() as f32;

    let mut dst = Pixmap::new(size_px, size_px).expect("non-zero pixmap");
    // Transparent background so the icon shape is what the compositor
    // alpha-composites onto whatever dock/titlebar bg sits behind it.
    dst.fill(tiny_skia::Color::TRANSPARENT);

    // Uniform scale to fit the source into our square. The asset is
    // 1024×1024, so this is always a downscale for our install sizes.
    let scale = (size_px as f32) / src_w.max(src_h);
    let dx = (size_px as f32 - src_w * scale) * 0.5;
    let dy = (size_px as f32 - src_h * scale) * 0.5;

    let paint = PixmapPaint {
        opacity: 1.0,
        blend_mode: tiny_skia::BlendMode::SourceOver,
        // Bilinear gives clean downscaling without nearest-neighbor
        // aliasing. The source is large (1024) so quality is fine.
        quality: FilterQuality::Bilinear,
    };

    dst.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &paint,
        Transform::from_scale(scale, scale).post_translate(dx, dy),
        None,
    );

    dst
}

/// Decode the embedded PNG into a `Pixmap`. Cheap enough at startup
/// (~1MB PNG, decoded once per icon size) that we don't bother
/// caching across calls.
fn decode_logo() -> Pixmap {
    Pixmap::decode_png(LOGO_BYTES).unwrap_or_else(|e| {
        // The asset is part of the binary; failure here means a build
        // problem (corrupted file, wrong format). Fall back to a
        // 1×1 transparent pixmap so the binary still launches.
        eprintln!("gitara: failed to decode embedded logo: {e}");
        let mut pm = Pixmap::new(1, 1).expect("1×1");
        pm.fill(tiny_skia::Color::TRANSPARENT);
        pm
    })
}

// Keep IntSize/PixmapRef in scope so the imports above don't get
// flagged as unused if we ever drop one of the helpers.
const _: fn() = || {
    let _: Option<IntSize> = None;
    let _: Option<PixmapRef<'static>> = None;
};

#[cfg(test)]
mod tests {
    /// Render the icon and save it as a PNG. Run with:
    /// `cargo test dump_logo -- --include-ignored`
    /// Output goes to /tmp/gitara-logo.png — useful for visual checks
    /// after swapping the asset.
    #[test]
    #[ignore]
    fn dump_logo() {
        let pm = super::render_pixmap_at(super::SIZE);
        pm.save_png("/tmp/gitara-logo.png").unwrap();
    }
}
