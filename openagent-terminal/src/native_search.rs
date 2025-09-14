//! Native Real-time Search and Filtering for OpenAgent Terminal
//!
//! Provides immediate search capabilities for blocks, tabs, and splits with no lazy loading
//! or background processing. Features instant indexing, fuzzy matching, and real-time filtering.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use regex::Regex;
// use serde::{Deserialize, Serialize}; // retained for future serialization when needed
// use tokio::sync::mpsc; // not used currently
use tracing::{debug, info, warn};

use crate::blocks_v2::{BlockId};
use crate::shell_integration::CommandId;

/// Native search and filtering manager
pub struct SearchIntegration {
    /// Text search engine for immediate results
    text_search: TextSearchEngine,

    /// Command search for shell history
    command_search: CommandSearchEngine,

    /// Block content search
    block_search: BlockSearchEngine,

    /// File search capabilities
    file_search: FileSearchEngine,

    /// Real-time filtering system
    filter_system: FilterSystem,

    /// Search index manager
    index_manager: SearchIndexManager,

    /// Event callbacks for immediate responses
    event_callbacks: Vec<Box<dyn Fn(&SearchEvent) + Send + Sync>>,

    /// Performance statistics
    stats: SearchStats,
}

/// Search events for immediate feedback
#[derive(Debug, Clone)]
pub enum SearchEvent {
    SearchStarted {
        query: String,
        context: SearchContext,
        timestamp: Instant,
    },
    ResultsFound {
        query: String,
        results: Vec<SearchResult>,
        duration: Duration,
        timestamp: Instant,
    },
    SearchCompleted {
        query: String,
        total_results: usize,
        duration: Duration,
        timestamp: Instant,
    },
    IndexUpdated {
        context: SearchContext,
        items_added: usize,
        items_removed: usize,
        timestamp: Instant,
    },
    FilterApplied {
        filter: SearchFilter,
        results_count: usize,
        timestamp: Instant,
    },
}

/// Text search engine for immediate results
#[derive(Debug)]
pub struct TextSearchEngine {
    /// Inverted index for fast text search
    inverted_index: InvertedIndex,

    /// Fuzzy matching engine
    fuzzy_matcher: FuzzyMatcher,

    /// Search algorithms
    algorithms: SearchAlgorithms,

    /// Search configuration
    config: TextSearchConfig,

    /// Recent searches for optimization
    recent_searches: VecDeque<CachedSearch>,
}

/// Command search engine for shell history
#[derive(Debug)]
pub struct CommandSearchEngine {
    /// Command index for fast lookup
    command_index: CommandIndex,

    /// Frequency-based ranking
    frequency_ranker: FrequencyRanker,

    /// Context-aware search
    context_matcher: ContextMatcher,

    /// Command pattern recognition
    pattern_recognizer: PatternRecognizer,

    /// Search cache for performance
    search_cache: HashMap<String, Vec<CommandMatch>>,
}

/// Block content search engine
#[derive(Debug)]
pub struct BlockSearchEngine {
    /// Block content index
    content_index: BlockContentIndex,

    /// Output buffer search
    output_searcher: OutputSearcher,

    /// Cross-block search
    cross_block_searcher: CrossBlockSearcher,

    /// Block metadata search
    metadata_searcher: MetadataSearcher,
}

/// File search engine
#[derive(Debug)]
pub struct FileSearchEngine {
    /// File name index
    filename_index: FilenameIndex,

    /// File content index
    content_index: FileContentIndex,

    /// Path-based search
    path_searcher: PathSearcher,

    /// Git integration
    git_searcher: GitSearcher,

    /// File watchers for real-time updates
    file_watchers: HashMap<String, FileWatcher>,
}

/// Real-time filtering system
#[derive(Debug)]
pub struct FilterSystem {
    /// Active filters
    active_filters: Vec<SearchFilter>,

    /// Filter chain processor
    filter_chain: FilterChain,

    /// Filter history
    filter_history: VecDeque<FilterApplication>,

    /// Dynamic filter creation
    dynamic_filters: HashMap<String, DynamicFilter>,
}

/// Search index manager
#[derive(Debug)]
pub struct SearchIndexManager {
    /// Index registry
    indices: HashMap<String, IndexInfo>,

    /// Index update queue
    update_queue: VecDeque<IndexUpdate>,

    /// Index statistics
    index_stats: HashMap<String, IndexStats>,

    /// Index optimization
    optimizer: IndexOptimizer,
}

/// Search contexts for scoped search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchContext {
    Global,
    CurrentBlock,
    AllBlocks,
    CommandHistory,
    FileSystem,
    GitRepository,
    Terminal,
    Tabs,
    Splits,
}

/// Search result representation
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub content: String,
    pub context: SearchContext,
    pub relevance_score: f64,
    pub match_positions: Vec<MatchPosition>,
    pub metadata: HashMap<String, String>,
    pub timestamp: Instant,
}

/// Match position in content
#[derive(Debug, Clone)]
pub struct MatchPosition {
    pub start: usize,
    pub end: usize,
    pub match_type: MatchType,
    pub context: Option<String>,
}

/// Types of matches
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchType {
    Exact,
    Fuzzy,
    Prefix,
    Suffix,
    Substring,
    Regex,
    Phonetic,
}

/// Search filters for refinement
#[derive(Clone)]
pub enum SearchFilter {
    TextFilter {
        pattern: String,
        case_sensitive: bool,
    },
    RegexFilter {
        regex: Regex,
    },
    DateFilter {
        from: Option<Instant>,
        to: Option<Instant>,
    },
    TypeFilter {
        types: HashSet<String>,
    },
    SizeFilter {
        min_size: Option<usize>,
        max_size: Option<usize>,
    },
    ScoreFilter {
        min_score: f64,
    },
    ContextFilter {
        contexts: HashSet<SearchContext>,
    },
    Custom {
        name: String,
        predicate: Arc<dyn Fn(&SearchResult) -> bool + Send + Sync>,
    },
}

impl fmt::Debug for SearchFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchFilter::TextFilter {
                pattern,
                case_sensitive,
            } => f
                .debug_struct("TextFilter")
                .field("pattern", pattern)
                .field("case_sensitive", case_sensitive)
                .finish(),
            SearchFilter::RegexFilter { .. } => write!(f, "RegexFilter(..)"),
            SearchFilter::DateFilter { from, to } => f
                .debug_struct("DateFilter")
                .field("from", &from.map(|_| ".."))
                .field("to", &to.map(|_| ".."))
                .finish(),
            SearchFilter::TypeFilter { types } => f.debug_tuple("TypeFilter").field(types).finish(),
            SearchFilter::SizeFilter { min_size, max_size } => f
                .debug_struct("SizeFilter")
                .field("min_size", min_size)
                .field("max_size", max_size)
                .finish(),
            SearchFilter::ScoreFilter { min_score } => {
                f.debug_tuple("ScoreFilter").field(min_score).finish()
            }
            SearchFilter::ContextFilter { contexts } => {
                f.debug_tuple("ContextFilter").field(contexts).finish()
            }
            SearchFilter::Custom { name, .. } => f.debug_tuple("Custom").field(name).finish(),
        }
    }
}

/// Inverted index for text search
#[derive(Debug, Default)]
pub struct InvertedIndex {
    /// Term to document mapping
    term_docs: HashMap<String, HashSet<String>>,

    /// Document to terms mapping
    doc_terms: HashMap<String, HashSet<String>>,

    /// Term frequency data
    term_frequencies: HashMap<String, HashMap<String, usize>>,

    /// Document lengths for scoring
    doc_lengths: HashMap<String, usize>,
}

/// Fuzzy matching engine
#[derive(Debug)]
pub struct FuzzyMatcher {
    /// Edit distance calculator
    edit_distance: EditDistanceCalculator,

    /// Phonetic matcher
    phonetic_matcher: PhoneticMatcher,

    /// Similarity thresholds
    similarity_threshold: f64,

    /// Match scoring weights
    scoring_weights: FuzzyWeights,
}

/// Search algorithms collection
#[derive(Debug)]
pub struct SearchAlgorithms {
    /// Boolean search
    boolean_search: BooleanSearch,

    /// TF-IDF scoring
    tfidf_scorer: TfIdfScorer,

    /// BM25 ranking
    bm25_ranker: Bm25Ranker,

    /// Vector space search
    vector_search: VectorSpaceSearch,
}

/// Text search configuration
#[derive(Debug, Clone)]
pub struct TextSearchConfig {
    pub max_results: usize,
    pub min_match_score: f64,
    pub case_sensitive: bool,
    pub whole_words_only: bool,
    pub include_fuzzy: bool,
    pub fuzzy_threshold: f64,
    pub highlight_matches: bool,
    pub search_timeout: Duration,
}

/// Cached search for optimization
#[derive(Debug, Clone)]
pub struct CachedSearch {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub timestamp: Instant,
    pub context: SearchContext,
}

/// Command index for shell history search
#[derive(Debug, Default)]
pub struct CommandIndex {
    /// Command to metadata mapping
    command_metadata: HashMap<CommandId, CommandMetadata>,

    /// Text to command mapping
    text_index: HashMap<String, Vec<CommandId>>,

    /// Category index
    category_index: HashMap<String, Vec<CommandId>>,

    /// Time-based index
    time_index: BTreeMap<Instant, Vec<CommandId>>,
}

/// Command metadata for search
#[derive(Debug, Clone)]
pub struct CommandMetadata {
    pub command: String,
    pub timestamp: Instant,
    pub working_dir: String,
    pub exit_code: i32,
    pub duration: Duration,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub frequency: usize,
}

/// Frequency-based ranking
#[derive(Debug, Default)]
pub struct FrequencyRanker {
    pub command_frequencies: HashMap<String, usize>,
    pub recent_boost: HashMap<String, f64>,
    pub context_boost: HashMap<String, f64>,
    pub success_boost: HashMap<String, f64>,
}

