//! Auto-install a `.desktop` file + icon under `~/.local/share` so the
//! Wayland (or X11) compositor can match the running window to its
//! taskbar/dock icon.
//!
//! Why we need this on Wayland:
//!   * Wayland has no protocol for a window to declare its own taskbar
//!     icon. Compositors look up the window's `app_id` (set via
//!     winit's `with_name`) in the system's installed `.desktop`
//!     files and use the `Icon=` from there.
//!   * Without a matching `.desktop` file, the window inherits the
//!     parent process's icon (e.g. the launching terminal) or shows
//!     a generic application glyph.
//!
//! What this writes (idempotent — only creates files that don't
//! exist; respects a system-wide install in `/usr/share/applications`):
//!   * `~/.local/share/icons/hicolor/256x256/apps/gitara.png`
//!   * `~/.local/share/applications/gitara.desktop`
//!
//! Effect: the icon shows up after the *next* launch (the current
//! window's app_id was set before the .desktop file existed; some
//! compositors re-scan on demand, others on next session).

/// Escape and quote an executable path for the Desktop Entry `Exec=` key.
///
/// Returns `None` if the path contains control characters that the spec
/// cannot represent (e.g. newlines, NUL). Falls back to `"gitara"` at
/// the call site.
fn exec_quote(path: &str) -> Option<String> {
    if path.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return None;
    }
    // Escape backslash and double-quote per the Desktop Entry spec, then
    // wrap the whole path in double-quotes so spaces parse correctly.
    let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
    Some(format!("\"{escaped}\""))
}

#[cfg(target_os = "linux")]
pub fn ensure_installed() -> std::io::Result<()> {
    use std::fs;
    use std::path::PathBuf;

    // If a system package already provides gitara.desktop, leave it
    // alone — the system path takes precedence over the per-user one.
    let system_desktop = PathBuf::from("/usr/share/applications/gitara.desktop");
    if system_desktop.exists() {
        return Ok(());
    }

    let Some(home) = directories::BaseDirs::new() else {
        return Ok(());
    };
    let data_local = home.data_local_dir();

    let apps_dir = data_local.join("applications");
    let desktop_path = apps_dir.join("gitara.desktop");

    // Render at every hicolor size we ship. Plasma on high-DPI panels
    // reaches for 512+ and falls back to whatever cached icon it last
    // knew if no large entry exists, so shipping the full size ladder
    // keeps the dock crisp on any display. Always re-render so design
    // changes propagate without the user clearing caches by hand.
    let desktop_already = desktop_path.exists();
    for &px in crate::logo::INSTALL_SIZES {
        let dir = data_local.join(format!("icons/hicolor/{px}x{px}/apps"));
        fs::create_dir_all(&dir)?;
        let path = dir.join("gitara.png");
        let pixmap = crate::logo::render_pixmap_at(px);
        pixmap
            .save_png(&path)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
    }

    if !desktop_already {
        fs::create_dir_all(&apps_dir)?;
        let exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.to_str().and_then(|s| exec_quote(s)))
            .unwrap_or_else(|| "gitara".to_string());
        // StartupWMClass MUST match the app_id set via
        // winit::WindowAttributesExtWayland::with_name (general arg).
        let body = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=gitara\n\
             GenericName=Git GUI\n\
             Comment=Native git GUI in Rust\n\
             Exec={exe} %F\n\
             Icon=gitara\n\
             Terminal=false\n\
             Categories=Development;RevisionControl;\n\
             StartupWMClass=gitara\n"
        );
        fs::write(&desktop_path, body)?;
    }

    // Refresh the per-user icon and .desktop caches so launchers pick
    // up the new entry without a logout/login. Both are best-effort —
    // most compositors look up files directly anyway.
    let _ = std::process::Command::new("gtk-update-icon-cache")
        .arg("-q")
        .arg("-t")
        .arg(data_local.join("icons/hicolor"))
        .status();
    let _ = std::process::Command::new("update-desktop-database")
        .arg("-q")
        .arg(&apps_dir)
        .status();

    // KDE Plasma uses its own ksycoca cache for .desktop entries —
    // separate from the freedesktop xdg cache that update-desktop-
    // database refreshes. Without this, Plasma doesn't notice new
    // .desktop files and the window keeps a generic dock icon. Try
    // both ksycoca versions; whichever fails silently (Plasma 5 vs 6).
    let _ = std::process::Command::new("kbuildsycoca6").status();
    let _ = std::process::Command::new("kbuildsycoca5").status();

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn ensure_installed() -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::exec_quote;

    #[test]
    fn plain_path_is_quoted() {
        assert_eq!(
            exec_quote("/usr/bin/gitara").unwrap(),
            "\"/usr/bin/gitara\""
        );
    }

    #[test]
    fn path_with_spaces_is_quoted() {
        assert_eq!(
            exec_quote("/home/user/my apps/gitara").unwrap(),
            "\"/home/user/my apps/gitara\""
        );
    }

    #[test]
    fn backslash_is_escaped() {
        assert_eq!(exec_quote("C:\\gitara").unwrap(), "\"C:\\\\gitara\"");
    }

    #[test]
    fn double_quote_is_escaped() {
        assert_eq!(
            exec_quote("/path/\"bad\"/gitara").unwrap(),
            "\"/path/\\\"bad\\\"/gitara\""
        );
    }

    #[test]
    fn control_chars_return_none() {
        assert!(exec_quote("/path/with\nnewline").is_none());
        assert!(exec_quote("/path/with\x00nul").is_none());
    }
}
