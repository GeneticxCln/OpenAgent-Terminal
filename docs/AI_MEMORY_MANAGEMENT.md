# AI Memory Management and Aggressive Cleanup Mode

This document describes the memory management system in OpenAgent Terminal's AI runtime, including the configurable aggressive cleanup mode for long-running sessions.

## Overview

The AI runtime includes a sophisticated memory monitoring and cleanup system that automatically manages memory usage for:

- SQLite AI conversation history
- JSONL history files
- WarpHistory manager caches
- AI embeddings and similarity caches
- Provider-specific caches (OpenAI, Anthropic, OpenRouter, Ollama)

## Memory Monitor Configuration

The system uses a `MemoryMonitorConfig` struct with the following default settings:

```rust
cleanup_threshold_bytes: 50 * 1024 * 1024,  // 50MB - triggers cleanup
check_interval: Duration::from_secs(300),    // 5 minutes - check frequency
min_cleanup_interval: Duration::from_secs(600), // 10 minutes - min time between cleanups
aggressive_threshold_bytes: 200 * 1024 * 1024, // 200MB - triggers aggressive mode
enable_background_cleanup: true,              // Enable background cleanup tasks
aggressive_mode: AggressiveCleanupMode::default(),
```

## Aggressive Cleanup Mode

When memory usage exceeds the aggressive threshold (default: 200MB), the system switches to aggressive cleanup mode with:

- More frequent memory checks (2 minutes instead of 5)
- Shorter minimum cleanup intervals (3 minutes instead of 10)
- Reduced retention times for various data stores
- More frequent vacuum operations on databases
- Smaller cache size limits

### Environment Variables for Configuration

You can configure aggressive cleanup mode using these environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENAGENT_AI_AGGRESSIVE_CLEANUP` | `false` | Enable/disable aggressive mode |
| `OPENAGENT_AI_AGGRESSIVE_THRESHOLD_MB` | `150` | Memory threshold in MB to trigger aggressive mode |
| `OPENAGENT_AI_AGGRESSIVE_CHECK_INTERVAL_SECS` | `120` | Check interval in seconds during aggressive mode |
| `OPENAGENT_AI_AGGRESSIVE_MIN_CLEANUP_INTERVAL_SECS` | `180` | Minimum time between cleanups in seconds |
| `OPENAGENT_AI_AGGRESSIVE_HISTORY_RETENTION_HOURS` | `24` | History retention time in hours during aggressive mode |
| `OPENAGENT_AI_AGGRESSIVE_CACHE_SIZE_LIMIT` | `1000` | Maximum cache size during aggressive mode |
| `OPENAGENT_AI_AGGRESSIVE_VACUUM_FREQUENCY` | `10` | Vacuum database every N cleanups instead of 20-50 |

### Examples

Enable aggressive cleanup mode:
```bash
export OPENAGENT_AI_AGGRESSIVE_CLEANUP=true
```

Set aggressive mode to trigger at 100MB:
```bash
export OPENAGENT_AI_AGGRESSIVE_THRESHOLD_MB=100
```

Set more frequent checks (every 60 seconds):
```bash
export OPENAGENT_AI_AGGRESSIVE_CHECK_INTERVAL_SECS=60
```

Reduce history retention to 12 hours:
```bash
export OPENAGENT_AI_AGGRESSIVE_HISTORY_RETENTION_HOURS=12
```

## Background Cleanup Tasks

The system performs several cleanup operations automatically:

### 1. SQLite History Cleanup
- Removes conversations older than configured age (7 days in background mode, 30 days normally)
- Limits total row count (10,000 in background mode, 20,000 normally)
- Removes invalid/corrupted entries
- Performs VACUUM operations to reclaim space
- Uses file locking to prevent concurrent cleanup conflicts

### 2. JSONL History Rotation
- Rotates files when they exceed size limits (2MB default)
- Removes old rotated files based on age (7 days in background mode)
- Keeps only recent rotated files (3 in background mode, 5 normally)

### 3. Cache Cleanup
- **WarpHistory caches**: Removes cache files older than 24 hours
- **AI embeddings**: Removes files older than 7 days from embeddings/, similarity_cache/, semantic_index/
- **Provider caches**: Removes files older than 3 days from openai_cache/, anthropic_cache/, etc.

### 4. Memory Statistics
The system tracks:
- Current memory usage estimation
- Peak memory usage
- Number of cleanup operations performed
- Last cleanup time

## Manual Memory Management

You can also manually trigger cleanup operations:

```rust
// Get current memory statistics
let stats = ai_runtime.get_memory_stats();
println!("Current usage: {} bytes", stats.current_usage);
println!("Peak usage: {} bytes", stats.peak_usage);
println!("Cleanups performed: {}", stats.cleanup_count);

