# [CRITICAL] Fix PTY drop order to prevent ConPTY deadlock

## Priority
🔴 **Critical** - Can cause deadlocks in production

## Description
There's a critical ordering issue with PTY and ConPTY dropping that can cause deadlocks on Windows. The current implementation has a FIXME comment indicating this needs to be fixed at the type system level.

## Location
- **File**: `openagent-terminal/src/main.rs`
- **Line**: 249
- **Code context**:
```rust
// FIXME: Change PTY API to enforce the correct drop order with the typesystem.
//
// The fix is to ensure that processor is dropped first. That way, when window context (i.e.
// PTY) is dropped, it can ensure ConPTY is dropped before the conout pipe in the PTY drop
// order.
```

## Root Cause
The issue stems from both the processor and window context owning an `Arc<ConPTY>`. When dropped in the wrong order:
- ConPTY will deadlock if the conout pipe has already been dropped
- This happens when ConPTY is dropped after the conout pipe in the PTY drop order

## Current Workaround
The code currently ensures the processor is dropped before calling `FreeConsole()`, but this is a fragile solution that relies on manual drop ordering.

## Proposed Solution
Redesign the PTY API to enforce correct drop order through the type system:

1. **Use TypeState Pattern**: Create separate types for different PTY lifecycle states
2. **Linear Types**: Ensure PTY resources can only be consumed in the correct order
3. **RAII Guards**: Use guard types that automatically handle the drop order
4. **Arc Elimination**: Remove shared ownership of ConPTY where possible

## Implementation Ideas
```rust
// Example approach using typestate pattern
struct PtyBuilder { /* ... */ }
struct PtyActive { conpty: ConPTY, conout: Pipe }
struct PtyShutdown { /* conpty already cleaned up */ }

impl PtyActive {
    fn shutdown(self) -> PtyShutdown {
        // Guaranteed correct drop order
        drop(self.conout);
        drop(self.conpty);
        PtyShutdown { }
    }
}
```

## Files to Modify
- `openagent-terminal/src/main.rs`
- `openagent-terminal-core/src/tty/windows/mod.rs`
- `openagent-terminal-core/src/tty/windows/conpty.rs`

## Testing Requirements
- [ ] Test on Windows with ConPTY
- [ ] Stress test with rapid PTY creation/destruction
- [ ] Verify no deadlocks during shutdown
- [ ] Test graceful handling of unexpected termination

## Labels
- `priority/critical`
- `platform/windows`
- `component/pty`
- `type/bug`

## Definition of Done
- [ ] PTY drop order is enforced by the type system
- [ ] No manual drop order management required
- [ ] All existing tests pass
- [ ] New tests added to prevent regression
- [ ] Windows-specific testing completed
- [ ] Documentation updated to reflect the new API
