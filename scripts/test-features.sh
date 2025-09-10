#!/usr/bin/env bash
# Simple test script to verify Security Lens feature gates work

set -e

echo "Testing Security Lens feature gates..."

# Test build WITHOUT Security Lens features
echo "1. Testing build without Security Lens features..."
cargo check -p openagent-terminal --no-default-features --features "wayland,x11" 2>/dev/null && {
    echo "✅ Build succeeds without Security Lens"
} || {
    echo "❌ Build fails without Security Lens"
}

# Test build WITH Security Lens core features
echo "2. Testing build with Security Lens core..."
cargo check -p openagent-terminal --features "security-lens" 2>/dev/null && {
    echo "✅ Build succeeds with Security Lens core"
} || {
    echo "❌ Build fails with Security Lens core"
}

# Test build with all Security Lens features
echo "3. Testing build with all Security Lens features..."
cargo check -p openagent-terminal --features "security-lens-full" 2>/dev/null && {
    echo "✅ Build succeeds with full Security Lens"
} || {
    echo "❌ Build fails with full Security Lens"
}

echo "Feature gate testing complete!"
