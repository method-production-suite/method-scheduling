#!/usr/bin/env bash
# Checks FILE.minisig against the public key the app is built with (tauri.conf.json), using
# minisign itself, the way anyone can. Holds no secret. Usage: scripts/verify-file.sh FILE
set -euo pipefail

file="${1:?usage: verify-file.sh FILE}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pubkey="$(python3 - "$root/crates/method-schedule-app/tauri.conf.json" <<'PY'
import base64, json, sys
box = json.load(open(sys.argv[1]))["plugins"]["updater"]["pubkey"]
print(base64.b64decode(box).decode().splitlines()[1])
PY
)"
minisign -Vm "$file" -x "$file.minisig" -P "$pubkey"
