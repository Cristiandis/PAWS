#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."

export MACOSX_DEPLOYMENT_TARGET=11.0

cargo build --release --locked --target aarch64-apple-darwin -p paws -p paws-gui
cargo build --release --locked --target x86_64-apple-darwin -p paws -p paws-gui

mkdir -p lipo
for bin in paws paws-gui; do
	lipo -create -output "lipo/$bin" \
		"target/aarch64-apple-darwin/release/$bin" \
		"target/x86_64-apple-darwin/release/$bin"
done

echo "Universal binaries written to lipo/"
