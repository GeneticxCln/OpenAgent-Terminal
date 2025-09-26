#!/bin/bash
# Feature Activation Script
#
# This script removes all feature="never" gates to enable previously disabled functionality
# throughout the OpenAgent Terminal codebase.
#
# Usage:
#   chmod +x ./scripts/enable_features.sh
#   ./scripts/enable_features.sh

set -e

# Define colors for output
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Define source code path
SRC_PATH="$(pwd)"
if [[ ! -d "${SRC_PATH}/src" ]]; then
    echo -e "${RED}Error: This script must be run from the root of the OpenAgent-Terminal project.${NC}"
    exit 1
fi

echo -e "${GREEN}=== OpenAgent Terminal Feature Activation ===${NC}"
echo -e "${BLUE}This script will enable previously disabled features by removing feature=\"never\" gates.${NC}"
echo

# Find all files with feature="never" gates
echo -e "${YELLOW}Searching for feature=\"never\" gates...${NC}"
FILES_WITH_NEVER=$(grep -l -r --include="*.rs" 'feature.*=.*"never"\|cfg(feature.*never\|#\[cfg(not(feature.*never' "${SRC_PATH}")

if [[ -z "${FILES_WITH_NEVER}" ]]; then
    echo -e "${GREEN}No feature=\"never\" gates found. All features may already be enabled.${NC}"
    exit 0
fi

echo -e "${GREEN}Found $(echo "${FILES_WITH_NEVER}" | wc -l) files with disabled features.${NC}"
echo

# Function to process each file
process_file() {
    local file="$1"
    local rel_file="${file#${SRC_PATH}/}"
    local backup_file="${file}.bak"
    
    echo -e "${BLUE}Processing: ${rel_file}${NC}"
    
    # Create backup
    cp "$file" "$backup_file"
    
    # Replace feature="never" with feature="blocks"
    sed -i 's/feature *= *"never"/feature = "blocks"/g' "$file"
    
    # Replace cfg(feature = "never" with cfg(feature = "blocks"
    sed -i 's/cfg(feature *= *"never"/cfg(feature = "blocks"/g' "$file"
    
    # Replace #[cfg(not(feature = "never" with #[cfg(feature = "blocks"
    sed -i 's/#\[cfg(not(feature *= *"never"))]/#[cfg(feature = "blocks")]/g' "$file"
    
    # Count replacements
    local count=$(diff -U0 "$backup_file" "$file" | grep -c '^\+')
    
    if [[ $count -gt 0 ]]; then
        echo -e "${GREEN}  - Enabled $count features${NC}"
    else
        echo -e "${YELLOW}  - No replacements made${NC}"
        # Restore backup if no changes
        mv "$backup_file" "$file"
    fi
}

# Process each file
echo -e "${YELLOW}Enabling features in files...${NC}"
echo "${FILES_WITH_NEVER}" | while read -r file; do
    process_file "$file"
done

# Update Cargo.toml to add blocks feature
echo -e "${YELLOW}Updating Cargo.toml...${NC}"
if grep -q '\[features\]' "${SRC_PATH}/Cargo.toml"; then
    # Add blocks feature if [features] section exists
    if ! grep -q 'blocks *= *\[' "${SRC_PATH}/Cargo.toml"; then
        sed -i '/\[features\]/a blocks = []' "${SRC_PATH}/Cargo.toml"
        echo -e "${GREEN}Added 'blocks' feature to Cargo.toml${NC}"
    else
        echo -e "${YELLOW}The 'blocks' feature already exists in Cargo.toml${NC}"
    fi
else
    # Add [features] section if it doesn't exist
    cat >> "${SRC_PATH}/Cargo.toml" << 'EOL'

[features]
blocks = []
EOL
    echo -e "${GREEN}Added [features] section with 'blocks' feature to Cargo.toml${NC}"
fi

# Clean up backups
find "${SRC_PATH}" -name "*.rs.bak" -delete

echo
echo -e "${GREEN}=== Feature Activation Complete ===${NC}"
echo -e "${BLUE}All disabled features have been enabled via the 'blocks' feature.${NC}"
echo -e "${BLUE}To build with all features enabled:${NC}"
echo -e "${YELLOW}cargo build --features blocks${NC}"
echo
echo -e "${BLUE}To make this the default, add to .cargo/config.toml:${NC}"
echo -e "${YELLOW}[build]${NC}"
echo -e "${YELLOW}rustflags = [\"--cfg\", \"feature=\\\"blocks\\\"\"]${NC}"
echo