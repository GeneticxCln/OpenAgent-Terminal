#!/bin/bash

# Script to audit and identify non-Warp features that need to be removed
# This ensures compatibility with Warp Terminal only

echo "🔍 Auditing OpenAgent Terminal for non-Warp features..."
echo "=============================================="

# Define non-Warp features that need to be removed
NON_WARP_FEATURES=(
    "workflow"
    "notebook" 
    "sync"
    "ai.*agent"
    "anthropic"
    "ollama"
    "openrouter"
    "plugin"
    "wasi"
    "ide"
    "lsp"
    "security_lens"
    "privacy_content_filter"
    "enhanced_plugin_system"
)

# Define files/directories that should be removed completely
REMOVE_COMPLETELY=(
    "crates/workflow-engine"
    "openagent-terminal-sync"
    "openagent-terminal-ai"
    "crates/openagent-terminal-ide"
    "examples/workflows"
    "examples/plugins"
    "examples/ai-runtime-examples"
    "policies"
    "shell-integration"
    "docs/workflows.md"
    "docs/sync.md"
    "docs/ai.md"
    "docs/AI_*.md"
    "docs/plugins.md"
    "docs/AGENT_*.md"
    "docs/IDE_*.md"
    "docs/PRIVACY_*.md"
    "docs/SECURITY_LENS_*.md"
    "docs/WASI_*.md"
    "docs/adr/001-ai-architecture.md"
    "docs/adr/003-plugin-system.md"
    "docs/implementation/*WORKFLOW*"
    "docs/implementation/*AI*"
    "docs/implementation/*AGENT*"
    "docs/github-issues/008-workflow-foundations.md"
    "docs/github-issues/007-plugin-system-hardening.md"
    "docs/roadmaps/ai-terminal-roadmap.md"
)

# Define source files that need modification 
MODIFY_FILES=(
    "openagent-terminal/src/config"
    "openagent-terminal/src/display"
    "openagent-terminal/src/ai"
    "openagent-terminal/src/ide"
    "openagent-terminal/src/notebooks.rs"
    "openagent-terminal/src/workflow_persistence.rs"
    "openagent-terminal/src/ai_*.rs"
    "openagent-terminal/src/cli_ai.rs"
    "openagent-terminal/src/cli_sync.rs"
    "openagent-terminal/Cargo.toml"
    "Cargo.toml"
)

echo "📋 NON-WARP FEATURES TO REMOVE:"
echo "-------------------------------"

# Check for non-Warp features in codebase
for feature in "${NON_WARP_FEATURES[@]}"; do
    echo "🔍 Searching for: $feature"
    
    # Count occurrences
    count=$(find . -name "*.rs" -o -name "*.toml" -o -name "*.md" | xargs grep -i "$feature" | wc -l)
    
    if [ "$count" -gt 0 ]; then
        echo "  ❌ Found $count occurrences"
        echo "  📁 Files containing '$feature':"
        find . -name "*.rs" -o -name "*.toml" -o -name "*.md" | xargs grep -l -i "$feature" | head -10 | sed 's/^/     /'
        [ "$count" -gt 10 ] && echo "     ... and $(($count - 10)) more"
        echo
    else
        echo "  ✅ No occurrences found"
    fi
done

echo "📂 DIRECTORIES/FILES TO REMOVE COMPLETELY:"
echo "------------------------------------------"

for item in "${REMOVE_COMPLETELY[@]}"; do
    if [ -e "$item" ] || ls $item > /dev/null 2>&1; then
        echo "❌ $item"
    else
        echo "✅ $item (already removed)"
    fi
done

echo ""
echo "🔧 FILES REQUIRING MODIFICATION:"
echo "--------------------------------"

for pattern in "${MODIFY_FILES[@]}"; do
    if ls $pattern > /dev/null 2>&1; then
        echo "🔧 $pattern"
        # Show size of files to modify
        find $pattern -name "*.rs" -o -name "*.toml" 2>/dev/null | head -5 | while read file; do
            if [ -f "$file" ]; then
                lines=$(wc -l < "$file")
                echo "   📄 $file ($lines lines)"
            fi
        done
    fi
done

echo ""
echo "📊 SUMMARY:"
echo "----------"
echo "Total non-Warp features found: ${#NON_WARP_FEATURES[@]}"
echo "Directories/files to remove: ${#REMOVE_COMPLETELY[@]}"
echo "Source files to modify: ${#MODIFY_FILES[@]}"

echo ""
echo "⚠️  RECOMMENDED ACTIONS:"
echo "------------------------"
echo "1. 🗑️  Remove all workflow-related code and crates"
echo "2. 🗑️  Remove AI agent system (keep only basic Warp AI features)"
echo "3. 🗑️  Remove sync functionality entirely"
echo "4. 🗑️  Remove notebook features"
echo "5. 🗑️  Remove plugin/WASI system"
echo "6. 🗑️  Remove IDE features"
echo "7. 🗑️  Remove security lens and privacy filters"
echo "8. 🗑️  Remove multiple AI providers (keep only what Warp has)"
echo "9. 🔧 Update Cargo.toml files to remove dependencies"
echo "10. 🔧 Update configuration system to match Warp's structure"

echo ""
echo "💡 To automatically remove these features, run:"
echo "   ./scripts/remove_non_warp_features.sh"

echo ""
echo "✅ Audit complete!"