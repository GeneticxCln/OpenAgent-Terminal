# [FEATURE] Complete Warp Integration Implementation

## Priority
🟠 **High** - Key differentiation feature

## Description
Multiple TODO comments throughout the Warp integration module indicate incomplete functionality. The Warp-style features are a key differentiator but many core operations are not yet implemented.

## Current Status
The `WarpIntegration` struct and related managers are in place, but critical functionality is missing:

### Missing Features (High Priority)
1. **Split Operations** - Split right/down, navigation, resizing
2. **Pane Management** - Zoom, close, cycle recent panes
3. **Session Restoration** - Loading saved sessions with terminals
4. **PTY Manager Integration** - Terminal process management
5. **Context Awareness** - Working directory, shell detection

### Locations with TODOs

#### Core Integration (`openagent-terminal/src/workspace/warp_integration.rs`)
- **Line 67**: PTY managers commented out - needs PtyManager implementation  
- **Line 112**: PTY managers HashMap initialization
- **Line 181**: Session restoration not implemented
- **Line 220-222**: PTY manager creation and storage
- **Lines 277, 299, 314, 321, 328, 335, 342, 349, 356, 363**: All split/pane operations stubbed out

#### AI Runtime Context (`openagent-terminal/src/ai_runtime.rs`)
- **Line 440**: Working directory context - "TODO: Get from context"
- **Line 441**: Shell kind context - "TODO: Get from context"

#### Display Backend (`openagent-terminal/src/display/mod.rs`)
- **Line 2324**: WGPU sprite implementation - "TODO: implement for WGPU backend"
- **Line 2335**: WGPU sprite filter - "TODO: implement for WGPU backend"

## Implementation Plan

### Phase 1: Core Infrastructure
1. **PTY Manager Integration**
   ```rust
   // Implement PtyManager for terminal process lifecycle
   pub struct PtyManager {
       pane_id: PaneId,
       process: Child,
       working_dir: PathBuf,
       shell_kind: ShellKind,
   }
   ```

2. **Context Management**
   ```rust
   // Add context tracking to WorkspaceManager
   pub struct PaneContext {
       working_directory: PathBuf,
       shell_kind: ShellKind,
       last_command: Option<String>,
       environment: HashMap<String, String>,
   }
   ```

### Phase 2: Split Operations
1. **Split Creation** - Implement `handle_split_right()` and `handle_split_down()`
2. **Navigation** - Implement `handle_navigate_pane()` with directional movement
3. **Resizing** - Implement `handle_resize_pane()` with proportional adjustments
4. **Pane Management** - Implement zoom, close, and recent pane cycling

### Phase 3: Session Management
1. **Session Restoration** - Implement `restore_session_terminals()`
2. **Context Persistence** - Save/restore working directories and shell states
3. **Terminal Recreation** - Properly recreate terminals with saved context

### Phase 4: Advanced Features
1. **Split Equalization** - Implement `handle_equalize_splits()`
2. **Recent Pane Cycling** - Implement `handle_cycle_recent_panes()`
3. **Enhanced UI Integration** - Complete WGPU sprite rendering

## Files to Modify

### Core Implementation
- `openagent-terminal/src/workspace/warp_integration.rs`
- `openagent-terminal/src/workspace/warp_split_manager.rs`
- `openagent-terminal/src/workspace/warp_tab_manager.rs`

### Context Integration
- `openagent-terminal/src/ai_runtime.rs`
- `openagent-terminal/src/workspace/mod.rs`

### Display Integration  
- `openagent-terminal/src/display/mod.rs`
- `openagent-terminal/src/display/warp_ui.rs` (if exists)

### PTY Integration
- `openagent-terminal-core/src/tty/` (new PtyManager)

## API Design Examples

```rust
// Enhanced split operations
impl WarpIntegration {
    fn handle_split_right(&mut self) -> WarpResult<bool> {
        let active_pane = self.get_active_pane()?;
        let new_pane = self.create_pane_with_context(&active_pane.context)?;
        self.split_manager.split_right(active_pane.id, new_pane.id, 0.5)?;
        Ok(true)
    }
    
    fn handle_navigate_pane(&mut self, direction: WarpNavDirection) -> WarpResult<bool> {
        let current_pane = self.get_active_pane()?;
        if let Some(target_pane) = self.split_manager.find_adjacent_pane(current_pane.id, direction)? {
            self.set_active_pane(target_pane)?;
            self.send_ui_update_event(WarpUiUpdateType::PaneChanged { pane_id: target_pane });
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

// Context-aware terminal creation
impl WarpIntegration {
    fn create_terminal_with_context(&mut self, pane_id: PaneId, context: &PaneContext) -> WarpResult<()> {
        let pty_manager = PtyManager::new(
            pane_id,
            &context.working_directory,
            context.shell_kind,
            &context.environment,
        )?;
        
        let terminal = self.create_terminal_for_pane(pane_id, &context.working_directory)?;
        
        self.terminals.insert(pane_id, terminal);
        self.pty_managers.insert(pane_id, Arc::new(pty_manager));
        
        Ok(())
    }
}
```

## Testing Requirements
- [ ] Split operations create proper layouts
- [ ] Navigation works in complex split arrangements  
- [ ] Session restoration recreates all terminals
- [ ] Context is properly tracked and restored
- [ ] PTY processes are managed correctly
- [ ] Memory cleanup works properly

## Labels
- `priority/high`
- `type/feature`
- `component/workspace`  
- `epic/warp-integration`

## Definition of Done
- [ ] All TODO comments in warp_integration.rs resolved
- [ ] Split operations fully functional
- [ ] Session restoration working
- [ ] PTY manager integration complete
- [ ] Context awareness implemented
- [ ] AI runtime has proper context access
- [ ] All tests passing
- [ ] Documentation updated
- [ ] Performance benchmarks acceptable
