# OpenAgent Terminal Development Roadmap

## Phase 1: WGPU Renderer Foundation
**Timeline: 4-6 weeks**

### 1.1 WGPU Integration
- [ ] Implement WGPU renderer backend with feature flag `--renderer=wgpu`
- [ ] Create abstraction layer for renderer switching (OpenGL/WGPU)
- [ ] Implement Wayland-specific surface creation and handling
- [ ] Implement macOS Metal backend support
- [ ] Ensure WGPU adapter selection and clear error reporting when adapter unavailable (no OpenGL fallback)

### 1.2 Performance Monitoring
- [ ] Integrate GPU timing markers using `wgpu::RenderPass::push_debug_group`
- [ ] Implement frame time tracking and statistics collection
- [ ] Create performance HUD overlay with toggle (Ctrl+Shift+P)
  - Frame time graph (last 60 frames)
  - GPU/CPU time breakdown
  - Draw call count
  - Memory usage indicators
- [ ] Add configuration for performance thresholds and alerts

**Key Dependencies:**
- wgpu = "0.20"
- raw-window-handle = "0.6"
- profiling = "1.0"

---

## Phase 2: Advanced Text Rendering
**Timeline: 3-4 weeks**

### 2.1 HarfBuzz Integration
- [ ] Integrate harfbuzz-rs or swash for text shaping
- [ ] Implement shaping cache with LRU eviction
- [ ] Support for:
  - Complex scripts (Arabic, Devanagari, Thai)
  - Ligatures (programming ligatures, typography)
  - Bidirectional text (RTL/LTR mixing)
  - Combining characters and diacritics

### 2.2 Font Management
- [ ] Implement font fallback chain system
- [ ] Add dedicated emoji font support (Noto Color Emoji, Apple Color Emoji)
- [ ] Create font configuration API:
  ```toml
  [fonts]
  primary = "JetBrains Mono"
  fallback = ["Noto Sans", "DejaVu Sans"]
  emoji = "Noto Color Emoji"
  ```
- [ ] Implement font feature controls (stylistic sets, variants)
- [ ] Add per-language font preferences

**Key Dependencies:**
- swash = "0.1" or harfbuzz-rs = "2.0"
- fontdb = "0.16"
- unicode-bidi = "0.3"

---

## Phase 3: AI Hardening + WGPU-Only (Completed)

Status: Complete. WGPU-only rendering enforced in code and CI; AI runtime hardened (sanitization, retry-after, context propagation) with tests; Windows PTY lifecycle drop-order fix added and tested; TypeScript dev tools stabilized. See guides/PHASE3_COMPLETE.md.

Legacy text below reflects original planning for Blocks 2.0; current project has reprioritized AI integration and WGPU stability as Phase 3.

## Phase 3: Blocks 2.0
**Timeline: 5-6 weeks**

### 3.1 Block Environment Controls
- [ ] Per-block configuration system:
  ```yaml
  block:
    id: "unique-id"
    directory: "/path/to/dir"
    env:
      NODE_ENV: "development"
      CUSTOM_VAR: "value"
    shell: "zsh"  # bash/zsh/fish/nu
  ```
- [ ] Inline environment editor UI component
- [ ] Shell switching without session restart
- [ ] Environment inheritance options

### 3.2 Block Organization
- [ ] Implement tagging system with SQLite backend
- [ ] Star/favorite blocks functionality
- [ ] Global search across block history:
  - Full-text search in outputs
  - Command search
  - Tag and metadata filtering
  - Date range filters
- [ ] Block grouping and collections

### 3.3 Import/Export
- [ ] Export formats:
  - JSON with full metadata
  - Markdown with code blocks
  - Shell script generation
- [ ] Import with conflict resolution
- [ ] Batch operations support
- [ ] Privacy controls (scrub sensitive data)

**Storage Schema:**
```sql
CREATE TABLE blocks (
  id TEXT PRIMARY KEY,
  command TEXT,
  output TEXT,
  directory TEXT,
  environment JSON,
  shell TEXT,
  created_at TIMESTAMP,
  tags JSON,
  starred BOOLEAN
);
```

