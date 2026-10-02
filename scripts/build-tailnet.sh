#!/usr/bin/env bash
# SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
#
# Build the bundled Tailscale helper (tailnet/) for Tauri to ship as a sidecar.
#
#   scripts/build-tailnet.sh                 # this machine's Rust target
#   scripts/build-tailnet.sh universal-apple-darwin
#   scripts/build-tailnet.sh x86_64-pc-windows-msvc
#
# Tauri looks for `src-tauri/binaries/fieldnotes-tailnet-<target triple>[.exe]`
# at compile time, so this must run before `cargo build`, `cargo test` or
# `tauri build`. Pure Go (CGO_ENABLED=0), so any target builds from any host.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/src-tauri/binaries"
mkdir -p "$out"

triple="${1:-$(rustc -vV | sed -n 's/^host: //p')}"

build() { # <triple>
  local t="$1" goos goarch ext=""
  case "$t" in
    x86_64-unknown-linux-gnu)  goos=linux   goarch=amd64 ;;
    aarch64-unknown-linux-gnu) goos=linux   goarch=arm64 ;;
    x86_64-apple-darwin)       goos=darwin  goarch=amd64 ;;
    aarch64-apple-darwin)      goos=darwin  goarch=arm64 ;;
    x86_64-pc-windows-msvc)    goos=windows goarch=amd64 ext=".exe" ;;
    aarch64-pc-windows-msvc)   goos=windows goarch=arm64 ext=".exe" ;;
    *) echo "build-tailnet: no Go target for $t" >&2; exit 1 ;;
  esac
  echo "build-tailnet: $t"
  (cd "$root/tailnet" && CGO_ENABLED=0 GOOS="$goos" GOARCH="$goarch" \
    go build -trimpath -ldflags "-s -w" -o "$out/fieldnotes-tailnet-$t$ext" .)
}

if [ "$triple" = "universal-apple-darwin" ]; then
  # A universal app is built per architecture and then merged, so Tauri needs
  # all three: each half, and the merged one it bundles.
  build aarch64-apple-darwin
  build x86_64-apple-darwin
  lipo -create -output "$out/fieldnotes-tailnet-universal-apple-darwin" \
    "$out/fieldnotes-tailnet-aarch64-apple-darwin" \
    "$out/fieldnotes-tailnet-x86_64-apple-darwin"
else
  build "$triple"
fi
