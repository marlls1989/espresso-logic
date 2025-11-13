#!/bin/bash
set -e

echo "Building Rust to WebAssembly with Emscripten..."

# Build with Emscripten
cargo build --target wasm32-unknown-emscripten --release --bin espresso_demo

# Create public directory structure for Vite
mkdir -p public

# Copy the generated files to public directory (Vite serves this)
cp target/wasm32-unknown-emscripten/release/espresso_demo.js public/
cp target/wasm32-unknown-emscripten/release/espresso_demo.wasm public/

echo "WASM build complete! Files are in public/"
echo "Run 'npm run dev' to start the development server"

