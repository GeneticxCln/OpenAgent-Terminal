#!/bin/bash
# Test script for TypeScript development tools

set -euo pipefail

echo "Testing OpenAgent Terminal TypeScript Development Tools"
echo "=================================================="

# Check Node.js version
if ! command -v node &> /dev/null; then
    echo "❌ Node.js not found. Please install Node.js 18.0.0 or higher."
    echo "   Visit: https://nodejs.org/"
    exit 1
fi

NODE_VERSION=$(node --version)
echo "✅ Node.js version: $NODE_VERSION"

# Check npm
if ! command -v npm &> /dev/null; then
    echo "❌ npm not found. Please install npm."
    exit 1
fi

NPM_VERSION=$(npm --version)
echo "✅ npm version: $NPM_VERSION"

# Install dependencies
echo ""
echo "📦 Installing dependencies..."
npm install

# Run TypeScript checks
echo ""
echo "🔍 Running type checks..."
npm run type-check

# Run linting
echo ""
echo "🧹 Running ESLint..."
npm run lint

# Build TypeScript
echo ""
echo "🔨 Building TypeScript..."
npm run build

# Simple functionality test
echo ""
echo "🧪 Testing basic functionality..."

if [ -f "dist/security/security-lens.js" ]; then
    echo "✅ Security Lens built successfully"
else
    echo "❌ Security Lens build failed"
    exit 1
fi

if [ -f "dist/testing/fuzz-tester.js" ]; then
    echo "✅ Fuzz Tester built successfully"
else
    echo "❌ Fuzz Tester build failed"
    exit 1
fi

if [ -f "dist/testing/gpu-snapshot.js" ]; then
    echo "✅ GPU Snapshot Tester built successfully"
else
    echo "❌ GPU Snapshot Tester build failed"
    exit 1
fi

if [ -f "dist/sync/local-sync.js" ]; then
    echo "✅ Local Sync built successfully"
else
    echo "❌ Local Sync build failed"
    exit 1
fi

if [ -f "dist/workspace/workspace-manager.js" ]; then
    echo "✅ Workspace Manager built successfully"
else
    echo "❌ Workspace Manager build failed"
    exit 1
fi

echo ""
echo "✨ All TypeScript tools validated successfully!"
echo "   See src/README.md for usage examples."