---

## Phase 4: Plugin System MVP + Workflow Foundations
**Timeline: 4-5 weeks**

### 4.1 Plugin System MVP
- [ ] Minimal runtime using Wasmtime with strict sandboxing (WASM only)
- [ ] Host interface for logging, file read/write (scoped), notifications
- [ ] Command execution via host with Security Lens policy
- [ ] Manifest format (TOML) with permissions and capabilities
- [ ] Example plugin (hello-wasi) and tests

### 4.2 Workflow Definition
- [ ] TOML/YAML workflow parser
- [ ] Template syntax with parameters:
  ```yaml
  name: "Deploy Application"
  description: "Deploy to specified environment"
  parameters:
    - name: environment
      type: choice
      options: [dev, staging, prod]
    - name: version
      type: string
      default: "latest"
  steps:
    - name: "Build"
      command: "cargo build --release"
    - name: "Test"
      command: "cargo test"
    - name: "Deploy"
      command: "deploy.sh {{environment}} {{version}}"
  ```

### 4.3 Workflow UI
- [ ] Workflow launcher panel (Ctrl+Shift+W)
- [ ] Parameter input forms with validation
- [ ] Progress tracking and step visualization
- [ ] Workflow history and re-run capability
- [ ] Keyboard shortcuts for frequent workflows

### 4.4 AI Integration
- [ ] "Convert to Workflow" AI command
- [ ] Parameter extraction from natural language
- [ ] Workflow suggestion based on command history
- [ ] Template library with community workflows

---

## Phase 5: AI Capabilities
**Timeline: 4-5 weeks**

### 5.1 Context System
- [ ] Plugin architecture for context providers:
  ```rust
  trait ContextProvider {
      fn name(&self) -> &str;
      fn collect(&self) -> Result<Context>;
      fn sensitivity_level(&self) -> SensitivityLevel;
  }
  ```
- [ ] Built-in providers:
  - File tree (with .gitignore respect)
  - Git status/diff
  - Selected block outputs
  - Environment variables (filtered)
  - Recent command history
- [ ] Size limits and chunking strategies
- [ ] Sensitivity controls and PII detection

### 5.2 Provider Configuration
- [ ] Provider settings UI:
  - Model selection (GPT-4, Claude, Llama, etc.)
  - Temperature, max_tokens, top_p controls
  - System prompt templates
  - Cost tracking and limits
- [ ] Quick provider switching (Alt+1-9)
- [ ] Per-workspace AI settings
- [ ] Prompt template library

### 5.3 Async Runtime
- [ ] Migrate to async streaming with tokio
- [ ] Implement proper cancellation tokens
- [ ] Response streaming with backpressure
- [ ] Connection pooling and retry logic
- [ ] Rate limiting and quota management

**Configuration Example:**
```toml
[ai.providers.openai]
model = "gpt-4-turbo"
temperature = 0.7
max_tokens = 2000
api_key_env = "OPENAI_API_KEY"

[ai.providers.anthropic]
model = "claude-3-opus"
temperature = 0.5
max_tokens = 4000
api_key_env = "ANTHROPIC_API_KEY"

[ai.context]
max_size_kb = 32
include_git_diff = true
include_env_vars = false
```

---

## Phase 6: Plugin System
**Timeline: 6-8 weeks**

### 6.1 Plugin Host Architecture
- [ ] WASI-based plugin runtime using Wasmtime
- [ ] Alternative: IPC-based plugins with JSON-RPC
- [ ] Plugin manifest format:
  ```toml
  [plugin]
  name = "git-helper"
  version = "1.0.0"
  author = "example"

  [permissions]
  read_files = ["*.git"]
  network = false
  max_memory_mb = 50
  timeout_ms = 5000

  [capabilities]
  completions = true
  context_provider = true
  commands = ["git-smart-commit"]
  ```

