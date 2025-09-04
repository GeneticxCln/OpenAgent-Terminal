// Shaping Cache Module - LRU cache for shaped text results

use lru::LruCache;
use std::num::NonZeroUsize;
use super::ShapedText;

/// Cache for shaped text results
pub struct ShapingCache {
    cache: LruCache<String, ShapedText>,
}

impl ShapingCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1000).unwrap())),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<ShapedText> {
        self.cache.get(key).cloned()
    }

    pub fn put(&mut self, key: String, value: ShapedText) {
        self.cache.put(key, value);
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }
}
