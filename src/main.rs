//! gitara — a native Git GUI.
//!
//! Entry point. Boots Xilem, installs fonts, shows the main window.
//!
//! NOTE: Xilem's top-level app + event-loop API has shifted across 0.1/0.2/0.3.
//! The skeleton below is the general shape; reconcile with the examples in
//! https://github.com/linebender/xilem/tree/main/xilem/examples for your pin.

mod app;
mod theme;
mod fonts;
mod model;
mod git;
mod graph_layout;
mod widgets;
mod views;
mod persist;
mod mock;
mod logo;
mod desktop_install;

use app::AppState;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // wgpu auto-detects "best" by initialising every candidate backend in
    // parallel and picking one. On Linux this can hit a buggy GLES init
    // (EGL_BAD_ATTRIBUTE) that doesn't cleanly fail and segfaults later;
    // pinning the candidate set keeps the broken backend out. On macOS we
    // only have Metal; on Windows DX12 is the native pick. Respect any
    // existing user override.
    if std::env::var_os("WGPU_BACKEND").is_none()
        && std::env::var_os("WGPU_BACKENDS").is_none()
    {
        // Per-OS defaults match what other wgpu apps (Bevy, Zed) use.
        // Safe: we're still single-threaded — wgpu reads this lazily.
        let preferred = if cfg!(target_os = "macos") {
            "metal"
        } else if cfg!(target_os = "windows") {
            "dx12,vulkan"
        } else {
            // Linux / BSD / etc.
            //
            // Vulkan-only when Vulkan is available; GL otherwise. We
            // can't list both ("vulkan,gl") because wgpu probes every
            // listed backend, and the GL probe loads libEGL_nvidia +
            // libnvidia-egl-wayland which have a long-known double-free
            // on shutdown (wl_map_insert_at via terminateDisplay). The
            // probe alone is enough to crash on exit even when vulkan
            // was actually selected. So pick one.
            //
            // Probing libvulkan.so.1 with dlopen is cheap and doesn't
            // load any Vulkan driver — libvulkan is just the libglvnd
            // dispatcher.
            if linux_has_vulkan() { "vulkan" } else { "gl" }
        };
        unsafe { std::env::set_var("WGPU_BACKEND", preferred); }
    }

    // Last-resort safety net. Even with vulkan-only there are still
    // routes to a SIGSEGV during shutdown (e.g. xkbcommon, wayland-client
    // statics torn down out of order by a third-party crate). Install a
    // SIGSEGV handler that hard-exits cleanly. Set GITARA_DEBUG_CRASHES=1
    // to disable for debugging.
    #[cfg(unix)]
    if std::env::var_os("GITARA_DEBUG_CRASHES").is_none() {
        install_segv_handler();
    }

    let settings = persist::Settings::load().unwrap_or_default();
    let state = AppState::boot(settings)?;

    // On Linux, install ~/.local/share/applications/gitara.desktop +
    // the icon PNG once so the Wayland compositor (or X11 launcher)
    // can match our window's app_id to a real icon. No-op on macOS /
    // Windows. Failure is non-fatal — the app still runs, just with a
    // generic taskbar icon.
    if let Err(e) = desktop_install::ensure_installed() {
        eprintln!("gitara: desktop-entry install: {e:#}");
    }

    // Render the window icon vector → RGBA at startup. Used on X11 via
    // _NET_WM_ICON; ignored on Wayland (see desktop_install for that
    // path), but rendering it is cheap so we always do it.
    let window_icon = {
        let (rgba, side) = logo::render();
        xilem::winit::window::Icon::from_rgba(rgba, side, side).ok()
    };

    let mut window_attributes = xilem::winit::window::Window::default_attributes()
        .with_title("gitara")
        .with_resizable(true)
        .with_window_icon(window_icon)
        .with_min_inner_size(xilem::winit::dpi::LogicalSize::new(800.0, 500.0))
        .with_inner_size(xilem::winit::dpi::LogicalSize::new(1280.0, 800.0));

    // Linux: set the window's app_id (Wayland) / WM_CLASS (X11) to
    // "gitara". Wayland compositors look up gitara.desktop by this id;
    // without it the window inherits the launching terminal's icon.
    #[cfg(target_os = "linux")]
    {
        use xilem::winit::platform::wayland::WindowAttributesExtWayland;
        use xilem::winit::platform::x11::WindowAttributesExtX11;
        window_attributes = WindowAttributesExtWayland::with_name(
            window_attributes,
            "gitara",
            "gitara",
        );
        window_attributes = WindowAttributesExtX11::with_name(
            window_attributes,
            "gitara",
            "gitara",
        );
    }

    // Headless screenshot harness runs inside Xvfb without a window manager;
    // winit's inner_size hint is ignored there, so force borderless fullscreen
    // to fill the Xvfb screen.
    if std::env::var_os("GITARA_HEADLESS").is_some() {
        window_attributes = window_attributes
            .with_fullscreen(Some(xilem::winit::window::Fullscreen::Borderless(None)))
            .with_decorations(false);
    }

    let result = xilem::Xilem::new(state, app::root_view)
        .run_windowed_in(xilem::EventLoop::with_user_event(), window_attributes);

    // Bypass static destructors and atexit handlers. wgpu/vello/winit on
    // Wayland (and sometimes X11) can SIGSEGV during shutdown when their
    // worker pools race with GPU-context teardown — and the std::process::exit
    // path still runs libc atexit handlers, which is where some of the
    // shared-lib statics race. _exit terminates the process immediately and
    // lets the kernel reclaim everything.
    let code = match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("gitara: {e:#}");
            1
        }
    };
    #[cfg(unix)]
    unsafe { libc::_exit(code); }
    #[cfg(not(unix))]
    std::process::exit(code);
}

/// True if `libvulkan.so.1` can be dlopen'd. Used at startup to pick
/// between WGPU_BACKEND=vulkan (preferred) and WGPU_BACKEND=gl
/// (fallback for pre-2014 GPUs that have no Vulkan driver).
#[cfg(all(unix, not(target_os = "macos")))]
fn linux_has_vulkan() -> bool {
    use std::ffi::CString;
    let name = CString::new("libvulkan.so.1").expect("nul-free");
    unsafe {
        let handle = libc::dlopen(name.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
        if handle.is_null() {
            false
        } else {
            libc::dlclose(handle);
            true
        }
    }
}
#[cfg(any(not(unix), target_os = "macos"))]
fn linux_has_vulkan() -> bool { true }

/// Install a SIGSEGV handler that prints a one-line message and hard-exits
/// the process. Trades coredump-on-crash (for debugging) for clean shutdown
/// — see the comment at the call site.
///
/// Async-signal-safe: only `write(2)` and `_exit` are used inside the
/// handler. No allocation, no locking, no Rust runtime calls.
#[cfg(unix)]
fn install_segv_handler() {
    extern "C" fn handler(
        _sig: libc::c_int,
        _info: *mut libc::siginfo_t,
        _ctx: *mut libc::c_void,
    ) {
        const MSG: &[u8] = b"gitara: SIGSEGV during shutdown - exiting cleanly (set GITARA_DEBUG_CRASHES=1 for a coredump)\n";
        unsafe {
            libc::write(2, MSG.as_ptr() as *const _, MSG.len());
            libc::_exit(0);
        }
    }
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = handler as *const () as usize;
        sa.sa_flags = libc::SA_SIGINFO;
        libc::sigaction(libc::SIGSEGV, &sa, std::ptr::null_mut());
    }
}