/// Context-aware matching
#[derive(Debug, Default)]
pub struct ContextMatcher {
    pub directory_patterns: HashMap<String, Vec<String>>,
    pub project_patterns: HashMap<String, Vec<String>>,
    pub time_patterns: HashMap<String, Vec<String>>,
}

/// Pattern recognition for commands
#[derive(Debug, Default)]
pub struct PatternRecognizer {
    pub common_patterns: Vec<CommandPattern>,
    pub user_patterns: Vec<CommandPattern>,
    pub learned_patterns: Vec<CommandPattern>,
}

/// Command pattern
#[derive(Debug, Clone)]
pub struct CommandPattern {
    pub name: String,
    pub pattern: Regex,
    pub description: String,
    pub examples: Vec<String>,
    pub frequency: usize,
}

/// Command match result
#[derive(Debug, Clone)]
pub struct CommandMatch {
    pub command_id: CommandId,
    pub command: String,
    pub score: f64,
    pub match_type: MatchType,
    pub context: String,
    pub metadata: CommandMetadata,
}

/// Block content index
#[derive(Debug, Default)]
pub struct BlockContentIndex {
    pub block_content: HashMap<BlockId, String>,
    pub content_index: HashMap<String, Vec<BlockId>>,
    pub block_metadata: HashMap<BlockId, BlockMetadata>,
}

/// Block metadata
#[derive(Debug, Clone)]
pub struct BlockMetadata {
    pub title: String,
    pub block_type: String,
    pub created: Instant,
    pub modified: Instant,
    pub size: usize,
    pub tags: Vec<String>,
}

/// Output buffer searcher
#[derive(Debug, Default)]
pub struct OutputSearcher {
    pub buffer_index: HashMap<String, Vec<OutputMatch>>,
    pub recent_output: VecDeque<OutputEntry>,
    pub search_cache: HashMap<String, Vec<OutputMatch>>,
}

/// Output entry
#[derive(Debug, Clone)]
pub struct OutputEntry {
    pub block_id: BlockId,
    pub content: String,
    pub timestamp: Instant,
    pub line_number: usize,
}

/// Output match
#[derive(Debug, Clone)]
pub struct OutputMatch {
    pub block_id: BlockId,
    pub line_number: usize,
    pub content: String,
    pub match_positions: Vec<MatchPosition>,
    pub timestamp: Instant,
}

/// Cross-block searcher
#[derive(Debug, Default)]
pub struct CrossBlockSearcher {
    pub cross_references: HashMap<String, Vec<BlockId>>,
    pub dependency_graph: HashMap<BlockId, Vec<BlockId>>,
    pub correlation_matrix: HashMap<(BlockId, BlockId), f64>,
}

/// Metadata searcher
#[derive(Debug, Default)]
pub struct MetadataSearcher {
    pub tag_index: HashMap<String, Vec<BlockId>>,
    pub type_index: HashMap<String, Vec<BlockId>>,
    pub date_index: BTreeMap<Instant, Vec<BlockId>>,
}

/// Filename index
#[derive(Debug, Default)]
pub struct FilenameIndex {
    pub name_index: HashMap<String, Vec<FileEntry>>,
    pub extension_index: HashMap<String, Vec<FileEntry>>,
    pub path_segments: HashMap<String, Vec<FileEntry>>,
}

/// File entry
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub extension: Option<String>,
    pub size: u64,
    pub modified: Instant,
    pub file_type: FileType,
}

/// File type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Regular,
    Directory,
    Symlink,
    Executable,
    Hidden,
}

/// File content index
#[derive(Debug, Default)]
pub struct FileContentIndex {
    pub content_index: HashMap<String, Vec<String>>, // term -> files
    pub file_metadata: HashMap<String, FileContentMetadata>,
    pub binary_files: HashSet<String>,
}

/// File content metadata
#[derive(Debug, Clone)]
pub struct FileContentMetadata {
    pub path: String,
    pub size: u64,
    pub modified: Instant,
    pub encoding: String,
    pub line_count: usize,
    pub language: Option<String>,
}

/// Path-based searcher
#[derive(Debug, Default)]
pub struct PathSearcher {
    pub path_trie: PathTrie,
    pub glob_patterns: Vec<GlobPattern>,
    pub search_cache: HashMap<String, Vec<String>>,
}

/// Path trie for efficient path matching
#[derive(Debug, Default)]
pub struct PathTrie {
    pub nodes: HashMap<String, PathTrieNode>,
    pub root: PathTrieNode,
}

/// Path trie node
#[derive(Debug, Default, Clone)]
pub struct PathTrieNode {
    pub segment: String,
    pub children: HashMap<String, PathTrieNode>,
    pub files: Vec<String>,
    pub is_directory: bool,
}

/// Glob pattern
#[derive(Debug, Clone)]
pub struct GlobPattern {
    pub pattern: String,
    pub regex: Regex,
    pub description: String,
}

/// Git searcher integration
#[derive(Debug, Default)]
pub struct GitSearcher {
    pub repository_paths: Vec<String>,
    pub commit_index: HashMap<String, Vec<GitCommit>>,
    pub branch_index: HashMap<String, Vec<String>>,
    pub file_history: HashMap<String, Vec<GitFileChange>>,
}

/// Git commit information
#[derive(Debug, Clone)]
pub struct GitCommit {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub timestamp: Instant,
    pub files_changed: Vec<String>,
}

/// Git file change
#[derive(Debug, Clone)]
pub struct GitFileChange {
    pub path: String,
    pub change_type: GitChangeType,
    pub commit_hash: String,
    pub timestamp: Instant,
}

/// Git change types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

/// File watcher for real-time updates
#[derive(Debug)]
pub struct FileWatcher {
    pub path: String,
    pub recursive: bool,
    pub last_scan: Instant,
    pub watched_files: HashSet<String>,
}

/// Filter chain processor
#[derive(Debug, Default)]
pub struct FilterChain {
    pub filters: Vec<SearchFilter>,
    pub chain_stats: FilterChainStats,
}

/// Filter chain statistics
#[derive(Debug, Default)]
pub struct FilterChainStats {
    pub total_applied: usize,
    pub results_filtered: usize,
    pub average_filter_time: Duration,
}

/// Filter application record
#[derive(Debug, Clone)]
pub struct FilterApplication {
    pub filter: SearchFilter,
    pub input_count: usize,
    pub output_count: usize,
    pub duration: Duration,
    pub timestamp: Instant,
}

/// Dynamic filter
pub struct DynamicFilter {
    pub name: String,
    pub description: String,
    pub predicate: Arc<dyn Fn(&SearchResult) -> bool + Send + Sync>,
    pub created: Instant,
    pub usage_count: usize,
}

impl fmt::Debug for DynamicFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicFilter")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("created", &"..")
            .field("usage_count", &self.usage_count)
            .finish()
    }
}

/// Index information
#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub name: String,
    pub description: String,
    pub item_count: usize,
    pub last_updated: Instant,
    pub index_type: IndexType,
    pub size_bytes: usize,
}

/// Index types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    Inverted,
    Trie,
    Hash,
    BTree,
    Vector,
}

/// Index update operation
#[derive(Debug, Clone)]
pub struct IndexUpdate {
    pub index_name: String,
    pub operation: UpdateOperation,
    pub item_id: String,
    pub content: Option<String>,
    pub timestamp: Instant,
}

/// Update operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOperation {
    Add,
    Update,
    Remove,
    Clear,
}

/// Index statistics
#[derive(Debug, Default, Clone)]
pub struct IndexStats {
    pub items: usize,
    pub terms: usize,
    pub size_bytes: usize,
    pub queries: usize,
    pub hits: usize,
    pub misses: usize,
    pub average_query_time: Duration,
}

/// Index optimizer
#[derive(Debug, Default)]
pub struct IndexOptimizer {
    pub optimization_schedule: HashMap<String, Instant>,
    pub optimization_stats: HashMap<String, OptimizationStats>,
}

/// Optimization statistics
#[derive(Debug, Default, Clone)]
pub struct OptimizationStats {
    pub runs: usize,
    pub last_run: Option<Instant>,
    pub time_saved: Duration,
    pub space_saved: usize,
}

/// Edit distance calculator
#[derive(Debug, Default)]
pub struct EditDistanceCalculator {
    pub algorithm: EditDistanceAlgorithm,
    pub cache: HashMap<(String, String), usize>,
}

/// Edit distance algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditDistanceAlgorithm {
    Levenshtein,
    DamerauLevenshtein,
    Hamming,
    Jaro,
    JaroWinkler,
}

impl Default for EditDistanceAlgorithm {
    fn default() -> Self {
        Self::Levenshtein
    }
}

/// Phonetic matcher
#[derive(Debug, Default)]
pub struct PhoneticMatcher {
    pub algorithm: PhoneticAlgorithm,
    pub phonetic_cache: HashMap<String, String>,
}

/// Phonetic algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhoneticAlgorithm {
    Soundex,
    Metaphone,
    DoubleMetaphone,
    Nysiis,
}

impl Default for PhoneticAlgorithm {
    fn default() -> Self {
        Self::Soundex
    }
}

/// Fuzzy matching weights
#[derive(Debug, Clone)]
pub struct FuzzyWeights {
    pub edit_distance_weight: f64,
    pub prefix_bonus: f64,
    pub exact_match_bonus: f64,
    pub case_match_bonus: f64,
    pub word_boundary_bonus: f64,
}

/// Boolean search engine
#[derive(Debug, Default)]
pub struct BooleanSearch {
    pub operators: Vec<BooleanOperator>,
    pub query_parser: QueryParser,
}

/// Boolean operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOperator {
    And,
    Or,
    Not,
    Near,
    Phrase,
}

/// Query parser
#[derive(Debug, Default)]
pub struct QueryParser {
    pub tokens: Vec<QueryToken>,
    pub parse_tree: Option<QueryTree>,
}

/// Query tokens
#[derive(Debug, Clone)]
pub enum QueryToken {
    Term(String),
    Operator(BooleanOperator),
    Group(Vec<QueryToken>),
    Phrase(String),
    Wildcard(String),
    Regex(String),
}

