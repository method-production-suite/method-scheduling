#!/usr/bin/env bash
# A signed and notarized Mac release, made on this Mac (decisions.md, entry 196):
#
#   1. the release app bundle and .dmg, with the Tauri bundler (`cargo tauri build`);
#   2. the app signed with the hardened runtime and a secure timestamp, by the Developer ID
#      in METHOD_SIGN_IDENTITY (the Tauri bundler signs it, then this script checks both);
#   3. the .dmg signed with a timestamp;
#   4. `codesign --verify --strict --deep` on the app and the .dmg;
#   5. the .dmg notarized (`xcrun notarytool submit --wait`) with the keychain profile in
#      METHOD_NOTARY_PROFILE, then stapled, with the app inside it;
#   6. `spctl --assess` and `xcrun stapler validate`.
#
# Nothing is uploaded anywhere but Apple's notary service. The results are in
# <cargo's build folder>/release-mac/: `Method Schedule.app` and `Method-Schedule-<version>-<arch>.dmg`.
#
# Needs Xcode's command line tools and the Tauri CLI, nothing else:
#   cargo install tauri-cli --locked --version 2.12.1
#
# Notarizing needs a keychain profile, made once by the owner (the app-specific password
# is typed at the prompt, never on the command line or in a file):
#   xcrun notarytool store-credentials method-schedule-notary \
#     --apple-id <the Apple ID> --team-id XSRB3D643J
# Without it the script stops after signing and verifying, before anything is sent.
#
# Settings, all optional:
#   METHOD_SIGN_IDENTITY   the signing identity (default: the suite's Developer ID, by team ID)
#   METHOD_NOTARY_PROFILE  the notarytool keychain profile (default method-schedule-notary)
#   METHOD_MAC_TARGET      a Rust target, such as universal-apple-darwin (default: this Mac's)
#   METHOD_SKIP_NOTARIZE=1 sign and verify only
#
set -euo pipefail
cd "$(dirname "$0")/.."

IDENTITY="${METHOD_SIGN_IDENTITY:-XSRB3D643J}"
PROFILE="${METHOD_NOTARY_PROFILE:-method-schedule-notary}"
TARGET="${METHOD_MAC_TARGET:-}"
TAURI_CLI_VERSION="2.12.1"
# Cargo's build folder (CARGO_TARGET_DIR or ./target), as cargo metadata says.
TARGET_DIR="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')"
OUT="$TARGET_DIR/release-mac"

say() { printf 'release-mac: %s\n' "$*"; }
die() { printf 'release-mac: %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || die "this makes a Mac release; run it on a Mac"
for tool in codesign xcrun spctl hdiutil security; do
  command -v "$tool" >/dev/null || die "$tool is missing; install Xcode's command line tools (xcode-select --install)"
done
if ! cargo tauri --version >/dev/null 2>&1; then
  die "the Tauri CLI is missing; install it with:
  cargo install tauri-cli --locked --version $TAURI_CLI_VERSION"
fi
have_cli="$(cargo tauri --version | awk '{print $2}')"
[ "$have_cli" = "$TAURI_CLI_VERSION" ] || say "note: Tauri CLI $have_cli, the pinned one is $TAURI_CLI_VERSION"

# The one version, from the workspace's Cargo.toml ([workspace.package]).
VERSION="$(awk '/^\[workspace.package\]/{p=1; next} /^\[/{p=0} p && /^version *=/{gsub(/[" ]/, "", $0); sub(/version=/, "", $0); print; exit}' Cargo.toml)"
[ -n "$VERSION" ] || die "no version under [workspace.package] in Cargo.toml"

security find-identity -v -p codesigning | grep -qF "\"$IDENTITY\"" \
  || die "no signing identity \"$IDENTITY\" in the keychain (security find-identity -v -p codesigning); set METHOD_SIGN_IDENTITY"

# 1 and 2. Build, bundle and sign. The bundler signs with the identity in
# APPLE_SIGNING_IDENTITY, hardened runtime on. Apple ID and API key variables are cleared
# so the bundler never notarizes on its own; this script does that, once, below. CI=true
# makes a plain .dmg without driving Finder to arrange its window.
say "building Method Schedule $VERSION${TARGET:+ for $TARGET}"
build_args=(tauri build --bundles app dmg)
bundle_dir="$TARGET_DIR/release/bundle"
if [ -n "$TARGET" ]; then
  build_args+=(--target "$TARGET")
  bundle_dir="$TARGET_DIR/$TARGET/release/bundle"
