#!/bin/bash
# Terminal Utilities - Leveraging Modern Terminal Protocols
# Uses existing tools: img2sixel, ImageMagick, built-in shell features

# Colors for status messages
declare -A COLORS=(
    ["success"]="48;2;0;200;0"
    ["error"]="48;2;200;0;0"
    ["warning"]="48;2;255;165;0"
    ["info"]="48;2;0;150;255"
)

# Status function with true colors and emoji
status() {
    local type="$1"
    local message="$2"
    local color="${COLORS[$type]:-48;2;128;128;128}"
    
    case "$type" in
        "success") echo -e "\033[${color}m ✅ \033[0m $message" ;;
        "error")   echo -e "\033[${color}m ❌ \033[0m $message" ;;
        "warning") echo -e "\033[${color}m ⚠️  \033[0m $message" ;;
        "info")    echo -e "\033[${color}m ℹ️  \033[0m $message" ;;
        *)         echo -e "\033[${color}m 📝 \033[0m $message" ;;
    esac
}

# Create clickable link
link() {
    local url="$1"
    local text="${2:-$url}"
    echo -e "\033]8;;$url\033\\$text\033]8;;\033\\"
}

# Display image in terminal using best available method
show_image() {
    local image_path="$1"
    local width="${2:-50}"
    local height="${3:-25}"
    
    if [ ! -f "$image_path" ]; then
        status error "Image file not found: $image_path"
        return 1
    fi
    
    if command -v img2sixel >/dev/null 2>&1; then
        status info "Displaying $image_path using Sixel protocol"
        img2sixel -w "$width" -h "$height" "$image_path"
    else
        status warning "img2sixel not available - image display not possible"
        return 1
    fi
}

# Create progress bar with true colors
progress_bar() {
    local current="$1"
    local total="$2"
    local width="${3:-40}"
    local message="${4:-Progress}"
    
    local percentage=$((current * 100 / total))
    local filled=$((current * width / total))
    local empty=$((width - filled))
    
    # Color calculation: red to green based on progress
    local red=$((255 - percentage * 255 / 100))
    local green=$((percentage * 255 / 100))
    
    printf "%s: [" "$message"
    
    # Filled portion
    for ((i=0; i<filled; i++)); do
        printf "\033[48;2;%d;%d;0m \033[0m" "$red" "$green"
    done
    
    # Empty portion
    for ((i=0; i<empty; i++)); do
        printf "\033[48;2;64;64;64m \033[0m"
    done
    
    printf "] %d%% (%d/%d)\n" "$percentage" "$current" "$total"
}

# File browser with clickable links
browse_directory() {
    local dir="${1:-.}"
    
    status info "Browsing directory: $dir"
    echo ""
    
    for item in "$dir"/*; do
        if [ -d "$item" ]; then
            echo -e "📁 $(link "file://$item" "$(basename "$item")/")"
        elif [ -f "$item" ]; then
            case "$item" in
                *.png|*.jpg|*.jpeg|*.gif|*.bmp)
                    echo -e "🖼️  $(link "file://$item" "$(basename "$item")")"
                    ;;
                *.txt|*.md|*.sh|*.py|*.js|*.html)
                    echo -e "📄 $(link "file://$item" "$(basename "$item")")"
                    ;;
                *)
                    echo -e "📄 $(link "file://$item" "$(basename "$item")")"
                    ;;
            esac
        fi
    done
}

# Create quick visual git status with colors and emoji
git_status_visual() {
    if [ ! -d .git ]; then
        status error "Not in a git repository"
        return 1
    fi
    
    echo "📊 Git Status Visualization:"
    echo ""
    
    # Modified files
    local modified=$(git diff --name-only | wc -l)
    # Staged files  
    local staged=$(git diff --cached --name-only | wc -l)
    # Untracked files
    local untracked=$(git ls-files --others --exclude-standard | wc -l)
    
    echo "Modified files:  $(printf "\033[48;2;255;140;0m %2d \033[0m" $modified) 📝"
    echo "Staged files:    $(printf "\033[48;2;0;200;0m %2d \033[0m" $staged) ✅"
    echo "Untracked files: $(printf "\033[48;2;128;128;128m %2d \033[0m" $untracked) ❓"
    
    if [ $((modified + staged + untracked)) -eq 0 ]; then
        status success "Working directory clean! 🎉"
    fi
}

# System info with emojis and colors
system_info() {
    echo "🖥️  System Information:"
    echo ""
    echo -e "OS: \033[38;2;0;150;255m$(uname -s)\033[0m"
    echo -e "Kernel: \033[38;2;0;150;255m$(uname -r)\033[0m"
    echo -e "Architecture: \033[38;2;0;150;255m$(uname -m)\033[0m"
    echo -e "Terminal: \033[38;2;255;165;0m$TERM_PROGRAM\033[0m"
    echo -e "Shell: \033[38;2;0;200;0m$SHELL\033[0m"
    
    # Memory usage with progress bar
    if command -v free >/dev/null 2>&1; then
        local mem_info=$(free -m | awk '/^Mem:/ {print $2, $3}')
        local total=$(echo $mem_info | cut -d' ' -f1)
        local used=$(echo $mem_info | cut -d' ' -f2)
        echo ""
        progress_bar "$used" "$total" 30 "Memory"
    fi
}

echo "🛠️  Terminal utility functions loaded!"
echo "Available functions:"
echo "  • status <type> <message>     - Colored status messages"
echo "  • link <url> [text]           - Create clickable links"
echo "  • show_image <path> [w] [h]   - Display images with Sixel"
echo "  • progress_bar <cur> <tot>    - Colored progress bars"
echo "  • browse_directory [path]     - Clickable file browser"
echo "  • git_status_visual          - Visual git status"
echo "  • system_info                - System info with colors"
