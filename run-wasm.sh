#!/bin/bash

set -e

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "wasm-pack is not installed. Installing..."
    cargo install wasm-pack
fi

# Build WASM
echo "Building WASM package..."
wasm-pack build --target web

# Serve the directory
echo ""
echo "Serving at http://localhost:8000"
echo "Press Ctrl+C to stop"
echo ""
npx serve -p 8000