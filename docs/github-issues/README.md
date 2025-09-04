# GitHub Issues for TODO/FIXME Items

This directory contains GitHub issue templates for all the high-priority TODO and FIXME items found in the OpenAgent Terminal codebase.

## Issue Templates Created

### Critical Priority 🔴
- **[001-critical-pty-drop-order.md](./001-critical-pty-drop-order.md)** 
  - Fix PTY drop order to prevent ConPTY deadlock
  - **Impact**: Production stability issue on Windows
  - **Files**: `openagent-terminal/src/main.rs:249`

### High Priority 🟠
- **[002-warp-integration-features.md](./002-warp-integration-features.md)**
  - Complete Warp Integration Implementation  
  - **Impact**: Key differentiation feature
  - **Files**: `openagent-terminal/src/workspace/warp_integration.rs` (multiple TODOs)

- **[003-wgpu-rendering-backend.md](./003-wgpu-rendering-backend.md)**
  - Complete WGPU Rendering Backend Implementation
  - **Impact**: Modern rendering pipeline and cross-platform compatibility
  - **Files**: `src/renderer/shaders/terminal.wgsl`, `src/renderer/wgpu_renderer.rs`

### Medium Priority 🟡
- **[004-persistent-data-storage.md](./004-persistent-data-storage.md)**
  - Implement Persistent Data Storage System
  - **Impact**: Plugin ecosystem and user data persistence  
  - **Files**: `openagent-terminal/src/components_init.rs:415,420`

- **[005-tab-bar-configuration.md](./005-tab-bar-configuration.md)**
  - Add Tab Bar Configuration Options
  - **Impact**: User experience improvement
  - **Files**: `openagent-terminal/src/display/tab_bar.rs:200`

## How to Use These Templates

1. **Copy the content** from the relevant .md file
2. **Create a new GitHub issue** in the repository
3. **Paste the template content** as the issue description
4. **Add the suggested labels** from the template
5. **Assign to appropriate milestone** based on priority

## Priority Mapping

| Priority | GitHub Labels | Typical Timeline |
|----------|---------------|-----------------|
| 🔴 Critical | `priority/critical`, `type/bug` | Immediate (Week 1) |
| 🟠 High | `priority/high`, `type/feature` | Short-term (Weeks 2-4) |
| 🟡 Medium | `priority/medium`, `type/enhancement` | Medium-term (Weeks 5-8) |

## Implementation Phases

### Phase 1: Stability (Week 1)
- Issue #001: PTY drop order fix

### Phase 2: Core Features (Weeks 2-4)  
- Issue #002: Warp integration
- Issue #003: WGPU rendering (basic functionality)

### Phase 3: Infrastructure (Weeks 5-6)
- Issue #004: Persistent storage
- Issue #003: WGPU rendering (completion)

### Phase 4: Polish (Weeks 7-8)
- Issue #005: Tab bar configuration
- Additional enhancements

## Related Documentation

- **[TODO_FIXME_SUMMARY.md](../TODO_FIXME_SUMMARY.md)** - Complete overview of all TODO/FIXME items
- **[IMPLEMENTATION_PROGRESS.md](../IMPLEMENTATION_PROGRESS.md)** - Current implementation status

## Template Structure

Each issue template includes:

- **Priority level** and impact assessment
- **Current status** and missing features
- **Implementation plan** with phases
- **Technical details** and code examples  
- **Files to modify** 
- **Testing requirements**
- **Definition of Done** checklist
- **Suggested labels** for GitHub

## Contributing

When creating issues from these templates:

1. Review the current codebase state (templates may become outdated)
2. Update file paths and line numbers if they've changed
3. Add any additional context from recent development
4. Consider breaking large issues into smaller, focused issues
5. Link related issues together for better tracking

## Maintenance

These templates should be updated when:

- TODO/FIXME comments are resolved
- File locations change significantly  
- New high-priority TODO items are discovered
- Implementation approaches change

---

*Last updated: 2024-12-19*
