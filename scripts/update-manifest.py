#!/usr/bin/env python3
"""Write latest.json, the update manifest the app reads (decision 213).

    python3 scripts/update-manifest.py --dir updater --version 0.2.0 --out updater/latest.json

`--dir` holds the macOS updater archive and its signature as the release workflow names
them: Method-Schedule_<version>_universal.app.tar.gz and the same with .sig, one app for
Intel and Apple silicon listed under both platforms. The archive's
download address is the release's own (github.com/<repo>/releases/download/v<version>/...),
so the manifest can only point at a file of the same release. The notes are
docs/releases/<version>.md when it exists.

It refuses a signature that isn't minisign's base64 text, a missing archive, a version that
doesn't match, and a tauri.conf.json without a public key (the app would refuse the update).
Tauri's manifest format: version, notes, pub_date, platforms.<os-arch>.{signature, url}.
"""

import argparse
import base64
import datetime
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO = "method-production-suite/method-scheduling"
CONF = ROOT / "crates/method-schedule-app/tauri.conf.json"
# One universal Mac app (SUITE-STANDARDS.md section 3, 2026-10-19) under both platforms: the
# updater asks for the entry of the chip it runs on.
PLATFORMS = {"darwin-aarch64": "universal", "darwin-x86_64": "universal"}


def fail(msg):
    print(f"update-manifest: {msg}", file=sys.stderr)
    sys.exit(1)


def signature(path):
    text = path.read_text().strip()
    try:
        inner = base64.b64decode(text, validate=True).decode("utf-8")
    except Exception:
        fail(f"{path.name} is not a base64 minisign signature")
    if "untrusted comment:" not in inner or "trusted comment:" not in inner:
        fail(f"{path.name} is not a minisign signature")
    return text


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", required=True)
    ap.add_argument("--version", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    key = json.loads(CONF.read_text()).get("plugins", {}).get("updater", {}).get("pubkey", "")
    if not key.strip():
        fail("tauri.conf.json has no updater public key: the app would refuse any update")
    folder = Path(a.dir)
    platforms = {}
    for platform, arch in PLATFORMS.items():
        name = f"Method-Schedule_{a.version}_{arch}.app.tar.gz"
        archive = folder / name
        sig = folder / f"{name}.sig"
        if not archive.is_file() or not sig.is_file():
            fail(f"missing {name} or its .sig in {folder}")
        platforms[platform] = {
            "signature": signature(sig),
            "url": f"https://github.com/{REPO}/releases/download/v{a.version}/{name}",
        }
    notes_file = ROOT / "docs/releases" / f"{a.version}.md"
    manifest = {
        "version": a.version,
        "notes": notes_file.read_text() if notes_file.is_file() else "",
        "pub_date": datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "platforms": platforms,
    }
    Path(a.out).write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"wrote {a.out}")


if __name__ == "__main__":
    main()
