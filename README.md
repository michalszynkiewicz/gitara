# gitara

A native git GUI in Rust.

<img width="1924" height="1237" alt="image" src="https://github.com/user-attachments/assets/be128872-c392-4e6a-b4e0-455b9f2d679a" />


## Install

```sh
cargo build --release
cp target/release/gitara ~/.local/bin/      # ~/.local/bin is on PATH on most modern Linux distros
```

If `~/.local/bin` isn't on your `PATH`, add it to your shell rc:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

Or skip the copy and run the binary from `target/release/gitara` directly.

## Use

```sh
cd ~/your/repo
gitara
```

That's it — gitara opens the repo in the current working directory.

## Requirements

The `git` CLI must be on your `PATH`. gitara uses it for every write
operation (commit, push, fetch, branch, tag, …) so your `~/.gitconfig`,
hooks, signing key, and credential helpers all just work.

## Theme

gitara starts in **dark** mode by default. Click the **Dark** / **Light**
button in the top-right of the titlebar to switch — most of the UI updates
live, but a few label colors stay on the old palette until the next
restart, so launch gitara again to apply the new theme cleanly.

To force a theme at startup (skipping the persisted preference):

```sh
GITARA_DARK=1 gitara     # dark
GITARA_LIGHT=1 gitara    # light
```

## Linux first run

On its first launch on Linux, gitara writes a desktop entry and icons
under `~/.local/share/...` so your dock / taskbar / app menu picks up
the gitara icon. After that you can launch it from your launcher
instead of a terminal.

## More

* [docs/implementation.md](docs/implementation.md) — architecture,
  environment variables, design notes.
* [ISSUES.md](ISSUES.md) — known issues and rough edges.
