# Known issues

Findings from the parallel code-quality / dependency / security audit
that haven't been fixed yet. Severity tags: **CRITICAL** / **HIGH** /
**MEDIUM** / **LOW**.

## Security

### ~~**HIGH** — argument-injection via leading `-` in `git/ops.rs`~~ ✅ fixed
Every write op now passes `--end-of-options` (git 2.24+) before
user-controlled positional args, so a hostile name like `-fhostile`
is treated as a positional ref by git's parser instead of a flag.
Two regression tests in `git::ops::tests` cover the branch-create
and checkout paths.

### ~~**HIGH** — same issue in two ad-hoc shell-outs~~ ✅ fixed
`merge.rs` and `cherry_pick.rs` no longer build their own
`Command::new("git")`. Both route through `ops::merge(...)` and
`ops::cherry_pick(...)` which apply the `--end-of-options` separator
in one place.

### ~~**HIGH** — `add_remote` URL validation~~ ✅ mitigated by disable
The `add_remote` modal would otherwise accept any URL string, and a
URL like `-oProxyCommand=...` lands in the fetch config and
**executes on next fetch** (CVE-2017-1000117 class). Mitigation: the
UI entry point is removed in `views/modals/mod.rs` (renders a "feature
disabled" notice) and the env-var entry is removed in `app.rs`. The
modal code is kept under `#[allow(dead_code)]`.

**⚠ Before re-enabling the modal:** allow-list the URL scheme
(`https`, `ssh`, `git`, `file`); reject any URL whose first character
is `-`; reject any value that smells like an argument. Re-enabling
without this restores the RCE-class hole.

### ~~**MEDIUM** — `git` invoked unqualified~~ ℹ informational
The audit flagged that `Command::new("git")` does a PATH lookup, so a
hostile `~/.local/bin/git` would take over every write op. This is
true, but it's the same trust model every git-using tool on the
system has — bash, VS Code, IntelliJ, GitHub Desktop, even `cargo`
(which calls `git` for fetches) all resolve `git` the same way.
gitara doesn't add a new exposure beyond what every tool that types
"git" assumes. If a user's `PATH` is hostile, every git operation on
the machine is compromised, not just gitara's. Documented in
`README.md`. No code change planned.

### ~~**MEDIUM** — repo path not canonicalized~~ ℹ not applicable
The audit flagged that `GITARA_REPO` / `current_dir()` aren't
canonicalized before being used as the shell-out CWD. On a multi-
tenant or sandboxed system this matters; on a personal desktop tool
it doesn't. Gitara has no privilege boundary to escape from — the
user explicitly picks the repo. A `..` or symlink in the path is
exactly what the user typed; resolving it doesn't add a check, it
just changes the displayed string. No code change planned. (The
flag-injection compounding the audit cited is independently fixed by
`--end-of-options`.)

### **LOW** — `desktop_install.rs` `Exec=` line not quoted
`current_exe()` is interpolated unquoted. If the binary lives at a
path with spaces or shell metachars, the desktop entry parses
incorrectly. Not a vuln (Desktop Entry spec uses simple split), just
a robustness gap.

**Fix:** quote per the Desktop Entry spec, or reject paths with
control chars.

## Code quality

### **MEDIUM** — silent error swallowing in `app.rs`
`unstaged_files`, `staged_files`, `hunks_staged_for_path`,
`hunks_unstaged_for_path`, `read_status`, and `reload_commits` all
flatten errors via `unwrap_or_default()` / `.ok()`. A failing libgit2
call (corrupt index, permission denied) becomes "no changes" with no
signal — user can't tell when their working-tree state is being
misread.

**Fix:** surface those errors as toasts (the `Toast` type already
exists for this).

### ~~**HIGH (gap)** — `graph_layout.rs` has no tests~~ ✅ covered
Now has 10 unit tests covering: linear history, simple merges, octopus
merges, parallel lanes with off-screen parents, branch-tip /
root-commit lane flags, lane reuse after termination, max_column
uniformity, and the merge-collapse `terminating` vs `through`
invariant. Empty input edge case included.

### **MEDIUM** — `app.rs` is borderline god-file
619 lines holding 11 modal state structs, `AppState`, `ToastKind`,
`CtxMenu`, `boot()`, refresh helpers, *and* `root_view`.

**Fix:** split modal state into `app/modals.rs` and `root_view` into
`app/root.rs`.

### **LOW** — modal accessor pattern duplicated
Every modal under `views/modals/` defines its own
`state_get` / `state_mut` pair pattern-matching on `Modal::Variant`.

**Fix:** lift a generic accessor — e.g. an `impl Modal { fn
as_branch(&self) -> ... }` method per variant, or a small macro.

### **LOW** — `persist.rs` ProjectDirs naming mismatch
`directories::ProjectDirs::from("dev","ordo","Ordo")` while the crate
is called `gitara`. Settings land under an "Ordo" config dir on disk.

**Fix:** rename to `("dev","gitara","Gitara")` (will move the config
file — handle migration or document).

### **LOW** — `git/diff.rs:30-33` computes `diff.stats()` and discards
The per-file walk then duplicates the work in `per_file_stats`.

**Fix:** drop the stats call, or use it.

### **LOW** — `views/titlebar.rs:20` dead binding
`let _ = next_mode_text;` — leftover from previous design.

## Dependencies

### **LOW** — `paste` unmaintained (blocked by xilem/vello pin)
`paste 1.0.15` (transitive via `wgpu / vello / xilem`) —
RUSTSEC-2024-0436. Clears with `vello 0.5 → 0.8`, which is part of the
`xilem 0.4` upgrade. **Blocked because** `xilem 0.3` is pinned to the
vendored `masonry 0.3` — bumping `xilem` requires re-vendoring
masonry and re-applying our patches.

This is the only outstanding advisory; everything else (RUSTSEC-2025-
0140, RUSTSEC-2025-0021, RUSTSEC-2026-0008, RUSTSEC-2025-0141,
RUSTSEC-2024-0320) was closed by the `git2 0.20` and `gix 0.83` bumps
and by dropping `syntect`.

**Fix:** remove until the feature that needs them gets implemented.
