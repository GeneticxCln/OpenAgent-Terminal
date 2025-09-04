// Advanced shaped text caching system for OpenAgent Terminal
// Optimized for terminal text rendering patterns with LRU eviction

use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use lru::LruCache;
use std::num::NonZeroUsize;
use ahash::RandomState;

use super::harfbuzz::{ShapedText, TextDirection};
use crate::config::font::TextShapingConfig;

/// Cache key for shaped text segments
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapedTextCacheKey {
    /// The text content
    pub text: String,
    /// Font family name
    pub font_family: String,
    /// Font size in points
    pub font_size_bits: u32,
    /// Font style flags (bold, italic, etc.)
    pub font_flags: u8,
    /// Text shaping features enabled
    pub features_hash: u64,
}

impl Hash for ShapedTextCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.font_family.hash(state);
        self.font_size_bits.hash(state);
        self.font_flags.hash(state);
        self.features_hash.hash(state);
    }
}

/// Cached shaped text entry
#[derive(Debug, Clone)]
pub struct CachedShapedText {
    pub shaped_text: ShapedText,
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub access_count: u64,
}

/// Statistics for the shaped text cache
#[derive(Debug, Clone)]
pub struct ShapedTextCacheStats {
    pub cache_size: usize,
    pub max_cache_size: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub hit_ratio: f64,
    pub average_access_time_ms: f64,
}

/// Line-based cache for common terminal text patterns
#[derive(Debug)]
pub struct TerminalLineCache {
    /// Cache for complete terminal lines
    line_cache: LruCache<String, Arc<ShapedText>, RandomState>,
    /// Cache for common text fragments (words, symbols)
    fragment_cache: LruCache<String, Arc<ShapedText>, RandomState>,
    /// Recently used patterns for predictive caching
    pattern_tracker: VecDeque<String>,
    /// Statistics
    stats: CacheStats,
}

#[derive(Debug, Default)]
struct CacheStats {
    line_hits: u64,
    line_misses: u64,
    fragment_hits: u64,
    fragment_misses: u64,
    evictions: u64,
}

impl TerminalLineCache {
    /// Create a new terminal line cache
    pub fn new(max_lines: usize, max_fragments: usize) -> Self {
        Self {
            line_cache: LruCache::new(
                NonZeroUsize::new(max_lines).unwrap_or(NonZeroUsize::new(1000).unwrap())
            ),
            fragment_cache: LruCache::new(
                NonZeroUsize::new(max_fragments).unwrap_or(NonZeroUsize::new(5000).unwrap())
            ),
            pattern_tracker: VecDeque::with_capacity(100),
            stats: CacheStats::default(),
        }
    }

    /// Get a cached line
    pub fn get_line(&mut self, line: &str) -> Option<Arc<ShapedText>> {
        if let Some(shaped) = self.line_cache.get(line) {
            self.stats.line_hits += 1;
            Some(shaped.clone())
        } else {
            self.stats.line_misses += 1;
            None
        }
    }

    /// Cache a shaped line
    pub fn cache_line(&mut self, line: String, shaped: Arc<ShapedText>) {
        self.line_cache.put(line.clone(), shaped);
        self.track_pattern(line);
    }

    /// Get a cached fragment
    pub fn get_fragment(&mut self, fragment: &str) -> Option<Arc<ShapedText>> {
        if let Some(shaped) = self.fragment_cache.get(fragment) {
            self.stats.fragment_hits += 1;
            Some(shaped.clone())
        } else {
            self.stats.fragment_misses += 1;
            None
        }
    }

    /// Cache a shaped fragment
    pub fn cache_fragment(&mut self, fragment: String, shaped: Arc<ShapedText>) {
        self.fragment_cache.put(fragment, shaped);
    }