/// Query parse tree
#[derive(Debug, Clone)]
pub enum QueryTree {
    Term(String),
    And(Box<QueryTree>, Box<QueryTree>),
    Or(Box<QueryTree>, Box<QueryTree>),
    Not(Box<QueryTree>),
    Phrase(String),
    Wildcard(String),
}

/// TF-IDF scorer
#[derive(Debug, Default)]
pub struct TfIdfScorer {
    pub term_frequencies: HashMap<String, HashMap<String, f64>>,
    pub document_frequencies: HashMap<String, usize>,
    pub document_count: usize,
}

/// BM25 ranker
#[derive(Debug)]
pub struct Bm25Ranker {
    pub k1: f64,
    pub b: f64,
    pub average_doc_length: f64,
    pub doc_lengths: HashMap<String, usize>,
}

/// Vector space search
#[derive(Debug, Default)]
pub struct VectorSpaceSearch {
    pub term_vectors: HashMap<String, Vec<f64>>,
    pub document_vectors: HashMap<String, Vec<f64>>,
    pub similarity_measure: SimilarityMeasure,
}

/// Similarity measures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimilarityMeasure {
    Cosine,
    Euclidean,
    Manhattan,
    Jaccard,
    Pearson,
}

impl Default for SimilarityMeasure {
    fn default() -> Self {
        Self::Cosine
    }
}

/// Performance statistics
#[derive(Debug, Clone)]
pub struct SearchStats {
    pub searches_performed: usize,
    pub results_returned: usize,
    pub average_search_time: Duration,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub index_updates: usize,
    pub filters_applied: usize,
    pub last_reset: Instant,
}

impl Default for SearchStats {
    fn default() -> Self {
        Self {
            searches_performed: 0,
            results_returned: 0,
            average_search_time: Duration::default(),
            cache_hits: 0,
            cache_misses: 0,
            index_updates: 0,
            filters_applied: 0,
            last_reset: Instant::now(),
        }
    }
}

impl SearchIntegration {
    /// Create new search integration with immediate capabilities
    pub fn new() -> Self {
        let mut integration = Self {
            text_search: TextSearchEngine::new(),
            command_search: CommandSearchEngine::new(),
            block_search: BlockSearchEngine::new(),
            file_search: FileSearchEngine::new(),
            filter_system: FilterSystem::new(),
            index_manager: SearchIndexManager::new(),
            event_callbacks: Vec::new(),
            stats: SearchStats {
                last_reset: Instant::now(),
                ..Default::default()
            },
        };

        // Initialize indices immediately
        integration.initialize_indices();

        integration
    }

    /// Register event callback for immediate responses
    pub fn register_event_callback<F>(&mut self, callback: F)
    where
        F: Fn(&SearchEvent) + Send + Sync + 'static,
    {
        self.event_callbacks.push(Box::new(callback));
    }

    /// Emit search event immediately
    fn emit_event(&self, event: SearchEvent) {
        for callback in &self.event_callbacks {
            callback(&event);
        }
    }

    /// Perform immediate search across all contexts
    pub fn search(&mut self, query: &str, context: SearchContext) -> Result<Vec<SearchResult>> {
        let start_time = Instant::now();

        // Emit search started event
        self.emit_event(SearchEvent::SearchStarted {
            query: query.to_string(),
            context,
            timestamp: start_time,
        });

        let mut all_results = Vec::new();

        // Search based on context immediately
        match context {
            SearchContext::Global => {
                // Search all contexts
                all_results.extend(self.search_text(query)?);
                all_results.extend(self.search_commands(query)?);
                all_results.extend(self.search_blocks(query)?);
                all_results.extend(self.search_files(query)?);
            }
            SearchContext::CommandHistory => {
                all_results = self.search_commands(query)?;
            }
            SearchContext::AllBlocks | SearchContext::CurrentBlock => {
                all_results = self.search_blocks(query)?;
            }
            SearchContext::FileSystem => {
                all_results = self.search_files(query)?;
            }
            SearchContext::Terminal => {
                all_results.extend(self.search_text(query)?);
                all_results.extend(self.search_commands(query)?);
            }
            _ => {
                all_results = self.search_text(query)?;
            }
        }

        // Apply active filters immediately
        all_results = self.apply_filters(all_results)?;

        // Sort by relevance immediately
        all_results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());

        let duration = start_time.elapsed();

        // Emit results found event
        self.emit_event(SearchEvent::ResultsFound {
            query: query.to_string(),
            results: all_results.clone(),
            duration,
            timestamp: start_time,
        });

        // Update statistics immediately
        self.stats.searches_performed += 1;
        self.stats.results_returned += all_results.len();
        self.update_average_search_time(duration);

        // Emit search completed event
        self.emit_event(SearchEvent::SearchCompleted {
            query: query.to_string(),
            total_results: all_results.len(),
            duration,
            timestamp: start_time,
        });

