# Terminal Protocol Examples - Working Commands

## ✅ What Works Perfectly in Your Warp Terminal

### 1. True Color (24-bit RGB)
```bash
# Red text
echo -e "\033[38;2;255;0;0mRed text\033[0m"

# Green background  
echo -e "\033[48;2;0;255;0mGreen background\033[0m"

# Gradient bar
for i in {0..255..8}; do printf "\033[48;2;%d;0;0m \033[0m" "$i"; done; echo
```

### 2. OSC 8 Hyperlinks
```bash
# Clickable URL
echo -e "\033]8;;https://github.com\033\\GitHub\033]8;;\033\\"

# Clickable file
echo -e "\033]8;;file:///home/sasha\033\\Home Directory\033]8;;\033\\"

# Email link
echo -e "\033]8;;mailto:user@example.com\033\\Send Email\033]8;;\033\\"
```

### 3. Unicode 15.0+ & Emoji
```bash
# Modern emoji
echo "🫨🩷🩵🪿🫎🪼🫏🪽"

# Complex scripts
echo "العربية 中文 हिन्दी ಕನ್ನಡ ქართული"

# Emoji with modifiers
echo "👋🏻👋🏼👋🏽👋🏾👋🏿"
```

### 4. Sixel Graphics (with img2sixel)
```bash
# Display image
img2sixel -w 50 -h 25 image.png

# Create and display chart
magick -size 200x100 gradient:red-blue chart.png
img2sixel -w 60 -h 15 chart.png
```

## 🎯 Best Use Cases

1. **Status Dashboards**: Use colored emoji for system monitoring
2. **File Navigation**: Clickable file paths for easy navigation  
3. **Data Visualization**: Color-coded progress bars and charts
4. **Quick Image Preview**: View images directly in terminal
5. **Multilingual Support**: Perfect international text rendering

## 🛠️ Ready-to-Use Commands

```bash
# Colored status function
status_msg() { echo -e "\033[48;2;0;200;0m ✅ \033[0m $1"; }

# Quick link function  
make_link() { echo -e "\033]8;;$1\033\\$2\033]8;;\033\\"; }

# Simple image display
show_img() { img2sixel -w 50 -h 25 "$1"; }

# Progress bar
progress() { 
  local p=$1
  printf "Progress: ["
  for i in {1..20}; do
    if [ $i -le $((p/5)) ]; then
      printf "\033[48;2;0;200;0m█\033[0m"
    else
      printf "\033[48;2;64;64;64m░\033[0m" 
    fi
  done
  printf "] %d%%\n" "$p"
}
```

Your Warp Terminal has excellent support for all these protocols!
