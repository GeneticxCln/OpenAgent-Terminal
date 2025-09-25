# Pane-synced input mirroring

This document describes the architecture and behavior of the pane synchronization feature that mirrors terminal input across panes in the active tab.

Overview
- When pane sync is ON for the active tab, OpenAgent Terminal mirrors input from the focused pane to all other panes in the same tab (excluding the focused pane itself).
- Mirrored content includes:
  - Key-typed bytes and escape sequences (keyboard input)
  - Pasted content (both bracketed and non-bracketed paste)
  - Bottom composer execution (Shift+Enter): pastes the command and sends Enter
- Mirroring is best-effort. Failures on individual panes are surfaced as a transient message indicating the number of panes that did not receive the input.

Key components
- ActionContext::write_terminal_input
  - New API added to route input through the pane-synced broadcast path when sync is enabled.
  - Always writes to the focused pane first via the PTY Notifier.
  - If the active tab has panes_synced = true, broadcasts the same bytes through WorkspaceManager::broadcast_input_active_tab.

- WorkspaceManager::broadcast_input_active_tab
  - Forwards to WarpIntegration when Warp is enabled, returning (attempted_writes, successful_writes).
  - If Warp is disabled, returns (0, 0).

- WarpIntegration::broadcast_input_active_tab
  - Identifies all pane IDs in the active tab’s split layout.
  - Skips:
    - The currently focused pane (to avoid double-writing)
    - Any panes in alternate screen (ALT_SCREEN) mode (to avoid corrupting TUIs)
  - Writes bytes to each pane’s PTY via PtyManagerCollection and counts successes.
  - Returns (attempted_writes, successful_writes).

- Paste path
  - Bracketed paste: mirrors the start marker (ESC [ 200 ~), filtered payload (ESC and ^C removed), and end marker (ESC [ 201 ~).
  - Non-bracketed paste: mirrors normalized payload bytes (\n converted to \r if bracketed=true brought into non-bracketed path).

Safety guards
- ALT_SCREEN skip: TUIs often run in alternate screen; mirroring raw typed bytes could break them. These panes are skipped for broadcasts.
- IME composition: IME preedit remains local to the focused pane.
- Mouse and cursor movement events are not mirrored.

Error handling and messages
- Broadcast returns (attempted, ok). If attempted > 0 and ok < attempted, a transient warning message is posted: “Pane sync: N pane(s) failed to receive input”.
- The focused pane’s input is always written first; broadcast failures do not interfere with the focused pane.

User controls
- Toggle pane sync via command palette (“TogglePaneSync”) or configured keybinding.
- A concise message indicates the new state: “Pane sync ON/OFF”.

Implementation entry points
- Keyboard input: input/keyboard.rs → Processor::key_input → ActionContext::write_terminal_input
- Paste operations: event.rs → ActionContext::paste (mirrors markers/payload)
- Composer execution: event.rs → ActionContext::execute_composer_command (paste + Enter via write_terminal_input)

Files changed
- openagent-terminal-core/src/tty/pty_manager.rs
  - Add PtyManager::write and PtyManagerCollection::write_to for PTY write access.
- openagent-terminal/src/workspace/warp_integration.rs
  - Add broadcast_input_active_tab returning (attempted, ok), implement ALT_SCREEN skip.
- openagent-terminal/src/workspace/mod.rs
  - Add WorkspaceManager::broadcast_input_active_tab wrapper.
- openagent-terminal/src/event.rs
  - Implement ActionContext::write_terminal_input and integrate with keyboard/paste/composer.
  - Add user-visible warning when some broadcast writes fail.
- openagent-terminal/src/input/keyboard.rs
  - Switch typing and key release to write_terminal_input.

Testing notes
- Manual testing:
  - Split panes in a tab; toggle pane sync ON; type/paste/Shift+Enter in one pane; observe mirrored input in other panes not running TUIs.
  - Toggle pane sync OFF; verify only focused pane receives input.
- Unit tests (planned):
  - Validate attempted/ok counts for broadcast.
  - Validate ALT_SCREEN skip behavior via a public API harness.
