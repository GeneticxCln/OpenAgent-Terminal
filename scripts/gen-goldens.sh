#!/usr/bin/env bash
set -euo pipefail

# Generate/update visual snapshot goldens for GL and WGPU backends on Linux.
# Usage:
#   scripts/gen-goldens.sh [gl|wgpu|both] [scenario1 scenario2 ...]
# If no scenarios are provided, a default set will be used.
# Requires: cargo, and on Linux an X server or xvfb-run.

BACKEND_ARG="both"
if [[ ${1-} == "gl" || ${1-} == "wgpu" || ${1-} == "both" ]]; then
  BACKEND_ARG=${1-}
  shift || true
fi

# Default scenarios
SCENARIOS=(
  confirm_overlay
  message_bar_error
  message_bar_warning
  search_bar_cursor
  folded_blocks
  split_panes
  split_overlay
  tab_bar
  tab_bar_hover
  tab_bar_drag
  tab_bar_overflow
  tab_bar_bottom
  tab_bar_reduce_motion
)

if [[ $# -gt 0 ]]; then
  SCENARIOS=("$@")
fi

run_with_display() {
  if command -v xvfb-run >/dev/null 2>&1; then
    xvfb-run -a "$@"
  else
    echo "xvfb-run not found; attempting to run without it. Ensure an X server is available." >&2
    "$@"
  fi
}

# Ensure golden/output dirs exist at repo root
mkdir -p tests/golden_images tests/snapshot_output

# GL goldens
if [[ "$BACKEND_ARG" == "gl" || "$BACKEND_ARG" == "both" ]]; then
  echo "==> Generating GL goldens"
  for scenario in "${SCENARIOS[@]}"; do
    SNAPSHOT_BACKEND=gl run_with_display \
      cargo run -p openagent-terminal --release --no-default-features -F x11,wayland,preview_ui --example snapshot_capture -- \
      --update-golden --scenario="$scenario" --backend=gl
  done
fi

# WGPU goldens (feature flag)
if [[ "$BACKEND_ARG" == "wgpu" || "$BACKEND_ARG" == "both" ]]; then
  echo "==> Generating WGPU goldens"
  for scenario in "${SCENARIOS[@]}"; do
    SNAPSHOT_BACKEND=wgpu run_with_display \
      cargo run -p openagent-terminal --release --no-default-features -F x11,wayland,preview_ui,wgpu --example snapshot_capture -- \
      --update-golden --scenario="$scenario" --backend=wgpu
  done
fi

echo "Done. Goldens written under tests/golden_images/."

