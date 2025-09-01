# AI Terminal Enhancement Roadmap

## Overview
This roadmap focuses on improving AI streaming performance, rendering efficiency, and user experience while maintaining privacy-first principles and architectural consistency.

## Phase 1: Core Infrastructure (Weeks 1-3)

### 1.1 Async Streaming Migration
**Priority: Critical**
**Impact: Performance, Responsiveness**

#### Implementation Tasks:
- [ ] Replace blocking reqwest clients with async implementation
- [ ] Implement unified cancellation mechanism using AtomicFlag
- [ ] Add connection abort via body drop to reduce stall times
- [ ] Create shared streaming abstraction layer for all providers

#### Technical Details:
```rust
// Unified streaming client interface
pub trait StreamingClient {
    async fn stream(&self, request: Request) -> Result<ResponseStream>;
    fn cancel(&self, flag: Arc<AtomicBool>);
}

// Cancellation mechanism
struct CancellationHandler {
    flag: Arc<AtomicBool>,
    body_handle: Option<ResponseBody>,
}
```

### 1.2 Provider-Specific Improvements

#### OpenAI Migration
- [ ] Track and implement Responses API migration
- [ ] Ensure consistent streaming delta handling
- [ ] Prepare infrastructure for future tool-calls support
- [ ] Document API differences and migration path

#### Anthropic Event Handling
- [ ] Implement comprehensive event type parsing:
  - `message_start`
  - `content_block_start`
  - `content_block_delta`
  - `content_block_stop`
  - `message_delta`
  - `message_stop`
- [ ] Add resilient error recovery for malformed events
- [ ] Create event type registry for future extensions

## Phase 2: Runtime & Event System (Weeks 3-5)

### 2.1 Backpressure Handling
**Priority: High**
**Impact: UI Responsiveness**

#### Implementation:
```rust
struct BackpressureManager {
    buffer: Vec<StreamChunk>,
    last_tick: Instant,
    target_fps: f32, // ~60Hz
}

impl BackpressureManager {
    fn should_flush(&self) -> bool {
        self.last_tick.elapsed() >= Duration::from_millis(16) // 60Hz
    }
    
    fn coalesce_chunks(&mut self) -> Option<CombinedChunk> {
        // Combine buffered chunks for single UI update
    }
}
```

### 2.2 Structured Tracing & Metrics
**Priority: Medium**
**Impact: Observability**

#### Data Structure:
```rust
struct AITrace {
    start_ts: SystemTime,
    provider: Provider,
    model: String,
    tokens_in: Option<u32>,
    tokens_out: Option<u32>,
    latency_ms: u64,
    stream_events: Vec<StreamEvent>,
}

struct TokenCounters {
    session_total_in: AtomicU64,
    session_total_out: AtomicU64,
    provider_totals: HashMap<Provider, (u64, u64)>,
}
```

## Phase 3: Rendering Layer (Weeks 5-7)

### 3.1 WGPU Backend Completion
**Priority: High**
**Impact: Performance, Future-proofing**

#### Tasks:
- [ ] Implement shared text cache interface
- [ ] Unify cache invalidation between GL/WGPU backends
- [ ] Ensure zero-copy rect transfers where possible
- [ ] Create performance benchmarks for both backends

#### Architecture:
```rust
trait TextCache {
    fn get_or_insert(&mut self, text: &str, style: TextStyle) -> CacheHandle;
    fn invalidate(&mut self, handle: CacheHandle);
    fn invalidate_all(&mut self);
}

trait RenderBackend {
    type TextCache: TextCache;
    fn render_frame(&mut self, frame: &Frame) -> Result<()>;
    fn transfer_rects(&mut self, rects: &[Rect]) -> TransferHandle;
}
```

## Phase 4: Testing Infrastructure (Weeks 6-8)

### 4.1 Integration Tests
**Priority: Critical**
**Impact: Reliability**

#### Test Coverage:
- [ ] AI streaming with httpmock for all providers
  - Success scenarios
  - Backpressure handling
  - Cancellation flows
  - Timeout conditions
  - Network errors
- [ ] Provider-specific edge cases
  - Malformed responses
  - Partial chunks
  - Authentication failures

