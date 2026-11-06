#!/usr/bin/env bash
# Installs minisign from its prebuilt release on GitHub, for scripts/verify-file.sh. The
# version and the file's SHA-256 are pinned here: a download that doesn't match is refused.
# A job that does this later holds the update key and the write token, so nothing here takes
# whatever a package archive serves that day. Linux x86_64 only: the release's checks run on
# ubuntu-latest.
#
# To move to a new version: download the release's Linux archive and its .minisig, check them
# with `minisign -Vm FILE -P RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3` (the key
# on minisign's own page), check the release is at least 14 days old (SUITE-STANDARDS.md
# section 6), and change VERSION and SHA below together.
set -euo pipefail

VERSION=0.12
SHA=9a599b48ba6eb7b1e80f12f36b94ceca7c00b7a5173c95c3efc88d9822957e73

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) ;;
  *) echo "install-minisign: no pinned build for $(uname -s) $(uname -m)" >&2; exit 1 ;;
esac

asset="minisign-$VERSION-linux.tar.gz"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
url="https://github.com/jedisct1/minisign/releases/download/$VERSION/$asset"
curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$work/$asset" "$url"

got="$(sha256sum "$work/$asset" | cut -d' ' -f1)"
if [ "$got" != "$SHA" ]; then
  echo "install-minisign: $asset has SHA-256 $got, expected $SHA" >&2
  exit 1
fi

tar -xzf "$work/$asset" -C "$work" minisign-linux/x86_64/minisign
bin="${CARGO_HOME:-$HOME/.cargo}/bin"
mkdir -p "$bin"
install -m 0755 "$work/minisign-linux/x86_64/minisign" "$bin/minisign"
"$bin/minisign" -v
