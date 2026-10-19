# Releasing Method Schedule

Every Method app follows one checklist:
**[RELEASE-CHECKLIST.md](https://github.com/method-production-suite/.github/blob/main/RELEASE-CHECKLIST.md)**
in the organization's `.github` repository (still a draft beside the suite's notes, not
published yet). Tick it in order for every release. This page adds Method Schedule's own
commands for each step; how the build, signing and updates work is in
[APP.md](APP.md), "Updates and releases".

## Before the tag

- **Version**: the one place is `[workspace.package] version` in the root `Cargo.toml`;
  every crate and the bundle take it from there. `release.yml` refuses a tag that doesn't
  match it.
- **Every check green** on the release commit:

  ```bash
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --locked -- -D warnings
  cargo test --workspace --locked
  node --test crates/method-schedule-app/page-tests/
  cargo deny check
  python3 scripts/check-crate-ages.py
  python3 scripts/notices.py --check
  python3 scripts/site.py --check
  python3 scripts/make-brand.py --check
  ```

- **Pictures retaken** from Field Station (`fixtures/field-station.mtsc`):

  ```bash
  node scripts/capture-screens.mjs
  ```

  It writes `assets/screens/` (the README) and `site/img/` (the site), and only pictures
  whose bytes changed; it says which. Compare each changed one with the last release's
  (`git diff --stat`, then open old and new side by side) and check nothing real is in
  frame. If the sample itself changed, write it again first:
  `cargo run -p method-schedule-cli -- sample --field-station fixtures/field-station.mtsc`
  (delete the file before).
- **Release notes**: `docs/releases/<version>.md` from
  [release-notes-template.md](release-notes-template.md), then

  ```bash
  python3 scripts/notes-check.py <version>
  python3 scripts/site.py
  ```

  The second puts the notes on the site's "What's new" box and release notes page.
- **Helpers named** in the notes' `## Thanks`, film knowledge included.
- **README**: the platform table (below the pictures) and anything the release changes.

## The release

- **The weekly full build on the release commit:** run `platforms` by hand (Actions >
  platforms > Run workflow, on main) and wait for green before tagging (SUITE-STANDARDS.md
  section 6).
- **Tag** on main: `git tag vX.Y.Z && git push origin vX.Y.Z` (as the owner's
  account, through the repository's credential helper). `release.yml` drafts the release
  with `Method-Schedule-macOS.dmg`, `Method-Schedule-windows-x64-setup.exe`,
  `Method-Schedule-linux-amd64.deb`, `Method-Schedule-linux-x86_64.AppImage` and, when the
  updater secrets are set, the update archive and `latest.json`, plus `SHA256SUMS`
  listing every one of them by bare name (checked before upload; the launcher verifies
  downloads against it).
- **Check the draft**, then publish it, as the checklist says.

## After publishing

- **Website**: ask MOrg to sync and deploy once the release is published (the website copies `site/` from the newest published release tag),
  and to change Method Schedule's `status` or `platforms` in `apps.toml` if they moved.
