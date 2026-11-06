#!/usr/bin/env bash
# Signs a release file with the app's update key, writing FILE.minisig, a plain minisign
# signature (SUITE-STANDARDS.md section 3). Fails if it can't: no key, a wrong password, or
# no signature. Runs beside the key, so it runs only the prebuilt `cargo-tauri`, which
# scripts/install-tauri-cli.sh installs checked. Needs TAURI_SIGNING_PRIVATE_KEY and
# TAURI_SIGNING_PRIVATE_KEY_PASSWORD. Usage: scripts/sign-file.sh FILE
set -euo pipefail

file="${1:?usage: sign-file.sh FILE}"
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  echo "::error::TAURI_SIGNING_PRIVATE_KEY is not set, so $file can't be signed." >&2
  exit 1
fi
rm -f "$file.sig" "$file.minisig"
cargo-tauri signer sign "$file" >/dev/null
if [ ! -s "$file.sig" ]; then
  echo "::error::Signing $file made no signature." >&2
  exit 1
fi
# The Tauri CLI writes the minisign signature in base64; minisign itself reads it plain.
base64 --decode < "$file.sig" > "$file.minisig"
rm -f "$file.sig"
