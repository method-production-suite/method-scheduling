#!/usr/bin/env bash
# The hand half of the crash recovery checks (decisions.md, entry 200). The saving code is
# tested without a window (crates/method-schedule-app/src/safety_crash.rs: a real kill -9
# of a child process, a full disk, a locked file, a file changed outside); this script
# covers what only the real app shows: the app killed mid-edit and the Recovered schedules
# dialog on the next start.
#
# It can't drive the window, so it sets things up and asks you to do each step:
#   1. builds the debug app and the command line,
#   2. writes the sample production to a temporary folder,
#   3. opens the app on it; you make an edit and wait five seconds,
#   4. kills the app with kill -9 (no clean exit),
#   5. opens the app again; you check the Recovered schedules dialog,
#   6. locks the file (read-only, and the Finder's Locked on macOS) and asks you to edit
#      and save: the app should refuse and keep a recovery copy,
#   7. changes the file on disk while the app is open: it should ask before anything.
#
# The app uses its usual data folder, so recovery copies from these steps appear there;
# Discard them in the dialog when done. Needs a screen and someone at it.
#
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -q -p method-schedule-app -p method-schedule-cli
# Cargo's build folder (CARGO_TARGET_DIR or ./target), as cargo metadata says.
target="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')"
app="$target/debug/method-schedule-app"
cli="$target/debug/method-schedule"
work="$(mktemp -d)"
file="$work/Saltmarsh Road.mtsc"
pid=""
cleanup() {
  if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; fi
  chflags nouchg "$file" 2>/dev/null || true
  chmod u+w "$file" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT
"$cli" sample --demo "$file" >/dev/null
echo "The sample is at $file"

step() { printf '\n%s\nPress Return when done. ' "$1"; read -r _; }
launch() { "$app" "$file" >/dev/null 2>&1 & pid=$!; sleep 2; }

launch
step "1. In the app: move a strip or rename a scene, then wait five seconds (autosave)."
step "2. Now Autosave off (File > Autosave), make another edit, and wait five seconds."
echo "Killing the app (kill -9)."
kill -9 "$pid"
wait "$pid" 2>/dev/null || true
pid=""

launch
step "3. Check: Recovered schedules lists Saltmarsh Road.mtsc. Open Recovered: the edit
   from step 2 is there, unsaved; Save asks before replacing the file. Turn Autosave back on."

kill "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true
pid=""
chmod a-w "$file"
if command -v chflags >/dev/null; then chflags uchg "$file"; fi
launch
step "4. The file is now locked and read-only. Edit and choose Save: the app should say the
   file is locked or read-only, write nothing, and keep a recovery copy. Then Undo back to
   the saved state, so nothing is unsaved for the next step."
chflags nouchg "$file" 2>/dev/null || true
chmod u+w "$file"

step "5. With the app open on the file and nothing unsaved, press Return here: the file
   will be changed on disk. Then click the app's window: it should ask Load Theirs or Keep
   Mine, and write nothing until you choose."
printf '\n' >>"$file"
step "6. Check the question came up, then quit the app."
echo "Done. Discard any recovered copies these steps left at the next start."
