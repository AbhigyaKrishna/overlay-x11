#!/bin/bash
# Build script for stealth overlay with LD_PRELOAD hook

set -e

echo "=== Building Stealth Overlay ==="

# Build the LD_PRELOAD hook library
echo "Building LD_PRELOAD hook library..."
cd stealth_hook
cargo build --release
cd ..

# Copy the hook library to the main directory
echo "Copying hook library..."
cp stealth_hook/target/release/libstealth_hook.so .

# Build the main overlay application
echo "Building main overlay application..."
cargo build --release

echo ""
echo "=== Build Complete ==="
echo ""
echo "To run with full stealth mode:"
echo "  LD_PRELOAD=./libstealth_hook.so ./target/release/overlay-x11"
echo ""
echo "Or install and run as a service:"
echo "  sudo ./install.sh"
echo ""