#### Example Test:
```rust
#[tokio::test]
async fn test_openai_streaming_with_backpressure() {
    let mock_server = MockServer::start();
    mock_server.mock(|when, then| {
        when.method(POST)
            .path("/v1/chat/completions")
            .header("authorization", "Bearer test");
        then.status(200)
            .header("content-type", "text/event-stream")
            .body_stream(generate_slow_chunks());
    });
    
    // Test backpressure handling
    let client = create_client_with_backpressure();
    let result = client.stream_completion(&mock_server.url()).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().chunks_coalesced, true);
}
```

### 4.2 Snapshot Tests
**Priority: Medium**
**Impact: UI Consistency**

#### Test States:
- [ ] Loading state rendering
- [ ] Streaming chunks display
- [ ] Error state presentation
- [ ] Cancellation UI feedback
- [ ] Completion state

## Phase 5: Configuration & CLI (Week 7)

### 5.1 Verbose Logging Enhancement
**Priority: Low**
**Impact: Developer Experience**

#### Implementation:
- [ ] Add status banner in AI panel when verbose logging enabled
- [ ] Visual indicator (small icon/text) for debug mode
- [ ] Runtime toggle without restart

```rust
struct AIPanel {
    verbose_mode: bool,
    // ...
}

impl AIPanel {
    fn render_status_banner(&self) {
        if self.verbose_mode {
            render_text("🔍 Verbose logging active", StatusPosition::TopRight);
        }
    }
}
```

## Phase 6: UX Polish (Weeks 8-9)

### 6.1 Input Safety
**Priority: High**
**Impact: User Trust**

#### Features:
- [ ] "Paste to prompt without execution" (Enter + modifier)
- [ ] Separate "Execute" shortcut
- [ ] Confirmation dialog for potentially destructive operations
- [ ] Command preview before execution

### 6.2 Context Actions
**Priority: Medium**
**Impact: Productivity**

#### Implementation:
- [ ] "Explain this error" quick action on selected blocks
- [ ] Context menu integration
- [ ] Keyboard shortcut support
- [ ] Smart error detection and extraction

## Implementation Timeline

```
Week 1-2:  Async streaming migration
Week 3:    Provider-specific improvements
Week 4:    Backpressure & event system
Week 5-6:  WGPU backend work
Week 6-7:  Testing infrastructure
Week 7:    Configuration enhancements
Week 8-9:  UX polish & integration
Week 10:   Performance optimization & release prep
```

## Success Metrics

### Performance
- Stream latency: < 100ms to first token
- Cancellation response: < 50ms
- UI frame rate during streaming: stable 60fps
- Memory usage: < 10MB per active stream

### Reliability
- Test coverage: > 80% for AI modules
- Zero data loss on cancellation
- Graceful degradation on network issues

### User Experience
- No accidental command executions
- Clear visual feedback for all states
- Consistent behavior across providers

## Technical Debt Addressed
- Blocking I/O in streaming paths
- Inconsistent provider implementations
- Missing test coverage for edge cases
- Cache invalidation bugs between render backends

## Future Considerations

### Potential Extensions:
1. **Workflows Engine** (Future Phase)
   - TOML-based workflow definitions
   - UI runner for multi-step operations
   - Integration with existing command system

2. **Advanced Tool Calling**
   - Function calling support for OpenAI
   - Tool use for Anthropic
   - Custom action registry

3. **Performance Optimizations**
   - Response caching for identical queries
   - Predictive prefetching
   - Adaptive quality settings based on network

## Development Support Offer

Based on this roadmap, I can provide:

1. **Async Streaming Migration**
   - Draft implementation for OpenAI/Anthropic providers
   - Unified cancellation system
   - httpmock integration test suite

2. **Workflows Engine Specification**
   - TOML schema design
   - Minimal UI runner implementation
   - Integration points with existing architecture

3. **WGPU Implementation Plan**
   - Detailed milestones
   - Test harness setup
   - Performance benchmark framework

## Notes

### Why This Approach Works:
- **User-Centric**: Focuses on responsiveness, trust, and polish that users notice
- **Privacy-First**: Maintains local-first advantage while matching cloud UX
- **Architecturally Sound**: Maps to existing modules (renderer/AI/events/config)
- **Risk-Managed**: Incremental improvements with fallback paths
- **Measurable**: Clear metrics for success at each phase

### Key Principles:
1. **Performance**: Every interaction should feel instant
2. **Reliability**: Graceful handling of all failure modes
3. **Safety**: No destructive operations without explicit consent
4. **Consistency**: Uniform behavior across all AI providers
5. **Testability**: Comprehensive test coverage for confidence

---

*This roadmap is a living document. Update progress weekly and adjust timelines based on discoveries during implementation.*
