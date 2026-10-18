# Release 0.1.0: the checklist walk

Each line of the suite's release checklist (`RELEASE-CHECKLIST.md` in the organization's
`.github` repository) for 0.1.0, with where it stands. The commands are in
[RELEASING.md](../RELEASING.md).

## Before the tag

| Line | Status |
|---|---|
| Version bumped | Done: `0.1.0` in the root `Cargo.toml`, the one place. |
| Every check green | On the release pull request's CI: format, lint, tests, cargo-deny, the crate-age check, the site check and notes-check. |
| Pictures retaken | Done: `node scripts/capture-screens.mjs` from Field Station. Board and import changed (the full-width board and the new top bar); the day out of days and command line pictures came out byte-identical. Old and new compared side by side. |
| Release notes written | Done: [docs/releases/0.1.0.md](../releases/0.1.0.md), `notes-check` says ready. Three pictures; known gaps at the end. |
| Helpers named | Done: the Thanks section names masyarakatai. |
| README updated | Done: the macOS row of the platform table. Windows and Linux rows unchanged (built by CI, not yet installed by a person). |

## The release (the owner's go-ahead)

| Line | Status |
|---|---|
| Tag `v0.1.0` | Waiting on the owner: the eight release secrets on the public repository, then a yes to tag. The release pull request merges together with that yes, since merging it puts 0.1.0 in the site's What's new. |
| A person checks the draft | After the tag: install the `.dmg` on an Apple silicon Mac, open Field Station, check the update archive and `latest.json` are attached and signed. |
| Installer names version-free | Built in: `release.yml` names them `Method-Schedule-macOS.dmg` and so on. |
| Publish | The owner, after the draft check. |

## After publishing

| Line | Status |
|---|---|
| Update check works | Not testable for 0.1.0 (nothing older has the updater). First real test is 0.1.1. |
| Website synced | Ask MOrg to sync, and move `status` and `platforms` in `apps.toml`. |
| Downloads checked live | After the sync. |
| Announced | Edit the release line of the welcome discussion, and post the notes in Discussions. |
