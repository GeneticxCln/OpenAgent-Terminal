#!/bin/bash
# Installation script for Enhanced Terminal Blocks

echo "🚀 Installing Enhanced Terminal Blocks..."
echo

# Check if zsh is installed
if ! command -v zsh &> /dev/null; then
    echo "❌ Error: zsh is not installed. Please install zsh first."
    exit 1
fi

# Check if .zshrc exists
if [[ ! -f "$HOME/.zshrc" ]]; then
    echo "Creating ~/.zshrc..."
    touch "$HOME/.zshrc"
fi

# Add source line to .zshrc if not already present
SOURCE_LINE="source ~/.config/terminal-blocks/enable-terminal-blocks.zsh"
if ! grep -q "$SOURCE_LINE" "$HOME/.zshrc"; then
    echo "Adding terminal blocks to ~/.zshrc..."
    echo "" >> "$HOME/.zshrc"
    echo "# Enhanced Terminal Blocks" >> "$HOME/.zshrc"
    echo "$SOURCE_LINE" >> "$HOME/.zshrc"
    echo "✅ Added to ~/.zshrc"
else
    echo "✓ Terminal blocks already in ~/.zshrc"
fi

echo
echo "✅ Installation complete!"
echo
echo "To activate the features:"
echo "  1. Restart your terminal, or"
echo "  2. Run: source ~/.zshrc"
echo
echo "Available commands:"
echo "  • tb_help  - Show all available features"
echo "  • ?        - Show keyboard shortcuts"
echo "  • cb_help  - Command blocks help"
echo "  • cpath_help - Clickable paths help"
echo "  • err_help - Error navigation help"
echo "  • kb_help  - Keybinding hints help"
echo
echo "Enjoy your enhanced terminal! 🎉"
