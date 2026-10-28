#!/bin/sh
# Build the browser engine and bundle the web edition into web/dist/index.html.
# Needs the wasm target once:  rustup target add wasm32-unknown-unknown
set -e
cd "$(dirname "$0")/.."
cargo build --profile web -p landfall_web --target wasm32-unknown-unknown
node web/build.mjs