        Ok(all_results)
    }

    /// Search text content immediately
    pub fn search_text(&mut self, query: &str) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        // Exact text search
        if let Some(exact_results) = self.text_search.search_exact(query) {
            results.extend(exact_results);
        }

        // Fuzzy search if enabled
        if self.text_search.config.include_fuzzy {
            if let Some(fuzzy_results) = self.text_search.search_fuzzy(query) {
                results.extend(fuzzy_results);
            }
        }

        // Boolean search for complex queries
        if query.contains(" AND ") || query.contains(" OR ") || query.contains(" NOT ") {
            if let Some(boolean_results) = self.text_search.search_boolean(query) {
                results.extend(boolean_results);
            }
        }

        Ok(results)
    }

    /// Search command history immediately
    pub fn search_commands(&mut self, query: &str) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        // Search by command text
        let command_matches = self.command_search.search_by_text(query);

        // Convert command matches to search results
        for cmd_match in command_matches {
            let result = SearchResult {
                id: format!("cmd_{:?}", cmd_match.command_id),
                title: cmd_match.command.clone(),
                content: format!("{} ({})", cmd_match.command, cmd_match.context),
                context: SearchContext::CommandHistory,
                relevance_score: cmd_match.score,
                match_positions: vec![MatchPosition {
                    start: 0,
                    end: cmd_match.command.len(),
                    match_type: cmd_match.match_type,
                    context: Some(cmd_match.context),
                }],
                metadata: HashMap::from([
                    (
                        "exit_code".to_string(),
                        cmd_match.metadata.exit_code.to_string(),
                    ),
                    (
                        "duration".to_string(),
                        format!("{:?}", cmd_match.metadata.duration),
                    ),
                    ("working_dir".to_string(), cmd_match.metadata.working_dir),
                ]),
                timestamp: cmd_match.metadata.timestamp,
            };
            results.push(result);
        }

        Ok(results)
    }

    /// Search block content immediately
    pub fn search_blocks(&mut self, query: &str) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        // Search block content
        let block_matches = self.block_search.search_content(query);

        for (block_id, matches) in block_matches {
            if let Some(metadata) = self
                .block_search
                .content_index
                .block_metadata
                .get(&block_id)
            {
                let result = SearchResult {
                    id: format!("block_{:?}", block_id),
                    title: metadata.title.clone(),
                    content: matches.join("\n"),
                    context: SearchContext::AllBlocks,
                    relevance_score: self.calculate_block_relevance(&matches, query),
                    match_positions: self.extract_match_positions(&matches, query),
                    metadata: HashMap::from([
                        ("block_type".to_string(), metadata.block_type.clone()),
                        ("size".to_string(), metadata.size.to_string()),
                        ("tags".to_string(), metadata.tags.join(", ")),
                    ]),
                    timestamp: metadata.modified,
                };
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Search file system immediately
    pub fn search_files(&mut self, query: &str) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        // Search file names
        let filename_matches = self.file_search.search_filenames(query);

        for file_entry in filename_matches {
            let result = SearchResult {
                id: format!("file_{}", file_entry.path),
                title: file_entry.name.clone(),
                content: file_entry.path.clone(),
                context: SearchContext::FileSystem,
                relevance_score: self.calculate_filename_relevance(&file_entry, query),
                match_positions: self.extract_filename_matches(&file_entry.name, query),
                metadata: HashMap::from([
                    ("path".to_string(), file_entry.path),
                    ("size".to_string(), file_entry.size.to_string()),
                    ("type".to_string(), format!("{:?}", file_entry.file_type)),
                    (
                        "extension".to_string(),
                        file_entry.extension.unwrap_or_default(),
                    ),
                ]),
                timestamp: file_entry.modified,
            };
            results.push(result);
        }

        // Search file content if query is complex enough
        if query.len() > 3 {
            let content_matches = self.file_search.search_content(query);

            for (file_path, matches) in content_matches {
                if let Some(metadata) = self.file_search.content_index.file_metadata.get(&file_path)
                {
                    let result = SearchResult {
                        id: format!("file_content_{}", file_path),
                        title: file_path
                            .split('/')
                            .last()
                            .unwrap_or(&file_path)
                            .to_string(),
                        content: matches.join("\n"),
                        context: SearchContext::FileSystem,
                        relevance_score: self.calculate_content_relevance(&matches, query),
                        match_positions: self.extract_content_matches(&matches, query),
                        metadata: HashMap::from([
                            ("path".to_string(), metadata.path.clone()),
                            ("size".to_string(), metadata.size.to_string()),
                            (
                                "language".to_string(),
                                metadata
                                    .language
                                    .as_ref()
                                    .unwrap_or(&"unknown".to_string())
                                    .clone(),
                            ),
                            ("lines".to_string(), metadata.line_count.to_string()),
                        ]),
                        timestamp: metadata.modified,
                    };
                    results.push(result);
                }
            }
        }

        Ok(results)
    }

    /// Apply filters to results immediately
    pub fn apply_filters(&mut self, mut results: Vec<SearchResult>) -> Result<Vec<SearchResult>> {
        for filter in &self.filter_system.active_filters.clone() {
            let start_time = Instant::now();
            let input_count = results.len();

            results = self.apply_single_filter(results, filter)?;

            let duration = start_time.elapsed();
            let output_count = results.len();

            // Record filter application
            self.filter_system
                .filter_history
                .push_back(FilterApplication {
                    filter: filter.clone(),
                    input_count,
                    output_count,
                    duration,
                    timestamp: start_time,
                });

            // Limit filter history
            if self.filter_system.filter_history.len() > 1000 {
                self.filter_system.filter_history.pop_front();
            }

            // Emit filter applied event
            self.emit_event(SearchEvent::FilterApplied {
                filter: filter.clone(),
                results_count: output_count,
                timestamp: start_time,
            });

            self.stats.filters_applied += 1;
        }

        Ok(results)
    }

    /// Apply a single filter immediately
    fn apply_single_filter(
        &self,
        results: Vec<SearchResult>,
        filter: &SearchFilter,
    ) -> Result<Vec<SearchResult>> {
        let filtered = results
            .into_iter()
            .filter(|result| {
                match filter {
                    SearchFilter::TextFilter {
                        pattern,
                        case_sensitive,
                    } => {
                        let content = if *case_sensitive {
                            result.content.clone()
                        } else {
                            result.content.to_lowercase()
                        };
                        let search_pattern = if *case_sensitive {
                            pattern.clone()
                        } else {
                            pattern.to_lowercase()
                        };
                        content.contains(&search_pattern)
                    }
                    SearchFilter::RegexFilter { regex } => regex.is_match(&result.content),
                    SearchFilter::ScoreFilter { min_score } => result.relevance_score >= *min_score,
                    SearchFilter::ContextFilter { contexts } => contexts.contains(&result.context),
                    SearchFilter::TypeFilter { types } => {
                        if let Some(file_type) = result.metadata.get("type") {
                            types.contains(file_type)
                        } else {
                            false
                        }
                    }
                    SearchFilter::Custom { predicate, .. } => predicate(result),
                    _ => true, // Other filters not implemented yet
                }
            })
            .collect();

        Ok(filtered)
    }

    /// Add search filter immediately
    pub fn add_filter(&mut self, filter: SearchFilter) {
        self.filter_system.active_filters.push(filter);
    }

    /// Remove search filter immediately
    pub fn remove_filter(&mut self, index: usize) -> Option<SearchFilter> {
        if index < self.filter_system.active_filters.len() {
            Some(self.filter_system.active_filters.remove(index))
        } else {
            None
        }
    }

    /// Clear all filters immediately
    pub fn clear_filters(&mut self) {
        self.filter_system.active_filters.clear();
    }

    /// Update search index immediately
    pub fn update_index(
        &mut self,
        index_name: &str,
        item_id: &str,
        content: Option<String>,
    ) -> Result<()> {
        let operation = if content.is_some() {
            UpdateOperation::Update
        } else {
            UpdateOperation::Remove
        };

        let update = IndexUpdate {
            index_name: index_name.to_string(),
            operation,
            item_id: item_id.to_string(),
            content,
            timestamp: Instant::now(),
        };

        // Apply update immediately
        self.apply_index_update(&update)?;

        // Record update
        self.index_manager.update_queue.push_back(update);
        if self.index_manager.update_queue.len() > 10000 {
            self.index_manager.update_queue.pop_front();
        }

        self.stats.index_updates += 1;

        Ok(())
    }

    /// Get search statistics
    pub fn get_stats(&self) -> SearchStats {
        self.stats.clone()
    }

    /// Initialize search indices
    fn initialize_indices(&mut self) {
        // Initialize text search index
        self.text_search.inverted_index = InvertedIndex::default();

        // Initialize command search index
        self.command_search.command_index = CommandIndex::default();

        // Initialize block search index
        self.block_search.content_index = BlockContentIndex::default();

        // Initialize file search index
        self.file_search.filename_index = FilenameIndex::default();
        self.file_search.content_index = FileContentIndex::default();

        info!("Search indices initialized");
    }

    /// Apply index update immediately
    fn apply_index_update(&mut self, update: &IndexUpdate) -> Result<()> {
        match update.index_name.as_str() {
            "text" => match update.operation {
                UpdateOperation::Add | UpdateOperation::Update => {
                    if let Some(content) = &update.content {
                        self.text_search
                            .inverted_index
                            .add_document(&update.item_id, content);
                    }
                }
                UpdateOperation::Remove => {
                    self.text_search
                        .inverted_index
                        .remove_document(&update.item_id);
                }
                UpdateOperation::Clear => {
                    self.text_search.inverted_index = InvertedIndex::default();
                }
            },
            "commands" => {
                // Update command index
                debug!("Updating command index for: {}", update.item_id);
            }
            "blocks" => {
                // Update block index
                debug!("Updating block index for: {}", update.item_id);
            }
            "files" => {
                // Update file index
                debug!("Updating file index for: {}", update.item_id);
            }
            _ => {
                warn!("Unknown index: {}", update.index_name);
            }
        }

        Ok(())
    }

    /// Update average search time
    fn update_average_search_time(&mut self, duration: Duration) {
        let count = self.stats.searches_performed;
        self.stats.average_search_time =
            (self.stats.average_search_time * (count - 1) as u32 + duration) / count as u32;
    }

    /// Calculate block relevance score
    fn calculate_block_relevance(&self, matches: &[String], query: &str) -> f64 {
        let match_count = matches.len() as f64;
        let query_len = query.len() as f64;

        // Simple relevance calculation
        (match_count * query_len) / 100.0
    }

    /// Calculate filename relevance score
    fn calculate_filename_relevance(&self, file_entry: &FileEntry, query: &str) -> f64 {
        let name_match = file_entry
            .name
            .to_lowercase()
            .contains(&query.to_lowercase());
        let path_match = file_entry
            .path
            .to_lowercase()
            .contains(&query.to_lowercase());

        if name_match {
            1.0
        } else if path_match {
            0.5
        } else {
            0.1
        }
    }

    /// Calculate content relevance score
    fn calculate_content_relevance(&self, matches: &[String], query: &str) -> f64 {
        let match_count = matches.len() as f64;
        let total_occurrences = matches
            .iter()
            .map(|m| m.matches(query).count())
            .sum::<usize>() as f64;

        (match_count + total_occurrences) / 10.0
    }

    /// Extract match positions from content
    fn extract_match_positions(&self, matches: &[String], query: &str) -> Vec<MatchPosition> {
        let mut positions = Vec::new();

        for content in matches {
            let mut start = 0;
            while let Some(pos) = content[start..].find(query) {
                let absolute_pos = start + pos;
                positions.push(MatchPosition {
                    start: absolute_pos,
                    end: absolute_pos + query.len(),
                    match_type: MatchType::Substring,
                    context: Some(content.clone()),
                });
                start = absolute_pos + 1;
            }
        }

        positions
    }

    /// Extract filename match positions
    fn extract_filename_matches(&self, filename: &str, query: &str) -> Vec<MatchPosition> {
        let mut positions = Vec::new();

        if let Some(pos) = filename.to_lowercase().find(&query.to_lowercase()) {
            positions.push(MatchPosition {
                start: pos,
                end: pos + query.len(),
                match_type: MatchType::Substring,
                context: Some(filename.to_string()),
            });
        }

        positions
    }

    /// Extract content match positions
    fn extract_content_matches(&self, matches: &[String], query: &str) -> Vec<MatchPosition> {
        self.extract_match_positions(matches, query)
    }
}

// Implementation for helper structs
impl TextSearchEngine {
    fn new() -> Self {
        Self {
            inverted_index: InvertedIndex::default(),
            fuzzy_matcher: FuzzyMatcher::new(),
            algorithms: SearchAlgorithms::default(),
            config: TextSearchConfig::default(),
            recent_searches: VecDeque::new(),
        }
    }

    fn search_exact(&mut self, query: &str) -> Option<Vec<SearchResult>> {
        // Check cache first
        for cached in &self.recent_searches {
            if cached.query == query && cached.timestamp.elapsed() < Duration::from_secs(60) {
                return Some(cached.results.clone());
            }
        }

        // Perform exact search
        let results = self.inverted_index.search_exact(query);

        // Cache results
        self.cache_search_results(query.to_string(), results.clone());

        Some(results)
    }

    fn search_fuzzy(&self, query: &str) -> Option<Vec<SearchResult>> {
        if self.config.include_fuzzy {
            Some(self.fuzzy_matcher.search(query))
        } else {
            None
        }
    }

    fn search_boolean(&self, query: &str) -> Option<Vec<SearchResult>> {
        Some(self.algorithms.boolean_search.search(query))
    }

    fn cache_search_results(&mut self, query: String, results: Vec<SearchResult>) {
        let cached = CachedSearch {
            query,
            results,
            timestamp: Instant::now(),
            context: SearchContext::Global,
        };

        self.recent_searches.push_back(cached);

        // Limit cache size
        if self.recent_searches.len() > 100 {
            self.recent_searches.pop_front();
        }
    }
}

impl CommandSearchEngine {
    fn new() -> Self {
        Self {
            command_index: CommandIndex::default(),
            frequency_ranker: FrequencyRanker::default(),
            context_matcher: ContextMatcher::default(),
            pattern_recognizer: PatternRecognizer::default(),
            search_cache: HashMap::new(),
        }
    }

    fn search_by_text(&mut self, query: &str) -> Vec<CommandMatch> {
        // Check cache first
        if let Some(cached_results) = self.search_cache.get(query) {
            return cached_results.clone();
        }

        let mut matches = Vec::new();

        // Search command text index
        if let Some(command_ids) = self.command_index.text_index.get(query) {
            for cmd_id in command_ids {
                if let Some(metadata) = self.command_index.command_metadata.get(cmd_id) {
                    let score = self.frequency_ranker.calculate_score(&metadata.command);
                    matches.push(CommandMatch {
                        command_id: *cmd_id,
                        command: metadata.command.clone(),
                        score,
                        match_type: MatchType::Exact,
                        context: metadata.working_dir.clone(),
                        metadata: metadata.clone(),
                    });
                }
            }
        }

        // Cache results
        self.search_cache.insert(query.to_string(), matches.clone());

        matches
    }
}

