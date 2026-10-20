# Release 0.3.0: the checklist walk and the owner's tests

The suite's release checklist for 0.3.0, the first universal Mac release (decision 225),
under the faster release flow (CLAUDE.md, "Faster releases"; SUITE-STANDARDS.md section 6).
Commands: [RELEASING.md](../RELEASING.md).

## Before the tag

| Line | Status |
|---|---|
| Version bumped | Done: `0.3.0` in the root `Cargo.toml`. |
| Every check green | `cargo xtask ci` on the release pull request's head; the x86_64 tests under Rosetta; `cooldown` and `deny` on GitHub; the rehearsal (release.yml by hand on main, no tag) on the merge commit, which also runs the crate-age gate over the whole lock. |
| Pictures retaken | Nothing visible changed: the pictures are 0.2.0's. |
| Release notes written | Done: [docs/releases/0.3.0.md](../releases/0.3.0.md), `notes-check` says ready. |
| Helpers named | Done: the Thanks section. |
| README updated | Done: the status line and the platform tables say Intel and Apple silicon. |
| Launcher | Launcher 0.1.2, which accepts universal apps, is published (2026-10-19). |

## The release

| Line | Status |
|---|---|
| Tag `v0.3.0` | On the owner's yes through MOrg, after the rehearsal is green. One yes covers the tag and publishing. |
| The draft checked | Signed and notarized, Gatekeeper, every Mac executable exactly `x86_64 arm64`, `latest.json` with both `darwin-aarch64` and `darwin-x86_64` pointing at the one universal archive, `SHA256SUMS` matching, sizes sensible. Reported to MOrg; published unless the owner says stop. |
| Installer names version-free | Unchanged: `Method-Schedule-macOS.dmg` is now universal. |

## After publishing

| Line | Status |
|---|---|
| Update check works | 0.2.0 on Apple silicon finds and installs 0.3.0 (test 1). Intel: through the same manifest entry an Intel Mac asks for. |
| Website synced | MOrg syncs from the v0.3.0 tag. Going live: "Intel and Apple silicon" in the status line, Download, the platform table and the FAQ, plus #50's audit fixes (already live as a site-only fix) and the 0.3.0 notes. |
| Announced | The welcome discussion's release line. |

## The owner's tests (about five minutes, on the Mac)

1. **The update.** In 0.2.0, Help > Check for Updates offers 0.3.0, installs it and restarts
   into it (About says 0.3.0).
2. **Both chips in the app.** In Terminal:
   `lipo -archs "/Applications/Method Schedule.app/Contents/MacOS/method-schedule-app"`
   prints `x86_64 arm64`.
3. **The Intel half opens.** In Terminal:
   `arch -x86_64 "/Applications/Method Schedule.app/Contents/MacOS/method-schedule-app"`
   opens the window under Rosetta; Help > Open Sample shows Field Station; then quit. (The
   one check that needs a screen; everything else about the Intel half is tested headless.)
4. **A checksum.** `shasum -a 256 Method-Schedule-macOS.dmg` matches its line in the
   release's `SHA256SUMS`.