    /// Track a pattern for predictive caching
    fn track_pattern(&mut self, pattern: String) {
        if self.pattern_tracker.len() >= self.pattern_tracker.capacity() {
            self.pattern_tracker.pop_back();
        }
        self.pattern_tracker.push_front(pattern);
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> TerminalLineCacheStats {
        let total_hits = self.stats.line_hits + self.stats.fragment_hits;
        let total_requests = total_hits + self.stats.line_misses + self.stats.fragment_misses;

        TerminalLineCacheStats {
            line_cache_size: self.line_cache.len(),
            fragment_cache_size: self.fragment_cache.len(),
            line_hit_ratio: if self.stats.line_hits + self.stats.line_misses > 0 {
                self.stats.line_hits as f64 / (self.stats.line_hits + self.stats.line_misses) as f64
            } else {
                0.0
            },
            fragment_hit_ratio: if self.stats.fragment_hits + self.stats.fragment_misses > 0 {
                self.stats.fragment_hits as f64 / (self.stats.fragment_hits + self.stats.fragment_misses) as f64
            } else {
                0.0
            },
            overall_hit_ratio: if total_requests > 0 {
                total_hits as f64 / total_requests as f64
            } else {
                0.0
            },
            eviction_count: self.stats.evictions,
        }
    }

    /// Clear all caches
    pub fn clear(&mut self) {
        self.line_cache.clear();
        self.fragment_cache.clear();
        self.pattern_tracker.clear();
        self.stats = CacheStats::default();
    }
}

/// Statistics for terminal line cache
#[derive(Debug, Clone)]
pub struct TerminalLineCacheStats {
    pub line_cache_size: usize,
    pub fragment_cache_size: usize,
    pub line_hit_ratio: f64,
    pub fragment_hit_ratio: f64,
    pub overall_hit_ratio: f64,
    pub eviction_count: u64,
}

/// Advanced shaped text cache with multiple caching strategies
pub struct ShapedTextCache {
    /// Primary LRU cache for shaped text
    primary_cache: Arc<RwLock<LruCache<ShapedTextCacheKey, CachedShapedText, RandomState>>>,

    /// Secondary cache optimized for terminal line patterns
    terminal_cache: Arc<RwLock<TerminalLineCache>>,

    /// Cache configuration
    config: ShapedTextCacheConfig,

    /// Global cache statistics
    stats: Arc<RwLock<GlobalCacheStats>>,
}

/// Configuration for shaped text caching
#[derive(Debug, Clone)]
pub struct ShapedTextCacheConfig {
    /// Maximum entries in primary cache
    pub max_primary_entries: usize,
    /// Maximum terminal lines to cache
    pub max_terminal_lines: usize,
    /// Maximum terminal fragments to cache
    pub max_terminal_fragments: usize,
    /// Enable terminal-optimized caching
    pub enable_terminal_cache: bool,
    /// Enable predictive caching based on patterns
    pub enable_predictive_caching: bool,
    /// TTL for cache entries in seconds
    pub entry_ttl_seconds: u64,
}

impl Default for ShapedTextCacheConfig {
    fn default() -> Self {
        Self {
            max_primary_entries: 5000,
            max_terminal_lines: 1000,
            max_terminal_fragments: 3000,
            enable_terminal_cache: true,
            enable_predictive_caching: true,
            entry_ttl_seconds: 300, // 5 minutes
        }
    }
}

#[derive(Debug, Default)]
struct GlobalCacheStats {
    primary_hits: u64,
    primary_misses: u64,
    terminal_hits: u64,
    terminal_misses: u64,
    total_evictions: u64,
    cache_creation_time: Option<Instant>,
}

impl ShapedTextCache {
    /// Create a new shaped text cache
    pub fn new(config: ShapedTextCacheConfig) -> Self {
        let primary_cache = Arc::new(RwLock::new(
            LruCache::with_hasher(
                NonZeroUsize::new(config.max_primary_entries).unwrap(),
                RandomState::new()
            )
        ));

        let terminal_cache = Arc::new(RwLock::new(
            TerminalLineCache::new(config.max_terminal_lines, config.max_terminal_fragments)
        ));

        let mut stats = GlobalCacheStats::default();
        stats.cache_creation_time = Some(Instant::now());

        Self {
            primary_cache,
            terminal_cache,
            config,
            stats: Arc::new(RwLock::new(stats)),
        }
    }

