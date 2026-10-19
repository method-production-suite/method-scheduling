# Release 0.2.0: the checklist walk and the owner's tests

The suite's release checklist (`RELEASE-CHECKLIST.md` in the organization's `.github`
repository) for 0.2.0, with where each line stands. Commands: [RELEASING.md](../RELEASING.md).

## Before the tag

| Line | Status |
|---|---|
| Version bumped | Done: `0.2.0` in the root `Cargo.toml`. |
| Every check green | `cargo xtask ci` on the release pull request's head (in its description); `cooldown` and `deny` on GitHub; the full crate-age gate over the whole lock and the weekly full build (macOS, Windows, Linux) run by hand on the release commit before the tag. |
| Pictures retaken | Done: `node scripts/capture-screens.mjs` from Field Station; every picture is unchanged since they were retaken for the full Field Station. |
| Release notes written | Done: [docs/releases/0.2.0.md](../releases/0.2.0.md), `notes-check` says ready; two pictures; known gaps at the end. They are also What's New in the app. |
| Helpers named | Done: the Thanks section (no outside code this release). |
| README updated | Nothing changes: the platform table already says the Mac app is signed and notarized. |

## The release (the owner's go-ahead)

| Line | Status |
|---|---|
| Tag `v0.2.0` | Waiting on the owner's yes, through MOrg. The order: merge this pull request, run the weekly build by hand on that merge and see it green, tag `v0.2.0` on it, check the draft, publish, then MOrg syncs the site. |
| A person checks the draft | After the tag: the owner's tests below. |
| Installer names version-free | Built in. New this release: `SHA256SUMS` beside them. |
| Publish | The owner, after the draft check. |

## After publishing

| Line | Status |
|---|---|
| Update check works | The first real test: 0.1.0, installed, finds and installs 0.2.0 (test 1). |
| Website synced | MOrg syncs from the v0.2.0 tag. Going live with it: the new Field Station pictures and captions, the What's New box and the release notes page with 0.2.0. |
| Downloads checked live | After the sync. |
| Announced | The welcome discussion's release line, and the notes in Discussions. |

## The owner's tests (about ten minutes, on the Mac)

1. **The update.** With 0.1.0 still installed, open it and choose Help > Check for Updates.
   It should offer 0.2.0, download it, and restart into 0.2.0 (About says 0.2.0), then show
   What's New once.
2. **The sample.** Help > Open Sample opens "Field Station (sample)", untitled, with a line
   saying what it is. The board shows 18 days; the Day out of Days, Calendar and Red Flags
   tabs all have something in them.
3. **First launch, once.** Not testable on a Mac that ran 0.1.0 (by design it never springs
   the sample on someone who used an earlier version). Covered by tests instead.
4. **A checksum.** In Terminal, in the folder with the downloaded `.dmg`:
   `shasum -a 256 Method-Schedule-macOS.dmg` matches its line in the release's `SHA256SUMS`.