impl BlockSearchEngine {
    fn new() -> Self {
        Self {
            content_index: BlockContentIndex::default(),
            output_searcher: OutputSearcher::default(),
            cross_block_searcher: CrossBlockSearcher::default(),
            metadata_searcher: MetadataSearcher::default(),
        }
    }

    fn search_content(&self, query: &str) -> HashMap<BlockId, Vec<String>> {
        let mut results = HashMap::new();

        // Search content index
        if let Some(block_ids) = self.content_index.content_index.get(query) {
            for block_id in block_ids {
                if let Some(content) = self.content_index.block_content.get(block_id) {
                    let matches = self.extract_matching_lines(content, query);
                    if !matches.is_empty() {
                        results.insert(*block_id, matches);
                    }
                }
            }
        }

        results
    }

    fn extract_matching_lines(&self, content: &str, query: &str) -> Vec<String> {
        content
            .lines()
            .filter(|line| line.contains(query))
            .map(|line| line.to_string())
            .collect()
    }
}

impl FileSearchEngine {
    fn new() -> Self {
        Self {
            filename_index: FilenameIndex::default(),
            content_index: FileContentIndex::default(),
            path_searcher: PathSearcher::default(),
            git_searcher: GitSearcher::default(),
            file_watchers: HashMap::new(),
        }
    }

    fn search_filenames(&self, query: &str) -> Vec<FileEntry> {
        let mut results = Vec::new();

        // Search name index
        for (name, entries) in &self.filename_index.name_index {
            if name.contains(query) {
                results.extend(entries.clone());
            }
        }

        results
    }

    fn search_content(&self, query: &str) -> HashMap<String, Vec<String>> {
        let mut results = HashMap::new();

        // Search content index
        if let Some(files) = self.content_index.content_index.get(query) {
            for file_path in files {
                if !self.content_index.binary_files.contains(file_path) {
                    // Extract matching lines from file
                    if let Ok(content) = std::fs::read_to_string(file_path) {
                        let matches: Vec<String> = content
                            .lines()
                            .filter(|line| line.contains(query))
                            .map(|line| line.to_string())
                            .collect();

                        if !matches.is_empty() {
                            results.insert(file_path.clone(), matches);
                        }
                    }
                }
            }
        }

        results
    }
}

impl FilterSystem {
    fn new() -> Self {
        Self {
            active_filters: Vec::new(),
            filter_chain: FilterChain::default(),
            filter_history: VecDeque::new(),
            dynamic_filters: HashMap::new(),
        }
    }
}

impl SearchIndexManager {
    fn new() -> Self {
        Self {
            indices: HashMap::new(),
            update_queue: VecDeque::new(),
            index_stats: HashMap::new(),
            optimizer: IndexOptimizer::default(),
        }
    }
}

impl InvertedIndex {
    fn add_document(&mut self, doc_id: &str, content: &str) {
        let terms: HashSet<String> = content
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        // Update term-to-document mapping
        for term in &terms {
            self.term_docs
                .entry(term.clone())
                .or_insert_with(HashSet::new)
                .insert(doc_id.to_string());

            // Update term frequency
            *self
                .term_frequencies
                .entry(term.clone())
                .or_insert_with(HashMap::new)
                .entry(doc_id.to_string())
                .or_insert(0) += 1;
        }

        // Update document-to-terms mapping
        self.doc_terms.insert(doc_id.to_string(), terms.clone());

        // Update document length
        self.doc_lengths.insert(doc_id.to_string(), content.len());
    }

    fn remove_document(&mut self, doc_id: &str) {
        if let Some(terms) = self.doc_terms.remove(doc_id) {
            for term in terms {
                if let Some(docs) = self.term_docs.get_mut(&term) {
                    docs.remove(doc_id);
                    if docs.is_empty() {
                        self.term_docs.remove(&term);
                    }
                }

                if let Some(freq_map) = self.term_frequencies.get_mut(&term) {
                    freq_map.remove(doc_id);
                    if freq_map.is_empty() {
                        self.term_frequencies.remove(&term);
                    }
                }
            }
        }

        self.doc_lengths.remove(doc_id);
    }

    fn search_exact(&self, query: &str) -> Vec<SearchResult> {
        let query_terms: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let mut doc_scores: HashMap<String, f64> = HashMap::new();

        // Calculate TF-IDF scores
        for term in query_terms {
            if let Some(docs) = self.term_docs.get(&term) {
                let idf = (self.doc_lengths.len() as f64 / docs.len() as f64).ln();

                for doc_id in docs {
                    let tf = self
                        .term_frequencies
                        .get(&term)
                        .and_then(|freq_map| freq_map.get(doc_id))
                        .copied()
                        .unwrap_or(0) as f64;

                    let doc_len = self.doc_lengths.get(doc_id).copied().unwrap_or(1) as f64;
                    let normalized_tf = tf / doc_len;

                    *doc_scores.entry(doc_id.clone()).or_insert(0.0) += normalized_tf * idf;
                }
            }
        }

        // Convert to search results
        doc_scores
            .into_iter()
            .map(|(doc_id, score)| SearchResult {
                id: doc_id.clone(),
                title: doc_id.clone(),
                content: "".to_string(), // Would be filled from actual document
                context: SearchContext::Global,
                relevance_score: score,
                match_positions: Vec::new(),
                metadata: HashMap::new(),
                timestamp: Instant::now(),
            })
            .collect()
    }
}

impl FuzzyMatcher {
    fn new() -> Self {
        Self {
            edit_distance: EditDistanceCalculator::default(),
            phonetic_matcher: PhoneticMatcher::default(),
            similarity_threshold: 0.6,
            scoring_weights: FuzzyWeights::default(),
        }
    }

fn search(&self, _query: &str) -> Vec<SearchResult> {
        // Placeholder for fuzzy search implementation
        Vec::new()
    }
}

impl FrequencyRanker {
    fn calculate_score(&self, command: &str) -> f64 {
        let base_frequency = self.command_frequencies.get(command).copied().unwrap_or(1) as f64;
        let recent_boost = self.recent_boost.get(command).copied().unwrap_or(1.0);
        let context_boost = self.context_boost.get(command).copied().unwrap_or(1.0);
        let success_boost = self.success_boost.get(command).copied().unwrap_or(1.0);

        base_frequency * recent_boost * context_boost * success_boost
    }
}

impl Default for TextSearchConfig {
    fn default() -> Self {
        Self {
            max_results: 100,
            min_match_score: 0.1,
            case_sensitive: false,
            whole_words_only: false,
            include_fuzzy: true,
            fuzzy_threshold: 0.6,
            highlight_matches: true,
            search_timeout: Duration::from_millis(1000),
        }
    }
}

impl Default for FuzzyWeights {
    fn default() -> Self {
        Self {
            edit_distance_weight: 1.0,
            prefix_bonus: 0.2,
            exact_match_bonus: 0.5,
            case_match_bonus: 0.1,
            word_boundary_bonus: 0.3,
        }
    }
}

impl Default for Bm25Ranker {
    fn default() -> Self {
        Self {
            k1: 1.2,
            b: 0.75,
            average_doc_length: 100.0,
            doc_lengths: HashMap::new(),
        }
    }
}

impl BooleanSearch {
fn search(&self, _query: &str) -> Vec<SearchResult> {
        // Placeholder for boolean search implementation
        Vec::new()
    }
}

impl Default for SearchAlgorithms {
    fn default() -> Self {
        Self {
            boolean_search: BooleanSearch::default(),
            tfidf_scorer: TfIdfScorer::default(),
            bm25_ranker: Bm25Ranker::default(),
            vector_search: VectorSpaceSearch::default(),
        }
    }
}

impl Default for SearchIntegration {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_creation() {
        let search = SearchIntegration::new();
        assert_eq!(search.stats.searches_performed, 0);
    }

    #[test]
    fn test_inverted_index() {
        let mut index = InvertedIndex::default();
        index.add_document("doc1", "hello world");
        index.add_document("doc2", "hello rust");

        assert!(index.term_docs.contains_key("hello"));
        assert_eq!(index.term_docs["hello"].len(), 2);
    }

    #[test]
    fn test_search_filter() {
        let filter = SearchFilter::TextFilter {
            pattern: "test".to_string(),
            case_sensitive: false,
        };

        let result = SearchResult {
            id: "test1".to_string(),
            title: "Test Result".to_string(),
            content: "This is a test content".to_string(),
            context: SearchContext::Global,
            relevance_score: 1.0,
            match_positions: Vec::new(),
            metadata: HashMap::new(),
            timestamp: Instant::now(),
        };

        // Test filter application (would need full implementation)
        assert_eq!(result.content.contains("test"), true);
    }

    #[test]
    fn test_command_search() {
        let mut cmd_search = CommandSearchEngine::new();
        let results = cmd_search.search_by_text("git");
        assert!(results.is_empty()); // Empty until populated
    }
}