    /// Get shaped text from cache
    pub fn get(&self, key: &ShapedTextCacheKey) -> Option<ShapedText> {
        // Try terminal cache first for line-like patterns
        if self.config.enable_terminal_cache && self.is_terminal_like(&key.text) {
            if let Ok(mut cache) = self.terminal_cache.write() {
                if let Some(shaped) = cache.get_line(&key.text) {
                    if let Ok(mut stats) = self.stats.write() {
                        stats.terminal_hits += 1;
                    }
                    return Some((*shaped).clone());
                }
            }
        }

        // Try primary cache
        if let Ok(mut cache) = self.primary_cache.write() {
            if let Some(entry) = cache.get(key) {
                // Check TTL
                let now = Instant::now();
                if now.duration_since(entry.created_at).as_secs() <= self.config.entry_ttl_seconds {
                    if let Ok(mut stats) = self.stats.write() {
                        stats.primary_hits += 1;
                    }

                    // Update access info
                    let mut updated_entry = entry.clone();
                    updated_entry.last_accessed = now;
                    updated_entry.access_count += 1;
                    cache.put(key.clone(), updated_entry);

                    return Some(entry.shaped_text.clone());
                } else {
                    // Entry expired, remove it
                    cache.pop(key);
                }
            }
        }

        // Cache miss
        if let Ok(mut stats) = self.stats.write() {
            if self.config.enable_terminal_cache && self.is_terminal_like(&key.text) {
                stats.terminal_misses += 1;
            } else {
                stats.primary_misses += 1;
            }
        }

        None
    }

    /// Store shaped text in cache
    pub fn put(&self, key: ShapedTextCacheKey, shaped_text: ShapedText) {
        let now = Instant::now();

        // Store in terminal cache if appropriate
        if self.config.enable_terminal_cache && self.is_terminal_like(&key.text) {
            if let Ok(mut cache) = self.terminal_cache.write() {
                cache.cache_line(key.text.clone(), Arc::new(shaped_text.clone()));
            }
        }

        // Store in primary cache
        if let Ok(mut cache) = self.primary_cache.write() {
            let entry = CachedShapedText {
                shaped_text,
                created_at: now,
                last_accessed: now,
                access_count: 1,
            };
            cache.put(key, entry);
        }
    }

    /// Check if text looks like a terminal line (heuristic)
    fn is_terminal_like(&self, text: &str) -> bool {
        // Terminal lines are often:
        // 1. Contain mostly printable ASCII
        // 2. Have common patterns (prompts, commands, paths)
        // 3. Are not too long (reasonable terminal width)

        if text.len() > 500 {
            return false; // Too long for typical terminal line
        }

        let ascii_printable_count = text.chars()
            .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
            .count();

        // At least 80% ASCII printable characters
        ascii_printable_count as f64 / text.len() as f64 >= 0.8
    }

    /// Get comprehensive cache statistics
    pub fn get_stats(&self) -> ComprehensiveCacheStats {
        let mut stats = ComprehensiveCacheStats::default();

        if let Ok(global_stats) = self.stats.read() {
            stats.primary_hits = global_stats.primary_hits;
            stats.primary_misses = global_stats.primary_misses;
            stats.terminal_hits = global_stats.terminal_hits;
            stats.terminal_misses = global_stats.terminal_misses;
            stats.total_evictions = global_stats.total_evictions;

            if let Some(creation_time) = global_stats.cache_creation_time {
                stats.uptime_seconds = Instant::now().duration_since(creation_time).as_secs();
            }
        }

        if let Ok(primary) = self.primary_cache.read() {
            stats.primary_cache_size = primary.len();
        }

        if let Ok(terminal) = self.terminal_cache.read() {
            let terminal_stats = terminal.get_stats();
            stats.terminal_line_cache_size = terminal_stats.line_cache_size;
            stats.terminal_fragment_cache_size = terminal_stats.fragment_cache_size;
            stats.terminal_hit_ratio = terminal_stats.overall_hit_ratio;
        }

        // Calculate overall hit ratio
        let total_hits = stats.primary_hits + stats.terminal_hits;
        let total_requests = total_hits + stats.primary_misses + stats.terminal_misses;
        stats.overall_hit_ratio = if total_requests > 0 {
            total_hits as f64 / total_requests as f64
        } else {
            0.0
        };

        stats
    }