// Manually trigger cleanup
ai_runtime.trigger_memory_cleanup();
```

## Configuration for Different Use Cases

### Development Environment
```bash
# More relaxed settings for development
export OPENAGENT_AI_AGGRESSIVE_CLEANUP=false
export OPENAGENT_AI_AGGRESSIVE_THRESHOLD_MB=500
```

### Production Long-Running Server
```bash
# Aggressive cleanup for 24/7 operation
export OPENAGENT_AI_AGGRESSIVE_CLEANUP=true
export OPENAGENT_AI_AGGRESSIVE_THRESHOLD_MB=100
export OPENAGENT_AI_AGGRESSIVE_CHECK_INTERVAL_SECS=60
export OPENAGENT_AI_AGGRESSIVE_HISTORY_RETENTION_HOURS=12
```

### Memory-Constrained Environment
```bash
# Very aggressive settings for limited memory
export OPENAGENT_AI_AGGRESSIVE_CLEANUP=true
export OPENAGENT_AI_AGGRESSIVE_THRESHOLD_MB=50
export OPENAGENT_AI_AGGRESSIVE_CHECK_INTERVAL_SECS=30
export OPENAGENT_AI_AGGRESSIVE_MIN_CLEANUP_INTERVAL_SECS=90
export OPENAGENT_AI_AGGRESSIVE_HISTORY_RETENTION_HOURS=6
export OPENAGENT_AI_AGGRESSIVE_CACHE_SIZE_LIMIT=500
```

## Implementation Details

### Thread Safety
- Uses atomic operations for memory tracking
- Employs mutex locks for cleanup coordination
- File-based locking prevents concurrent database cleanup

### Error Handling
- All cleanup operations are best-effort and non-blocking
- Errors are logged but don't interrupt normal operation
- Graceful degradation when cleanup operations fail

### Performance Considerations
- Background cleanup runs in separate thread
- Memory estimation is lightweight and fast
- Cleanup operations are optimized with batch processing
- Database operations use WAL mode for better concurrency

## Monitoring and Debugging

### Log Messages
The system produces informative log messages:
```
INFO AI memory cleanup task started
INFO Triggering background memory cleanup (usage: 52428800 bytes)
INFO SQLite cleanup: deleted 150 entries (old: 120, excess: 20, invalid: 10), 9850 rows remaining
INFO WarpHistory cache cleanup: removed 15 old cache files
INFO AI embeddings cache cleanup: removed 8 old cache files
INFO Provider cache cleanup: removed 12 old cache files
INFO Background cleanup operations completed
```

### Environment Variables for Other Settings
Additional environment variables affecting cleanup behavior:

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENAGENT_AI_HISTORY_SQLITE_MAX_AGE_DAYS` | `30` (7 in background) | SQLite history retention |
| `OPENAGENT_AI_HISTORY_SQLITE_MAX_ROWS` | `20000` (10000 in background) | Maximum SQLite rows |
| `OPENAGENT_AI_HISTORY_JSONL_MAX_AGE_DAYS` | `30` (7 in background) | JSONL file age limit |
| `OPENAGENT_AI_HISTORY_ROTATED_KEEP` | `5` (3 in background) | Number of rotated files to keep |
| `OPENAGENT_AI_HISTORY_MAX_BYTES` | `2097152` | JSONL file size before rotation |

This memory management system ensures that OpenAgent Terminal can run efficiently for extended periods while maintaining optimal performance and preventing memory-related issues.