fi
(
  cd crates/method-schedule-app
  env -u APPLE_ID -u APPLE_PASSWORD -u APPLE_TEAM_ID \
    -u APPLE_API_KEY -u APPLE_API_ISSUER -u APPLE_API_KEY_PATH \
    -u APPLE_CERTIFICATE -u APPLE_CERTIFICATE_PASSWORD \
    APPLE_SIGNING_IDENTITY="$IDENTITY" CI=true \
    cargo "${build_args[@]}"
)

app_src="$bundle_dir/macos/Method Schedule.app"
[ -d "$app_src" ] || die "the bundler made no app at $app_src"
# The bundler names it `Method Schedule_<version>_<arch>.dmg`; the newest one.
dmg_src=""
for f in "$bundle_dir/dmg/"*"_${VERSION}_"*.dmg; do
  [ -e "$f" ] || continue
  if [ -z "$dmg_src" ] || [ "$f" -nt "$dmg_src" ]; then dmg_src="$f"; fi
done
[ -n "$dmg_src" ] || die "the bundler made no .dmg in $bundle_dir/dmg"

mkdir -p "$OUT"
arch="${TARGET:-$(uname -m)}"
arch="${arch%%-*}"
APP="$OUT/Method Schedule.app"
DMG="$OUT/Method-Schedule-$VERSION-$arch.dmg"
rm -rf "$APP" "$DMG"
ditto "$app_src" "$APP"
cp "$dmg_src" "$DMG"

# The app's signature: this identity, hardened runtime, a secure timestamp.
details="$(codesign --display --verbose=4 "$APP" 2>&1)"
grep -qF "Authority=$IDENTITY" <<<"$details" || die "the app is not signed by $IDENTITY:
$details"
grep -Eq 'flags=0x[0-9a-f]*\(.*runtime' <<<"$details" || die "the app's signature has no hardened runtime"
grep -q '^Timestamp=' <<<"$details" || die "the app's signature has no secure timestamp"
say "app signed: hardened runtime, timestamp $(grep '^Timestamp=' <<<"$details" | cut -d= -f2-)"

# 3. The .dmg, signed with a timestamp (a disk image takes no hardened runtime).
codesign --force --timestamp --sign "$IDENTITY" "$DMG"

# 4. Verify both, strictly, everything nested included.
codesign --verify --strict --deep --verbose=2 "$APP"
codesign --verify --strict --verbose=2 "$DMG"
# The app inside the .dmg is the one checked above.
mnt="$(mktemp -d)"
hdiutil attach -nobrowse -readonly -quiet -mountpoint "$mnt" "$DMG"
inside_ok=0
codesign --verify --strict --deep "$mnt/Method Schedule.app" && inside_ok=1
hdiutil detach -quiet "$mnt"
rmdir "$mnt" 2>/dev/null || true
[ "$inside_ok" = 1 ] || die "the app inside the .dmg does not verify"
say "signed and verified: $APP and $DMG"

if [ "${METHOD_SKIP_NOTARIZE:-}" = 1 ]; then
  say "METHOD_SKIP_NOTARIZE=1: stopping before notarization"
  exit 0
fi

# 5. Notarize. First make sure the keychain profile exists, before anything is sent.
if ! check="$(xcrun notarytool history --keychain-profile "$PROFILE" 2>&1)"; then
  if grep -q "No Keychain password item found" <<<"$check"; then
    die "no notarytool keychain profile \"$PROFILE\" yet. Nothing was uploaded.
Make it once (it asks for an app-specific password from appleid.apple.com; type it there):

  xcrun notarytool store-credentials $PROFILE --apple-id <the Apple ID> --team-id XSRB3D643J

then run this script again. The signed files above are ready."
  fi
  die "notarytool could not use the profile \"$PROFILE\":
$check"
fi
say "submitting $DMG to Apple's notary service (this waits for the answer)"
xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$DMG"
xcrun stapler staple "$APP"

# 6. What Gatekeeper will say.
spctl --assess --type execute -vv "$APP"
spctl --assess --type open --context context:primary-signature -vv "$DMG"
xcrun stapler validate "$DMG"
xcrun stapler validate "$APP"
say "done: $DMG (notarized and stapled)"