### 6.2 Security Model
- [ ] Capability-based security system
- [ ] Resource quotas (CPU, memory, I/O)
- [ ] Sandbox file system access
- [ ] Network access controls
- [ ] Plugin signing and verification

### 6.3 Plugin API
- [ ] Version 1.0 API specification:
  ```rust
  // Plugin trait v1.0
  trait Plugin {
      fn metadata(&self) -> PluginMetadata;
      fn init(&mut self, config: Config) -> Result<()>;
      fn provide_completions(&self, input: &str) -> Vec<Completion>;
      fn collect_context(&self) -> Option<Context>;
      fn execute_command(&self, cmd: &str, args: &[String]) -> Result<Output>;
  }
  ```
- [ ] Plugin development SDK
- [ ] Testing framework for plugins
- [ ] Plugin marketplace/registry design

### 6.4 Example Plugin
- [ ] Create minimal example plugin
- [ ] Documentation and tutorial
- [ ] Plugin template repository
- [ ] CI/CD pipeline for plugin builds

---

## Implementation Priorities

### Critical Path (Must Have - Q1)
1. WGPU renderer with Wayland/Metal support
2. Basic HarfBuzz integration
3. Block environment controls
4. Basic workflow system

### High Priority (Should Have - Q2)
1. Performance HUD
2. Font fallback chains
3. Block tagging and search
4. AI context providers
5. Async AI runtime

### Medium Priority (Nice to Have - Q3)
1. Complex text shaping (RTL, combining)
2. Import/export blocks
3. Workflow AI parametrization
4. Basic plugin system

### Future Considerations (Q4+)
1. Full plugin marketplace
2. Advanced workflow orchestration
3. Distributed block storage
4. Collaborative features

---

## Technical Debt & Refactoring

### Before Starting
- [ ] Refactor current renderer abstraction
- [ ] Implement proper error handling strategy
- [ ] Set up performance benchmarking suite
- [ ] Create integration test framework
- [ ] Document public APIs

### Ongoing
- [ ] Maintain backward compatibility
- [ ] Performance regression tests
- [ ] Security audits for plugin system
- [ ] Accessibility improvements
- [ ] Documentation updates

---

## Success Metrics

### Performance
- Frame time < 16ms (60 FPS) for normal usage
- Text shaping cache hit rate > 90%
- Plugin execution overhead < 5ms
- Memory usage < 200MB baseline

### User Experience
- Workflow execution success rate > 95%
- AI response time < 2s for context collection
- Plugin installation success rate > 99%
- Zero data loss for block operations

### Developer Experience
- Plugin API stability (no breaking changes after v1.0)
- Documentation coverage > 80%
- Example coverage for all major features
- CI/CD pipeline < 10min

---

## Risk Mitigation

### Technical Risks
1. **WGPU Compatibility**: Maintain OpenGL fallback
2. **Font Rendering Complexity**: Start with basic shaping, iterate
3. **Plugin Security**: Strict sandboxing, gradual permission expansion
4. **Performance Regression**: Continuous benchmarking, feature flags

### Timeline Risks
1. **Scope Creep**: Strict MVP definitions per phase
2. **Dependencies**: Vendor critical libraries
3. **Testing Overhead**: Invest in automation early
4. **Platform Differences**: CI matrix testing

---

## Team & Resources

### Required Expertise
- Graphics programming (WGPU/Metal/Vulkan)
- Text rendering and typography
- Systems programming (Rust)
- Security and sandboxing
- UI/UX design
- AI/ML integration

### Tooling Requirements
- GPU profiling tools
- Font debugging utilities
- WASM toolchain
- Security scanning tools
- Performance monitoring

---

## Next Steps

1. **Week 1-2**: Set up WGPU scaffolding and feature flags
2. **Week 3-4**: Implement basic Metal/Wayland surfaces
3. **Week 5-6**: Integrate performance monitoring
4. **Week 7+**: Begin text shaping integration

## Notes

- All features should be feature-flagged during development
- Maintain compatibility with existing configuration
- Privacy-first approach for all features
- Regular security reviews for plugin system
- Community feedback loops at each phase milestone
