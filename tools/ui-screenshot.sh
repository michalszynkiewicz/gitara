#!/usr/bin/env bash
# Offscreen UI screenshot harness for gitara.
#
# Runs the debug build inside Xvfb (no visible window), captures the virtual
# root X window with ImageMagick's `import`, saves as PNG. Nothing appears
# on the user's desktop.
#
# Usage: ./tools/ui-screenshot.sh [output.png]
# Default output: /tmp/gitara-ui.png

set -e
out="${1:-/tmp/gitara-ui.png}"
cd "$(dirname "$0")/.."

# Pick a free X display number (99+).
for n in 99 100 101 102; do
    [ -e "/tmp/.X${n}-lock" ] && continue
    display=":${n}"
    break
done
: "${display:?no free X display}"

# Dimensions match main.rs default inner_size.
Xvfb "$display" -screen 0 1280x800x24 -nolisten tcp >/tmp/xvfb.log 2>&1 &
xvfb_pid=$!
trap 'kill $xvfb_pid 2>/dev/null; wait 2>/dev/null' EXIT

# Wait for Xvfb to be ready.
for _ in 1 2 3 4 5 6 7 8 9 10; do
    DISPLAY="$display" xdpyinfo >/dev/null 2>&1 && break
    sleep 0.2
done

# Start fluxbox so that winit's size hints (1280x800) get honored.
DISPLAY="$display" fluxbox >/tmp/fluxbox.log 2>&1 &
fluxbox_pid=$!
trap 'kill $fluxbox_pid 2>/dev/null; kill $xvfb_pid 2>/dev/null; wait 2>/dev/null' EXIT
sleep 0.5

# Force X11 backend for winit: unset Wayland, point DISPLAY at Xvfb.
# GITARA_HEADLESS tells main.rs to open borderless-fullscreen so the window
# fills the Xvfb screen (Xvfb has no WM to honor inner_size hints).
# Pass through any GITARA_* env vars the caller already set — the
# screenshot harness is how we exercise selection, modals, tabs
# headlessly.
env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE \
    DISPLAY="$display" GITARA_HEADLESS=1 \
    ${GITARA_REPO:+GITARA_REPO="$GITARA_REPO"} \
    ${GITARA_SELECT:+GITARA_SELECT="$GITARA_SELECT"} \
    ${GITARA_TAB:+GITARA_TAB="$GITARA_TAB"} \
    ${GITARA_DARK:+GITARA_DARK="$GITARA_DARK"} \
    ${GITARA_MODAL:+GITARA_MODAL="$GITARA_MODAL"} \
    ${RUST_LOG:+RUST_LOG="$RUST_LOG"} \
    ./target/debug/gitara >/tmp/gitara-stdout.log 2>/tmp/gitara-stderr.log &
gitara_pid=$!
trap 'kill $gitara_pid 2>/dev/null; kill $fluxbox_pid 2>/dev/null; kill $xvfb_pid 2>/dev/null; wait 2>/dev/null' EXIT

# Give the app time to boot, size its window, and render one frame.
sleep 3

DISPLAY="$display" import -window root "$out"

kill "$gitara_pid" 2>/dev/null || true
wait "$gitara_pid" 2>/dev/null || true
kill "$fluxbox_pid" 2>/dev/null || true
wait "$fluxbox_pid" 2>/dev/null || true
kill "$xvfb_pid" 2>/dev/null || true
wait "$xvfb_pid" 2>/dev/null || true
trap - EXIT

echo "$out"
