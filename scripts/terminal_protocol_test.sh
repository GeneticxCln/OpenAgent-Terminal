#!/bin/bash

# Terminal Protocol & Standards Test Suite
# Tests for Sixel/Kitty graphics, OSC 8 hyperlinks, Unicode 15.0+, and true color support

echo "==============================================="
echo "Terminal Protocol & Standards Test Suite"
echo "==============================================="
echo ""

# Terminal Environment
echo "=== Terminal Environment ==="
echo "Terminal: $TERM"
echo "Terminal Program: $TERM_PROGRAM"
echo "Color Support: $(tput colors 2>/dev/null || echo 'unknown')"
echo ""

# Unicode 15.0+ Support Test
echo "=== Unicode 15.0+ Support Test ==="
echo "Modern Emoji (Unicode 15.0+): 🫨 🩷 🩵 🪿 🫎 🪼 🫏 🪽"
echo "Complex Scripts: العربية 中文 हिन्दी ಕನ್ನಡ ქართული မြန်မာ"
echo "Emoji with skin tones: 👋🏻 👋🏼 👋🏽 👋🏾 👋🏿"
echo "ZWJ sequences: 👨‍👩‍👧‍👦 🏳️‍🌈 🏳️‍⚧️"
echo ""

# True Color Support Test
echo "=== True Color Support Test ==="
echo "24-bit RGB gradient test:"
printf "Red gradient: "
for i in {0..255..8}; do
  printf "\033[48;2;%d;0;0m \033[0m" "$i"
done
echo ""

printf "Green gradient: "
for i in {0..255..8}; do
  printf "\033[48;2;0;%d;0m \033[0m" "$i"
done
echo ""

printf "Blue gradient: "
for i in {0..255..8}; do
  printf "\033[48;2;0;0;%dm \033[0m" "$i"
done
echo ""
echo ""

# OSC 8 Hyperlink Support Test
echo "=== OSC 8 Hyperlink Support Test ==="
echo -e "Clickable URL: \033]8;;https://github.com\033\\GitHub\033]8;;\033\\"
echo -e "Clickable file: \033]8;;file://$(pwd)\033\\$(pwd)\033]8;;\033\\"
echo -e "Email link: \033]8;;mailto:test@example.com\033\\Send Email\033]8;;\033\\"
echo ""

# Graphics Protocol Test
echo "=== Graphics Protocol Support Test ==="

# Create test image if it doesn't exist
if [ ! -f "test_gradient.png" ]; then
    echo "Creating test image..."
    if command -v magick >/dev/null 2>&1; then
        magick -size 100x100 gradient:red-blue test_gradient.png
    elif command -v convert >/dev/null 2>&1; then
        convert -size 100x100 gradient:red-blue test_gradient.png
    else
        echo "ImageMagick not available - cannot create test image"
    fi
fi

# Test Sixel support
echo "Testing Sixel protocol:"
if command -v img2sixel >/dev/null 2>&1 && [ -f "test_gradient.png" ]; then
    echo "Attempting to display image with Sixel..."
    img2sixel -w 50 -h 25 test_gradient.png 2>/dev/null || echo "Sixel display failed"
elif command -v magick >/dev/null 2>&1 && [ -f "test_gradient.png" ]; then
    echo "Attempting to display image with ImageMagick Sixel..."
    magick test_gradient.png -resize 50x50 sixel:- 2>/dev/null || echo "ImageMagick Sixel not supported"
else
    echo "No Sixel tools available"
fi
echo ""

# Test Kitty graphics protocol
echo "Testing Kitty graphics protocol:"
if [ -f "test_gradient.png" ]; then
    echo "Attempting to display image with Kitty protocol..."
    base64_img=$(base64 -w 0 test_gradient.png 2>/dev/null)
    if [ $? -eq 0 ]; then
        printf "\033_Ga=T,f=100,m=1;%s\033\\" "$base64_img" 2>/dev/null
        echo "Kitty protocol test sent (may not be visible if not supported)"
    else
        echo "Failed to encode image for Kitty protocol"
    fi
else
    echo "No test image available for Kitty protocol"
fi
echo ""

echo "=== Test Complete ==="
echo "If you see colors, emoji, and potentially images above, your terminal has good protocol support!"
echo "Clickable links (if supported) should be underlined or highlighted."