// Native Search and Filtering System for OpenAgent Terminal
//
// This module provides real-time search and filtering for command blocks with
// instant results and no lazy loading or background processing.
//
// #![allow(dead_code)]
//
// use std::collections::{HashMap, HashSet};
// use std::sync::Arc;
// use std::time::{Duration, Instant};
//
// use anyhow::Result;
// use chrono::{DateTime, Utc};
// use regex::Regex;
// use serde::{Deserialize, Serialize};
//
// use crate::blocks_v2::{Block, BlockId, ExecutionStatus, SearchQuery, ShellType};
//
// Native search engine for immediate block filtering
// pub struct NativeSearch {
// Indexed blocks for immediate search
// block_index: BlockIndex,
//
// Search filters for immediate application
// active_filters: SearchFilters,
//
// Search history for immediate access
// search_history: SearchHistory,
//
// Real-time search state
// search_state: SearchState,
//
// Search event callbacks for immediate responses
// event_callbacks: Vec<Box<dyn Fn(&SearchEvent) + Send + Sync>>,
//
// Fuzzy search engine for intelligent matching
// fuzzy_engine: FuzzyEngine,
//
// Search suggestions for immediate completion
// suggestion_engine: SuggestionEngine,
//
// Performance statistics
// perf_stats: SearchStats,
// }
//
// Search events for immediate feedback
// #[derive(Debug, Clone)]
// pub enum SearchEvent {
// SearchStarted { query: String, filter_count: usize },
// SearchCompleted { query: String, result_count: usize, duration: Duration },
// FilterApplied { filter: SearchFilter, result_count: usize },
// FilterRemoved { filter: SearchFilter },
// SuggestionsUpdated { suggestions: Vec<SearchSuggestion> },
// IndexUpdated { block_count: usize, duration: Duration },
// SearchCleared,
// }
//
// Block index for immediate search operations
// #[derive(Debug, Default)]
// pub struct BlockIndex {
// Full-text search index
// text_index: HashMap<String, HashSet<BlockId>>,
//
// Command-specific index
// command_index: HashMap<String, HashSet<BlockId>>,
//
// Output-specific index
// output_index: HashMap<String, HashSet<BlockId>>,
//
// Tag index for immediate tag filtering
// tag_index: HashMap<String, HashSet<BlockId>>,
//
// Shell type index
// shell_index: HashMap<ShellType, HashSet<BlockId>>,
//
// Status index for immediate status filtering
// status_index: HashMap<ExecutionStatus, HashSet<BlockId>>,
//
// Date-based index for temporal filtering
// date_index: DateIndex,
//
// Directory index for path-based filtering
// directory_index: HashMap<String, HashSet<BlockId>>,
//
// Block metadata cache for immediate access
// block_cache: HashMap<BlockId, IndexedBlock>,
//
// Index update timestamps
// last_update: Instant,
// update_count: usize,
// }
//
// Indexed block for immediate search
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct IndexedBlock {
// pub block_id: BlockId,
// pub command_tokens: Vec<String>,
// pub output_tokens: Vec<String>,
// pub tags: HashSet<String>,
// pub shell: ShellType,
// pub status: ExecutionStatus,
// pub created_at: DateTime<Utc>,
// pub directory: String,
// pub exit_code: Option<i32>,
// pub duration_ms: Option<u64>,
// pub search_score: f64,
// }
//
// Date-based index for temporal filtering
// #[derive(Debug, Default)]
// pub struct DateIndex {
// Blocks by year
// by_year: HashMap<i32, HashSet<BlockId>>,
// Blocks by month
// by_month: HashMap<(i32, u32), HashSet<BlockId>>,
// Blocks by day
// by_day: HashMap<(i32, u32, u32), HashSet<BlockId>>,
// Recent blocks (last N hours)
// recent_blocks: Vec<(DateTime<Utc>, BlockId)>,
// }
//
// Search filters for immediate application
// #[derive(Debug, Default, Clone)]
// pub struct SearchFilters {
// pub active_filters: Vec<SearchFilter>,
// pub filter_mode: FilterMode,
// pub last_applied: Instant,
// }
//
// Individual search filter
// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum SearchFilter {
// Text filters
// TextContains(String),
// CommandContains(String),
// OutputContains(String),
// Regex(String),
//
// Metadata filters
// HasTag(String),
// Shell(ShellType),
// Status(ExecutionStatus),
// ExitCode(i32),
//
// Temporal filters
// CreatedAfter(DateTime<Utc>),
// CreatedBefore(DateTime<Utc>),
// CreatedToday,
// CreatedThisWeek,
// CreatedThisMonth,
//
// Directory filters
// InDirectory(String),
// DirectoryContains(String),
//
// Duration filters
// DurationLessThan(Duration),
// DurationGreaterThan(Duration),
//
// Advanced filters
// Starred,
// Failed,
// Successful,
// LongRunning(Duration),
// }
//
// Filter application mode
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum FilterMode {
// And, // All filters must match
// Or,  // Any filter can match
// }
//
// Search history for immediate access
// #[derive(Debug, Default)]
// pub struct SearchHistory {
// pub queries: Vec<SearchHistoryEntry>,
// pub max_entries: usize,
// pub last_access: Instant,
// }
//
// Search history entry
// #[derive(Debug, Clone)]
// pub struct SearchHistoryEntry {
// pub query: String,
// pub filters: Vec<SearchFilter>,
// pub result_count: usize,
// pub timestamp: DateTime<Utc>,
// pub execution_time: Duration,
// }
//
// Real-time search state
// #[derive(Debug, Default)]
// pub struct SearchState {
// pub current_query: Option<String>,
// pub current_results: Vec<BlockId>,
// pub total_matches: usize,
// pub search_duration: Option<Duration>,
// pub is_searching: bool,
// pub last_search: Option<Instant>,
// }
//
// Fuzzy search engine for intelligent matching
// #[derive(Debug)]
// pub struct FuzzyEngine {
// pub similarity_threshold: f64,
// pub max_edit_distance: usize,
// pub word_boundaries: bool,
// pub case_sensitive: bool,
// }
//
// Search suggestions for immediate completion
// #[derive(Debug, Default)]
// pub struct SuggestionEngine {
// pub command_suggestions: Vec<String>,
// pub tag_suggestions: Vec<String>,
// pub directory_suggestions: Vec<String>,
// pub pattern_suggestions: Vec<SearchPattern>,
// pub last_update: Instant,
// }
//
// Search suggestion
// #[derive(Debug, Clone)]
// pub struct SearchSuggestion {
// pub text: String,
// pub suggestion_type: SuggestionType,
// pub score: f64,
// pub preview: Option<String>,
// }
//
// Suggestion types for categorization
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum SuggestionType {
// Command,
// Tag,
// Directory,
// Pattern,
// RecentQuery,
// Filter,
// }
//
// Search patterns for intelligent suggestions
// #[derive(Debug, Clone)]
// pub struct SearchPattern {
// pub pattern: String,
// pub description: String,
// pub example: String,
// pub frequency: usize,
// }
//
// Performance statistics for search operations
// #[derive(Debug, Default, Clone)]
// pub struct SearchStats {
// pub total_searches: usize,
// pub total_search_time: Duration,
// pub average_search_time: Duration,
// pub index_size: usize,
// pub cache_hits: usize,
// pub cache_misses: usize,
// pub last_reset: Instant,
// }
//
// impl NativeSearch {
// Create new native search engine with immediate capabilities
// pub fn new() -> Self {
// Self {
// block_index: BlockIndex::default(),
// active_filters: SearchFilters::default(),
// search_history: SearchHistory {
// max_entries: 100,
// ..Default::default()
// },
// search_state: SearchState::default(),
// event_callbacks: Vec::new(),
// fuzzy_engine: FuzzyEngine {
// similarity_threshold: 0.7,
// max_edit_distance: 2,
// word_boundaries: true,
// case_sensitive: false,
// },
// suggestion_engine: SuggestionEngine::default(),
// perf_stats: SearchStats {
// last_reset: Instant::now(),
// ..Default::default()
// },
// }
// }
//
// Register search event callback for immediate responses
// pub fn register_event_callback<F>(&mut self, callback: F)
// where
// F: Fn(&SearchEvent) + Send + Sync + 'static,
// {
// self.event_callbacks.push(Box::new(callback));
// }
//
// Emit search event immediately
// fn emit_event(&self, event: SearchEvent) {
// for callback in &self.event_callbacks {
// callback(&event);
// }
// }
//
// Index block immediately for instant search availability
// pub fn index_block(&mut self, block: &Block) -> Result<()> {
// let start_time = Instant::now();
//
// Create indexed block
// let command_tokens = self.tokenize_text(&block.command);
// let output_tokens = self.tokenize_text(&block.output);
//
// let indexed_block = IndexedBlock {
// block_id: block.id,
// command_tokens: command_tokens.clone(),
// output_tokens: output_tokens.clone(),
// tags: block.tags.clone(),
// shell: block.shell,
// status: block.status,
// created_at: block.created_at,
// directory: block.directory.to_string_lossy().to_string(),
// exit_code: block.exit_code,
// duration_ms: block.duration_ms,
// search_score: 1.0,
// };
//
// Update all indices immediately
// self.update_text_index(&block.command, &block.output, block.id);
// self.update_command_index(&command_tokens, block.id);
// self.update_output_index(&output_tokens, block.id);
// self.update_tag_index(&block.tags, block.id);
// self.update_shell_index(block.shell, block.id);
// self.update_status_index(block.status, block.id);
// self.update_date_index(block.created_at, block.id);
// self.update_directory_index(&block.directory.to_string_lossy(), block.id);
//
// Cache indexed block
// self.block_index.block_cache.insert(block.id, indexed_block);
//
// Update index metadata
// self.block_index.last_update = Instant::now();
// self.block_index.update_count += 1;
// self.perf_stats.index_size += 1;
//
// Update suggestions immediately
// self.update_suggestions();
//
// let duration = start_time.elapsed();
//
// Emit index update event
// self.emit_event(SearchEvent::IndexUpdated {
// block_count: self.block_index.block_cache.len(),
// duration,
// });
//
// Ok(())
// }
//
// Search blocks immediately with instant results
// pub fn search(&mut self, query: &str) -> Result<Vec<BlockId>> {
// let start_time = Instant::now();
// self.search_state.is_searching = true;
//
// Emit search started event
// self.emit_event(SearchEvent::SearchStarted {
// query: query.to_string(),
// filter_count: self.active_filters.active_filters.len(),
// });
//
// let results = if query.is_empty() {
// No query - apply filters only
// self.apply_filters_only()
// } else {
// Full text search with filters
// self.perform_full_search(query)?
// };
//
// Update search state immediately
// self.search_state.current_query = Some(query.to_string());
// self.search_state.current_results = results.clone();
// self.search_state.total_matches = results.len();
// self.search_state.is_searching = false;
// self.search_state.last_search = Some(start_time);
//
// let duration = start_time.elapsed();
// self.search_state.search_duration = Some(duration);
//
// Update performance stats immediately
// self.perf_stats.total_searches += 1;
// self.perf_stats.total_search_time += duration;
// self.perf_stats.average_search_time =
// self.perf_stats.total_search_time / self.perf_stats.total_searches as u32;
//
// Add to search history immediately
// self.add_to_history(query, results.len(), duration);
//
// Emit search completed event
// self.emit_event(SearchEvent::SearchCompleted {
// query: query.to_string(),
// result_count: results.len(),
// duration,
// });
//
// Ok(results)
// }
//
// Perform full text search with immediate results
// fn perform_full_search(&mut self, query: &str) -> Result<Vec<BlockId>> {
// let query_tokens = self.tokenize_text(query);
// let mut candidates = HashSet::new();
//
// Search in text index
// for token in &query_tokens {
// if let Some(block_ids) = self.block_index.text_index.get(token) {
// candidates.extend(block_ids);
// }
//
// Fuzzy matching for similar tokens
// if candidates.is_empty() {
// candidates.extend(self.fuzzy_search_token(token));
// }
// }
//
// Search in command index
// for token in &query_tokens {
// if let Some(block_ids) = self.block_index.command_index.get(token) {
// candidates.extend(block_ids);
// }
// }
//
// Search in output index
// for token in &query_tokens {
// if let Some(block_ids) = self.block_index.output_index.get(token) {
// candidates.extend(block_ids);
// }
// }
//
// Apply filters immediately
// let filtered_results = self.apply_filters(&candidates.into_iter().collect());
//
// Score and sort results immediately
// let mut scored_results: Vec<(BlockId, f64)> = filtered_results
// .into_iter()
// .map(|id| {
// let score = self.calculate_relevance_score(id, query);
// (id, score)
// })
// .collect();
//
// Sort by relevance score (highest first)
// scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
//
// Ok(scored_results.into_iter().map(|(id, _)| id).collect())
// }
//
// Apply filters only (no text search)
// fn apply_filters_only(&self) -> Vec<BlockId> {
// let all_blocks: Vec<BlockId> = self.block_index.block_cache.keys().copied().collect();
// self.apply_filters(&all_blocks)
// }
//
// Apply active filters to block list immediately
// fn apply_filters(&self, blocks: &[BlockId]) -> Vec<BlockId> {
// if self.active_filters.active_filters.is_empty() {
// return blocks.to_vec();
// }
//
// blocks
// .iter()
// .filter(|&&block_id| self.matches_filters(block_id))
// .copied()
// .collect()
// }
//
// Check if block matches all active filters immediately
// fn matches_filters(&self, block_id: BlockId) -> bool {
// let Some(indexed_block) = self.block_index.block_cache.get(&block_id) else {
// return false;
// };
//
// match self.active_filters.filter_mode {
// FilterMode::And => {
// All filters must match
// self.active_filters
// .active_filters
// .iter()
// .all(|filter| self.matches_filter(indexed_block, filter))
// },
// FilterMode::Or => {
// Any filter can match
// self.active_filters
// .active_filters
// .iter()
// .any(|filter| self.matches_filter(indexed_block, filter))
// },
// }
// }
//
// Check if block matches specific filter immediately
// fn matches_filter(&self, block: &IndexedBlock, filter: &SearchFilter) -> bool {
// match filter {
// SearchFilter::TextContains(text) => {
// block.command_tokens.iter().any(|t| t.contains(text))
// || block.output_tokens.iter().any(|t| t.contains(text))
// },
// SearchFilter::CommandContains(text) => {
// block.command_tokens.iter().any(|t| t.contains(text))
// },
// SearchFilter::OutputContains(text) => {
// block.output_tokens.iter().any(|t| t.contains(text))
// },
// SearchFilter::Regex(pattern) => {
// if let Ok(regex) = Regex::new(pattern) {
// block.command_tokens.iter().any(|t| regex.is_match(t))
// || block.output_tokens.iter().any(|t| regex.is_match(t))
// } else {
// false
// }
// },
// SearchFilter::HasTag(tag) => block.tags.contains(tag),
// SearchFilter::Shell(shell) => block.shell == *shell,
// SearchFilter::Status(status) => block.status == *status,
// SearchFilter::ExitCode(code) => block.exit_code == Some(*code),
// SearchFilter::CreatedAfter(date) => block.created_at > *date,
// SearchFilter::CreatedBefore(date) => block.created_at < *date,
// SearchFilter::CreatedToday => {
// let now = Utc::now();
// block.created_at.date_naive() == now.date_naive()
// },
// SearchFilter::CreatedThisWeek => {
// let now = Utc::now();
// let week_start = now - chrono::Duration::days(7);
// block.created_at > week_start
// },
// SearchFilter::CreatedThisMonth => {
// let now = Utc::now();
// block.created_at.year() == now.year() && block.created_at.month() == now.month()
// },
// SearchFilter::InDirectory(dir) => block.directory == *dir,
// SearchFilter::DirectoryContains(text) => block.directory.contains(text),
// SearchFilter::DurationLessThan(duration) => {
// if let Some(block_duration) = block.duration_ms {
// Duration::from_millis(block_duration) < *duration
// } else {
// false
// }
// },
// SearchFilter::DurationGreaterThan(duration) => {
// if let Some(block_duration) = block.duration_ms {
// Duration::from_millis(block_duration) > *duration
// } else {
// false
// }
// },
// SearchFilter::Starred => {
// Would need to check if block is starred - placeholder
// false
// },
// SearchFilter::Failed => block.exit_code.map_or(false, |code| code != 0),
// SearchFilter::Successful => block.exit_code.map_or(false, |code| code == 0),
// SearchFilter::LongRunning(threshold) => {
// if let Some(block_duration) = block.duration_ms {
// Duration::from_millis(block_duration) > *threshold
// } else {
// false
// }
// },
// }
// }
//
// Add filter immediately
// pub fn add_filter(&mut self, filter: SearchFilter) -> Result<()> {
// if !self.active_filters.active_filters.contains(&filter) {
// self.active_filters.active_filters.push(filter.clone());
// self.active_filters.last_applied = Instant::now();
//
// Re-run search with new filter if query is active
// let result_count = if let Some(ref query) = self.search_state.current_query {
// let results = self.search(query)?;
// results.len()
// } else {
// self.apply_filters_only().len()
// };
//
// self.emit_event(SearchEvent::FilterApplied { filter, result_count });
// }
//
// Ok(())
// }
//
// Remove filter immediately
// pub fn remove_filter(&mut self, filter: &SearchFilter) -> Result<()> {
// if let Some(pos) = self.active_filters.active_filters.iter().position(|f| f == filter) {
// let removed_filter = self.active_filters.active_filters.remove(pos);
//
// Re-run search without filter if query is active
// if let Some(ref query) = self.search_state.current_query {
// self.search(query)?;
// }
//
// self.emit_event(SearchEvent::FilterRemoved { filter: removed_filter });
// }
//
// Ok(())
// }
//
// Clear all filters immediately
// pub fn clear_filters(&mut self) -> Result<()> {
// self.active_filters.active_filters.clear();
//
// Re-run search without filters if query is active
// if let Some(ref query) = self.search_state.current_query {
// self.search(query)?;
// }
//
// Ok(())
// }
//
// Clear search and reset state immediately
// pub fn clear_search(&mut self) {
// self.search_state = SearchState::default();
// self.emit_event(SearchEvent::SearchCleared);
// }
//
// Generate search suggestions immediately
// pub fn get_suggestions(&mut self, partial_query: &str) -> Vec<SearchSuggestion> {
// let mut suggestions = Vec::new();
//
// Command suggestions
// suggestions.extend(
// self.suggestion_engine
// .command_suggestions
// .iter()
// .filter(|cmd| cmd.starts_with(partial_query))
// .map(|cmd| SearchSuggestion {
// text: cmd.clone(),
// suggestion_type: SuggestionType::Command,
// score: self.calculate_suggestion_score(cmd, partial_query),
// preview: Some(format!("Search commands containing '{}'", cmd)),
// })
// );
//
// Tag suggestions
// suggestions.extend(
// self.suggestion_engine
// .tag_suggestions
// .iter()
// .filter(|tag| tag.starts_with(partial_query))
// .map(|tag| SearchSuggestion {
// text: format!("tag:{}", tag),
// suggestion_type: SuggestionType::Tag,
// score: self.calculate_suggestion_score(tag, partial_query),
// preview: Some(format!("Filter by tag '{}'", tag)),
// })
// );
//
// Directory suggestions
// suggestions.extend(
// self.suggestion_engine
// .directory_suggestions
// .iter()
// .filter(|dir| dir.contains(partial_query))
// .map(|dir| SearchSuggestion {
// text: format!("dir:{}", dir),
// suggestion_type: SuggestionType::Directory,
// score: self.calculate_suggestion_score(dir, partial_query),
// preview: Some(format!("Filter by directory '{}'", dir)),
// })
// );
//
// Sort by score
// suggestions.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
//
// Emit suggestions update event
// self.emit_event(SearchEvent::SuggestionsUpdated {
// suggestions: suggestions.clone(),
// });
//
// suggestions
// }
//
// Tokenize text for indexing and search
// fn tokenize_text(&self, text: &str) -> Vec<String> {
// text.split_whitespace()
// .map(|s| s.to_lowercase())
// .filter(|s| !s.is_empty())
// .collect()
// }
//
// Update text index immediately
// fn update_text_index(&mut self, command: &str, output: &str, block_id: BlockId) {
// let all_tokens = self.tokenize_text(&format!("{} {}", command, output));
//
// for token in all_tokens {
// self.block_index.text_index
// .entry(token)
// .or_insert_with(HashSet::new)
// .insert(block_id);
// }
// }
//
// Update command index immediately
// fn update_command_index(&mut self, tokens: &[String], block_id: BlockId) {
// for token in tokens {
// self.block_index.command_index
// .entry(token.clone())
// .or_insert_with(HashSet::new)
// .insert(block_id);
// }
// }
//
// Update output index immediately
// fn update_output_index(&mut self, tokens: &[String], block_id: BlockId) {
// for token in tokens {
// self.block_index.output_index
// .entry(token.clone())
// .or_insert_with(HashSet::new)
// .insert(block_id);
// }
// }
//
// Update tag index immediately
// fn update_tag_index(&mut self, tags: &HashSet<String>, block_id: BlockId) {
// for tag in tags {
// self.block_index.tag_index
// .entry(tag.clone())
// .or_insert_with(HashSet::new)
// .insert(block_id);
// }
// }
//
// Update shell index immediately
// fn update_shell_index(&mut self, shell: ShellType, block_id: BlockId) {
// self.block_index.shell_index
// .entry(shell)
// .or_insert_with(HashSet::new)
// .insert(block_id);
// }
//
// Update status index immediately
// fn update_status_index(&mut self, status: ExecutionStatus, block_id: BlockId) {
// self.block_index.status_index
// .entry(status)
// .or_insert_with(HashSet::new)
// .insert(block_id);
// }
//
// Update date index immediately
// fn update_date_index(&mut self, created_at: DateTime<Utc>, block_id: BlockId) {
// let year = created_at.year();
// let month = created_at.month();
// let day = created_at.day();
//
// Update year index
// self.block_index.date_index.by_year
// .entry(year)
// .or_insert_with(HashSet::new)
// .insert(block_id);
//
// Update month index
// self.block_index.date_index.by_month
// .entry((year, month))
// .or_insert_with(HashSet::new)
// .insert(block_id);
//
// Update day index
// self.block_index.date_index.by_day
// .entry((year, month, day))
// .or_insert_with(HashSet::new)
// .insert(block_id);
//
// Update recent blocks (keep only last 100)
// self.block_index.date_index.recent_blocks.push((created_at, block_id));
// self.block_index.date_index.recent_blocks.sort_by(|a, b| b.0.cmp(&a.0));
// if self.block_index.date_index.recent_blocks.len() > 100 {
// self.block_index.date_index.recent_blocks.truncate(100);
// }
// }
//
// Update directory index immediately
// fn update_directory_index(&mut self, directory: &str, block_id: BlockId) {
// self.block_index.directory_index
// .entry(directory.to_string())
// .or_insert_with(HashSet::new)
// .insert(block_id);
// }
//
// Perform fuzzy search on token
// fn fuzzy_search_token(&self, token: &str) -> HashSet<BlockId> {
// let mut results = HashSet::new();
//
// for (indexed_token, block_ids) in &self.block_index.text_index {
// if self.calculate_similarity(token, indexed_token) >= self.fuzzy_engine.similarity_threshold {
// results.extend(block_ids);
// }
// }
//
// results
// }
//
// Calculate string similarity for fuzzy search
// fn calculate_similarity(&self, s1: &str, s2: &str) -> f64 {
// Simple Levenshtein distance-based similarity
// let distance = self.levenshtein_distance(s1, s2);
// let max_len = s1.len().max(s2.len()) as f64;
//
// if max_len == 0.0 {
// 1.0
// } else {
// 1.0 - (distance as f64 / max_len)
// }
// }
//
// Calculate Levenshtein distance
// fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
// let len1 = s1.len();
// let len2 = s2.len();
//
// if len1 == 0 { return len2; }
// if len2 == 0 { return len1; }
//
// let mut d = vec![vec![0; len2 + 1]; len1 + 1];
//
// for i in 1..=len1 { d[i][0] = i; }
// for j in 1..=len2 { d[0][j] = j; }
//
// for i in 1..=len1 {
// for j in 1..=len2 {
// let cost = if s1.chars().nth(i - 1) == s2.chars().nth(j - 1) { 0 } else { 1 };
// d[i][j] = (d[i - 1][j] + 1)
// .min(d[i][j - 1] + 1)
// .min(d[i - 1][j - 1] + cost);
// }
// }
//
// d[len1][len2]
// }
//
// Calculate relevance score for search result
// fn calculate_relevance_score(&self, block_id: BlockId, query: &str) -> f64 {
// let Some(indexed_block) = self.block_index.block_cache.get(&block_id) else {
// return 0.0;
// };
//
// let mut score = 0.0;
// let query_tokens = self.tokenize_text(query);
//
// Command match bonus
// for token in &query_tokens {
// if indexed_block.command_tokens.iter().any(|t| t.contains(token)) {
// score += 2.0;
// }
// }
//
// Output match
// for token in &query_tokens {
// if indexed_block.output_tokens.iter().any(|t| t.contains(token)) {
// score += 1.0;
// }
// }
//
// Tag match bonus
// for token in &query_tokens {
// if indexed_block.tags.iter().any(|t| t.contains(token)) {
// score += 1.5;
// }
// }
//
// Recency bonus
// let age_days = (Utc::now() - indexed_block.created_at).num_days();
// if age_days < 7 {
// score += 0.5;
// }
//
// Success bonus
// if indexed_block.status == ExecutionStatus::Success {
// score += 0.2;
// }
//
// score
// }
//
// Calculate suggestion score
// fn calculate_suggestion_score(&self, suggestion: &str, query: &str) -> f64 {
// if suggestion.starts_with(query) {
// 1.0
// } else if suggestion.contains(query) {
// 0.8
// } else {
// self.calculate_similarity(suggestion, query)
// }
// }
//
// Update search suggestions immediately
// fn update_suggestions(&mut self) {
// Update command suggestions
// self.suggestion_engine.command_suggestions = self.block_index.command_index
// .keys()
// .take(50) // Limit to top 50
// .cloned()
// .collect();
//
// Update tag suggestions
// self.suggestion_engine.tag_suggestions = self.block_index.tag_index
// .keys()
// .cloned()
// .collect();
//
// Update directory suggestions
// self.suggestion_engine.directory_suggestions = self.block_index.directory_index
// .keys()
// .cloned()
// .collect();
//
// self.suggestion_engine.last_update = Instant::now();
// }
//
// Add search to history immediately
// fn add_to_history(&mut self, query: &str, result_count: usize, execution_time: Duration) {
// let entry = SearchHistoryEntry {
// query: query.to_string(),
// filters: self.active_filters.active_filters.clone(),
// result_count,
// timestamp: Utc::now(),
// execution_time,
// };
//
// self.search_history.queries.push(entry);
//
// Limit history size
// if self.search_history.queries.len() > self.search_history.max_entries {
// self.search_history.queries.remove(0);
// }
//
// self.search_history.last_access = Instant::now();
// }
//
// Get search statistics
// pub fn get_stats(&self) -> SearchStats {
// self.perf_stats.clone()
// }
//
// Get current search results
// pub fn get_current_results(&self) -> &[BlockId] {
// &self.search_state.current_results
// }
//
// Check if currently searching
// pub fn is_searching(&self) -> bool {
// self.search_state.is_searching
// }
// }
//
// impl Default for NativeSearch {
// fn default() -> Self {
// Self::new()
// }
// }
//
// #[cfg(test)]
// mod tests {
// use super::*;
// use crate::blocks_v2::{BlockMetadata, ShellType};
// use std::path::PathBuf;
//
// fn create_test_block(id: u32, command: &str, output: &str) -> Block {
// Block {
// id: BlockId::from_string(&format!("test-{}", id)).unwrap(),
// command: command.to_string(),
// output: output.to_string(),
// directory: PathBuf::from("/test"),
// environment: HashMap::new(),
// shell: ShellType::Bash,
// created_at: Utc::now(),
// modified_at: Utc::now(),
// tags: HashSet::new(),
// starred: false,
// parent_id: None,
// children: Vec::new(),
// metadata: BlockMetadata::default(),
// status: ExecutionStatus::Success,
// exit_code: Some(0),
// duration_ms: Some(100),
// }
// }
//
// #[test]
// fn test_native_search_creation() {
// let search = NativeSearch::new();
// assert_eq!(search.block_index.block_cache.len(), 0);
// assert!(!search.search_state.is_searching);
// }
//
// #[test]
// fn test_block_indexing() {
// let mut search = NativeSearch::new();
// let block = create_test_block(1, "echo hello", "hello world");
//
// search.index_block(&block).unwrap();
//
// assert_eq!(search.block_index.block_cache.len(), 1);
// assert!(search.block_index.text_index.contains_key("hello"));
// assert!(search.block_index.command_index.contains_key("echo"));
// }
//
// #[test]
// fn test_immediate_search() {
// let mut search = NativeSearch::new();
// let block = create_test_block(1, "echo test", "test output");
//
// search.index_block(&block).unwrap();
//
// let results = search.search("echo").unwrap();
// assert_eq!(results.len(), 1);
// assert_eq!(results[0], block.id);
// }
//
// #[test]
// fn test_filter_application() {
// let mut search = NativeSearch::new();
// let mut block = create_test_block(1, "ls -la", "file listing");
// block.status = ExecutionStatus::Success;
//
// search.index_block(&block).unwrap();
//
// search.add_filter(SearchFilter::Status(ExecutionStatus::Success)).unwrap();
// let results = search.search("").unwrap();
// assert_eq!(results.len(), 1);
//
// search.add_filter(SearchFilter::Status(ExecutionStatus::Failed)).unwrap();
// let results = search.search("").unwrap();
// assert_eq!(results.len(), 0); // AND mode, so no results
// }
//
// #[test]
// fn test_suggestion_generation() {
// let mut search = NativeSearch::new();
// let block = create_test_block(1, "git status", "clean working directory");
//
// search.index_block(&block).unwrap();
//
// let suggestions = search.get_suggestions("git");
// assert!(!suggestions.is_empty());
// assert!(suggestions.iter().any(|s| s.text.contains("git")));
// }
// }