    /// Clear all caches
    pub fn clear(&self) {
        if let Ok(mut cache) = self.primary_cache.write() {
            cache.clear();
        }

        if let Ok(mut cache) = self.terminal_cache.write() {
            cache.clear();
        }

        if let Ok(mut stats) = self.stats.write() {
            *stats = GlobalCacheStats::default();
            stats.cache_creation_time = Some(Instant::now());
        }
    }

    /// Perform cache maintenance (cleanup expired entries, etc.)
    pub fn maintenance(&self) {
        let now = Instant::now();

        if let Ok(mut cache) = self.primary_cache.write() {
            // Remove expired entries (this is a simplified approach)
            let expired_keys: Vec<ShapedTextCacheKey> = cache
                .iter()
                .filter(|(_, entry)| {
                    now.duration_since(entry.created_at).as_secs() > self.config.entry_ttl_seconds
                })
                .map(|(key, _)| key.clone())
                .collect();

            for key in expired_keys {
                cache.pop(&key);
                if let Ok(mut stats) = self.stats.write() {
                    stats.total_evictions += 1;
                }
            }
        }
    }
}

/// Comprehensive cache statistics
#[derive(Debug, Clone, Default)]
pub struct ComprehensiveCacheStats {
    pub primary_cache_size: usize,
    pub terminal_line_cache_size: usize,
    pub terminal_fragment_cache_size: usize,
    pub primary_hits: u64,
    pub primary_misses: u64,
    pub terminal_hits: u64,
    pub terminal_misses: u64,
    pub total_evictions: u64,
    pub overall_hit_ratio: f64,
    pub terminal_hit_ratio: f64,
    pub uptime_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_equality() {
        let key1 = ShapedTextCacheKey {
            text: "hello world".to_string(),
            font_family: "JetBrains Mono".to_string(),
            font_size_bits: 16.0f32.to_bits(),
            font_flags: 0,
            features_hash: 12345,
        };

        let key2 = ShapedTextCacheKey {
            text: "hello world".to_string(),
            font_family: "JetBrains Mono".to_string(),
            font_size_bits: 16.0f32.to_bits(),
            font_flags: 0,
            features_hash: 12345,
        };

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_terminal_line_detection() {
        let config = ShapedTextCacheConfig::default();
        let cache = ShapedTextCache::new(config);

        assert!(cache.is_terminal_like("$ ls -la"));
        assert!(cache.is_terminal_like("user@host:~/project$ git status"));
        assert!(cache.is_terminal_like("Hello, World!"));
        assert!(!cache.is_terminal_like("这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本这是一段中文测试文本"));
    }

    #[test]
    fn test_terminal_line_cache() {
        let mut cache = TerminalLineCache::new(10, 20);

        // Test cache miss
        assert!(cache.get_line("$ ls").is_none());

        // Test cache hit after insertion
        let shaped_text = Arc::new(ShapedText {
            glyphs: vec![],
            width: 100.0,
            height: 20.0,
            baseline: 16.0,
            direction: TextDirection::LeftToRight,
        });

        cache.cache_line("$ ls".to_string(), shaped_text.clone());
        assert!(cache.get_line("$ ls").is_some());

        let stats = cache.get_stats();
        assert_eq!(stats.line_cache_size, 1);
        assert!(stats.line_hit_ratio > 0.0);
    }
}
