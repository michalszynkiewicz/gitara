# Implementation notes

## Stack

* **UI**: Xilem 0.3 + Masonry 0.3 (locally vendored at
  `../vendor/masonry`, with a couple of small patches kept under the
  `[patch.crates-io]` section of `Cargo.toml`).
* **Renderer**: Vello 0.5 + wgpu, GPU-accelerated.
* **Git reads**: `gix` (commit log, branches/remotes/tags/refs, head
  state) and `git2` (diffs, status, test fixtures).
* **Git writes**: shell out to the `git` CLI.

### Why the read/write split

Read-only inspection (log, diff, status) is independent of user
config and faster in-process via the libraries. Writes (commit,
branch ops, push, fetch, merge, rebase, …) shell out to `git` so the
user's `~/.gitconfig` (signing key, hooks path, includeIf, credential
helpers, SSH agent, …) just works. Hooks fire as expected, and the
operation is byte-for-byte what the user would type.

### libgit2 is vendored

`git2` is configured with `features = ["vendored-libgit2"]`. libgit2
is statically built from source and linked into the gitara binary, so
we don't depend on the system having a particular `libgit2.so.X.Y`
installed at runtime.

### `--end-of-options` against argument-injection

Every shell-out in `src/git/ops.rs` passes `--end-of-options` (git
2.24+) before user-controlled positional args. Without it, a hostile
branch / ref / remote name like `-fhostile` would be reinterpreted as
a flag (CVE-2017-1000117 class). The two ad-hoc shell-outs that used
to live in `views/modals/merge.rs` and `views/modals/cherry_pick.rs`
were merged into `ops::merge` and `ops::cherry_pick` so the
mitigation lives in one place.

## Linux desktop integration

On first launch (`src/desktop_install.rs`), gitara writes:

* `~/.local/share/applications/gitara.desktop` with
  `Icon=gitara`, `StartupWMClass=gitara`, and an `Exec=` line
  pointing at the running binary's absolute path.
* `~/.local/share/icons/hicolor/{64,128,256,512}x{...}/apps/gitara.png`
  rendered from the embedded `assets/logo.png` at the requested size.

Wayland compositors don't have a per-window icon protocol — they look
up the running window's `app_id` (we set it to `gitara` via winit's
`with_name(...)`) in the installed `.desktop` files. X11 also reads
the icon embedded via `_NET_WM_ICON`, which winit sets from the same
PNG bytes.

After writing the files, gitara best-effort runs:

* `gtk-update-icon-cache` (per-user hicolor)
* `update-desktop-database` (freedesktop xdg cache)
* `kbuildsycoca6` / `kbuildsycoca5` (KDE Plasma's own ksycoca cache —
  separate from xdg)

If the icon doesn't appear after a fresh launch, log out and back in
once. Plasma in particular caches per-app icons in plasmashell state
that survives kbuildsycoca refreshes.

The `.desktop` file is written only if it doesn't already exist. The
icon PNGs are always re-rendered so design changes propagate without
the user clearing caches by hand. A `/usr/share/applications/
gitara.desktop` (i.e. system-package install) takes precedence — the
per-user install bails out in that case.

## Shutdown

Wayland + NVIDIA proprietary driver has a long-known double-free
during EGL terminate (`wl_map_insert_at` via `terminateDisplay` in
`libnvidia-egl-wayland.so.1`). Two layers of defence:

1. `WGPU_BACKEND` is picked at startup. On Linux/BSD: if
   `libvulkan.so.1` is dlopen-able, force `vulkan`; otherwise force
   `gl`. Listing both (the wgpu default) loads `libEGL_nvidia.so.0`
   during the GL probe even when Vulkan was actually selected, which
   triggers the teardown crash on exit.
2. A SIGSEGV handler that writes one line to stderr and calls
   `_exit(0)`. Async-signal-safe (no allocation, no Rust runtime).
   Set `GITARA_DEBUG_CRASHES=1` to disable for getting coredumps.

## Environment variables

All optional. Most are for development.

| Var | Purpose |
|---|---|
| `GITARA_REPO=<path>` | Open this repo instead of the current directory. |
| `GITARA_DARK=1` | Force dark theme (overrides persisted setting). |
| `GITARA_HEADLESS=1` | Borderless fullscreen for screenshot harnesses inside Xvfb. |
| `GITARA_SELECT=<sha>` | Pre-select a commit by oid prefix at startup. |
| `GITARA_MODAL=<kind>` | Open a modal at startup (`commit`, `branch`, `tag`, …). |
| `GITARA_TAB=<tab>` | Open a specific inspector tab (`changes`, `diff`, `files`, `details`). |
| `GITARA_CTX_MENU=…` | Pin a context menu open at startup. |
| `GITARA_DEBUG_CRASHES=1` | Disable the SIGSEGV-to-clean-exit handler (for coredumps). |
| `WGPU_BACKEND=…` | Override the wgpu backend pick (defaults: Linux=`vulkan` or `gl`, macOS=`metal`, Windows=`dx12,vulkan`). |

## On the `git` PATH dependency

Because writes go through `Command::new("git")`, gitara resolves
`git` via `PATH` like any other tool that types `git`. A binary
called `git` placed earlier in `PATH` (e.g. `~/.local/bin/git`)
would be invoked instead of the system one. This is the same trust
model that bash, every IDE, lazygit, GitHub Desktop, and `cargo`
itself rely on. If a user's `PATH` is hostile, every git operation
on the machine is compromised, not just gitara's. No special
mitigation is in place.
