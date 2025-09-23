//! Warp-style Smart History and Search System
//!
//! Provides Warp-inspired intelligent command history with:
//! - AI-enhanced semantic search
//! - Context-aware command suggestions
//! - Pattern recognition and learning
//! - Smart filtering and categorization

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

use crate::ai::warp_integration::{WarpAiIntegration, ContextAnalyzer};
use crate::shell_integration::{CommandId, ShellEvent};

/// Warp-style smart history manager
pub struct WarpHistoryManager {
    /// History entries with enhanced metadata
    history: VecDeque<HistoryEntry>,
    
    /// AI integration for semantic search
    ai_integration: Option<WarpAiIntegration>,
    
    /// Search engine for fast lookups
    search_engine: HistorySearchEngine,
    
    /// Pattern analyzer for learning user habits
    pattern_analyzer: CommandPatternAnalyzer,
    
    /// Context matcher for relevant suggestions
    context_matcher: ContextMatcher,
    
    /// Configuration
    config: HistoryConfig,
    
    /// Performance metrics
    metrics: HistoryMetrics,
}

/// Enhanced history entry with rich metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Unique ID
    pub id: String,
    
    /// Original command
    pub command: String,
    
    /// Normalized command (for pattern matching)
    pub normalized_command: String,
    
    /// Command parts breakdown
    pub command_parts: Vec<CommandPart>,
    
    /// Execution metadata
    pub execution: ExecutionMetadata,
    
    /// Context information
    pub context: CommandContext,
    
    /// AI insights
    pub ai_insights: HistoryAiInsights,
    
    /// Usage statistics
    pub usage_stats: CommandUsageStats,
    
    /// Tags and categories
    pub tags: Vec<String>,
    
    /// User annotations
    pub annotations: Vec<String>,
}

/// Command part for semantic analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandPart {
    /// Part text
    pub text: String,
    
    /// Part type (command, flag, argument, etc.)
    pub part_type: CommandPartType,
    
    /// Semantic meaning
    pub semantic_type: Option<SemanticType>,
    
    /// Confidence score
    pub confidence: f32,
}

/// Types of command parts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandPartType {
    Command,
    Subcommand,
    Flag,
    LongFlag,
    Argument,
    File,
    Directory,
    Url,
    Variable,
    Pipe,
    Redirect,
    Operator,
}

/// Semantic types for AI understanding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SemanticType {
    FileManagement,
    GitOperation,
    NetworkRequest,
    ProcessManagement,
    SystemAdmin,
    Development,
    Database,
    Docker,
    Package,
    Build,
    Test,
    Deploy,
    Monitor,
    Security,
}

/// Execution metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    /// Exit code
    pub exit_code: Option<i32>,
    
    /// Execution duration
    pub duration: Option<Duration>,
    
    /// Output summary
    pub output_summary: Option<String>,
    
    /// Error information
    pub error_info: Option<String>,
    
    /// Resource usage
    pub resource_usage: Option<ResourceUsage>,
}

/// Resource usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_time: Option<Duration>,
    pub memory_peak: Option<usize>,
    pub io_read: Option<u64>,
    pub io_write: Option<u64>,
    pub network_sent: Option<u64>,
    pub network_received: Option<u64>,
}

/// Command execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandContext {
    /// Working directory
    pub working_directory: PathBuf,
    
    /// Environment variables
    pub environment: HashMap<String, String>,
    
    /// Shell type
    pub shell: String,
    
    /// Terminal size
    pub terminal_size: Option<(u16, u16)>,
    
    /// Git context
    pub git_context: Option<GitContext>,
    
    /// Project context
    pub project_context: Option<ProjectContext>,
    
    /// Time context
    pub time_context: TimeContext,
}

/// Git repository context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitContext {
    pub is_repository: bool,
    pub branch: Option<String>,
    pub commit_hash: Option<String>,
    pub has_changes: bool,
    pub remote_origin: Option<String>,
}

/// Project context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub project_type: Option<ProjectType>,
    pub project_name: Option<String>,
    pub config_files: Vec<String>,
    pub dependencies: Vec<String>,
}

/// Types of projects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectType {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Go,
    Java,
    C,
    Cpp,
    Web,
    Mobile,
    Docker,
    Kubernetes,
}

/// Time-based context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeContext {
    pub timestamp: DateTime<Utc>,
    pub hour_of_day: u8,
    pub day_of_week: chrono::Weekday,
}

/// AI insights for history entries
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoryAiInsights {
    /// Command intent classification
    pub intent: Option<CommandIntent>,
    
    /// Risk assessment
    pub risk_level: Option<crate::ai::warp_integration::RiskLevel>,
    
    /// Semantic embedding (for similarity search)
    pub embedding: Option<Vec<f32>>,
    
    /// Related commands
    pub related_commands: Vec<String>,
    
    /// Performance insights
    pub performance_notes: Vec<String>,
    
    /// Learning insights
    pub learning_insights: Vec<String>,
}

/// Command intent classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandIntent {
    Explore,          // ls, find, grep
    Modify,           // edit, create, delete
    Process,          // run, execute, compile
    Transfer,         // copy, move, sync
    Monitor,          // top, ps, logs
    Configure,        // config, setup, install
    Debug,            // debug, trace, profile
    Communicate,      // curl, ssh, ping
    Archive,          // tar, zip, backup
    Search,           // grep, find, locate
}

/// Command usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandUsageStats {
    /// Total usage count
    pub usage_count: u64,
    
    /// Success rate (0.0 to 1.0)
    pub success_rate: f32,
    
    /// Average execution time
    pub avg_execution_time: Option<Duration>,
    
    /// Last used timestamp
    pub last_used: DateTime<Utc>,
    
    /// Usage frequency (uses per day)
    pub usage_frequency: f32,
    
    /// Context correlation
    pub context_correlation: HashMap<String, f32>,
}

/// History search engine
#[derive(Debug)]
pub struct HistorySearchEngine {
    /// Text-based index
    text_index: TextIndex,
    
    /// Semantic index (AI embeddings)
    semantic_index: SemanticIndex,
    
    /// Context index
    context_index: ContextIndex,
    
    /// Search configuration
    search_config: SearchConfig,
}

/// Text-based search index
#[derive(Debug)]
pub struct TextIndex {
    /// Forward index (entry_id -> tokens)
    forward_index: HashMap<String, Vec<String>>,
    
    /// Inverted index (token -> entry_ids)
    inverted_index: HashMap<String, Vec<String>>,
    
    /// N-gram index for fuzzy search
    ngram_index: HashMap<String, Vec<String>>,
    
    /// Frequency scores
    token_frequencies: HashMap<String, f32>,
}

/// Semantic search index using AI embeddings
#[derive(Debug)]
pub struct SemanticIndex {
    /// Entry embeddings
    embeddings: HashMap<String, Vec<f32>>,
    
    /// Similarity cache
    similarity_cache: HashMap<(String, String), f32>,
    
    /// Clustering information
    clusters: HashMap<String, Vec<String>>,
}

/// Context-based search index
#[derive(Debug)]
pub struct ContextIndex {
    /// Directory-based index
    directory_index: HashMap<PathBuf, Vec<String>>,
    
    /// Time-based index
    time_index: HashMap<String, Vec<String>>, // time_bucket -> entry_ids
    
    /// Project-based index
    project_index: HashMap<String, Vec<String>>,
    
    /// Git-based index
    git_index: HashMap<String, Vec<String>>,
}

/// Search configuration
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Maximum results to return
    pub max_results: usize,
    
    /// Fuzzy search threshold
    pub fuzzy_threshold: f32,
    
    /// Semantic similarity threshold
    pub semantic_threshold: f32,
    
    /// Context weight in scoring
    pub context_weight: f32,
    
    /// Recency bias factor
    pub recency_bias: f32,
    
    /// Frequency bias factor
    pub frequency_bias: f32,
}

/// Pattern analyzer for learning user habits
#[derive(Debug)]
pub struct CommandPatternAnalyzer {
    /// Command sequences
    command_sequences: HashMap<String, CommandSequence>,
    
    /// Temporal patterns
    temporal_patterns: HashMap<String, TemporalPattern>,
    
    /// Context patterns
    context_patterns: HashMap<String, ContextPattern>,
    
    /// Error patterns
    error_patterns: HashMap<String, ErrorPattern>,
}

/// Command sequence patterns
#[derive(Debug, Clone)]
pub struct CommandSequence {
    pub commands: Vec<String>,
    pub frequency: u32,
    pub success_rate: f32,
    pub avg_duration: Duration,
    pub context_similarity: f32,
}

/// Temporal usage patterns
#[derive(Debug, Clone)]
pub struct TemporalPattern {
    pub command: String,
    pub time_distribution: HashMap<u8, f32>, // hour -> frequency
    pub day_distribution: HashMap<u8, f32>,  // day of week -> frequency
    pub seasonal_trends: Vec<f32>,
}

/// Context-based patterns
#[derive(Debug, Clone)]
pub struct ContextPattern {
    pub command: String,
    pub directory_affinity: HashMap<PathBuf, f32>,
    pub project_affinity: HashMap<String, f32>,
    pub environment_dependencies: Vec<String>,
}

/// Error recovery patterns
#[derive(Debug, Clone)]
pub struct ErrorPattern {
    pub failed_command: String,
    pub error_signature: String,
    pub recovery_commands: Vec<String>,
    pub success_rate: f32,
}

/// Context matcher for relevant suggestions
#[derive(Debug)]
pub struct ContextMatcher {
    /// Current context analyzer
    context_analyzer: ContextAnalyzer,
    
    /// Context similarity calculator
    similarity_calculator: ContextSimilarityCalculator,
    
    /// Suggestion ranker
    suggestion_ranker: SuggestionRanker,
}

/// Context similarity calculator
#[derive(Debug)]
pub struct ContextSimilarityCalculator {
    /// Weights for different context factors
    weights: ContextWeights,
}

/// Context weights for similarity calculation
#[derive(Debug, Clone)]
pub struct ContextWeights {
    pub directory: f32,
    pub time: f32,
    pub project: f32,
    pub git: f32,
    pub environment: f32,
    pub recent_commands: f32,
}

/// Suggestion ranking system
#[derive(Debug)]
pub struct SuggestionRanker {
    /// Ranking algorithm
    algorithm: RankingAlgorithm,
    
    /// Learning model for ranking
    learning_model: Option<LearningModel>,
}

/// Ranking algorithms
#[derive(Debug, Clone, Copy)]
pub enum RankingAlgorithm {
    Frequency,
    Recency,
    Similarity,
    Hybrid,
    Learned,
}

/// Learning model for ranking (placeholder)
#[derive(Debug)]
pub struct LearningModel {
    // Placeholder for ML model
    weights: Vec<f32>,
    features: Vec<String>,
}

/// History configuration
#[derive(Debug, Clone)]
pub struct HistoryConfig {
    /// Maximum history entries
    pub max_entries: usize,
    
    /// Persistence enabled
    pub persist_history: bool,
    
    /// History file path
    pub history_file: Option<PathBuf>,
    
    /// AI analysis enabled
    pub ai_analysis_enabled: bool,
    
    /// Automatic deduplication
    pub auto_dedup: bool,
    
    /// Privacy mode (exclude sensitive commands)
    pub privacy_mode: bool,
    
    /// Minimum command length to store
    pub min_command_length: usize,
}

/// Performance metrics for history system
#[derive(Debug, Default)]
pub struct HistoryMetrics {
    /// Search performance
    pub search_times: VecDeque<Duration>,
    
    /// Index update times
    pub index_update_times: VecDeque<Duration>,
    
    /// Memory usage
    pub memory_usage: usize,
    
    /// Cache hit rates
    pub cache_hits: u64,
    pub cache_misses: u64,
    
    /// AI analysis times
    pub ai_analysis_times: VecDeque<Duration>,
}

/// Search query with enhanced options
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Query text
    pub query: String,
    
    /// Search type
    pub search_type: SearchType,
    
    /// Filters
    pub filters: SearchFilters,
    
    /// Sort options
    pub sort: SortOptions,
    
    /// Context for relevance
    pub context: Option<CommandContext>,
}

/// Types of searches
#[derive(Debug, Clone, Copy)]
pub enum SearchType {
    /// Exact text match
    Exact,
    
    /// Fuzzy text match
    Fuzzy,
    
    /// Semantic similarity
    Semantic,
    
    /// Hybrid (text + semantic)
    Hybrid,
    
    /// Context-aware
    Contextual,
}

/// Search filters
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// Date range
    pub date_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    
    /// Directory filter
    pub directory: Option<PathBuf>,
    
    /// Exit code filter
    pub exit_code: Option<i32>,
    
    /// Duration range
    pub duration_range: Option<(Duration, Duration)>,
    
    /// Tags filter
    pub tags: Vec<String>,
    
    /// Project filter
    pub project: Option<String>,
    
    /// Command intent filter
    pub intent: Option<CommandIntent>,
}

/// Sort options
#[derive(Debug, Clone)]
pub struct SortOptions {
    /// Sort field
    pub field: SortField,
    
    /// Sort direction
    pub direction: SortDirection,
    
    /// Secondary sort
    pub secondary: Option<Box<SortOptions>>,
}

/// Fields to sort by
#[derive(Debug, Clone, Copy)]
pub enum SortField {
    Relevance,
    Recency,
    Frequency,
    Duration,
    Directory,
    Success,
}

/// Sort direction
#[derive(Debug, Clone, Copy)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Search results
#[derive(Debug, Clone)]
pub struct SearchResults {
    /// Matched entries
    pub entries: Vec<HistoryEntry>,
    
    /// Relevance scores
    pub scores: Vec<f32>,
    
    /// Total matches (before pagination)
    pub total_matches: usize,
    
    /// Search metadata
    pub metadata: SearchMetadata,
    
    /// Suggestions for better queries
    pub suggestions: Vec<String>,
}

/// Search metadata
#[derive(Debug, Clone)]
pub struct SearchMetadata {
    /// Search duration
    pub duration: Duration,
    
    /// Query analysis
    pub query_analysis: QueryAnalysis,
    
    /// Index statistics used
    pub index_stats: IndexStats,
}

/// Query analysis results
#[derive(Debug, Clone)]
pub struct QueryAnalysis {
    /// Detected intent
    pub intent: Option<CommandIntent>,
    
    /// Extracted entities
    pub entities: Vec<Entity>,
    
    /// Query complexity score
    pub complexity: f32,
    
    /// Confidence in analysis
    pub confidence: f32,
}

/// Extracted entities from query
#[derive(Debug, Clone)]
pub struct Entity {
    pub entity_type: EntityType,
    pub text: String,
    pub confidence: f32,
}

/// Types of entities
#[derive(Debug, Clone, Copy)]
pub enum EntityType {
    Command,
    File,
    Directory,
    Flag,
    Argument,
    Date,
    Time,
    Number,
    Url,
}

/// Index usage statistics
#[derive(Debug, Clone, Default)]
pub struct IndexStats {
    pub text_index_hits: u32,
    pub semantic_index_hits: u32,
    pub context_index_hits: u32,
    pub cache_hits: u32,
}

impl WarpHistoryManager {
    /// Create new history manager
    pub fn new(config: HistoryConfig) -> Self {
        Self {
            history: VecDeque::new(),
            ai_integration: None,
            search_engine: HistorySearchEngine::new(),
            pattern_analyzer: CommandPatternAnalyzer::new(),
            context_matcher: ContextMatcher::new(),
            config,
            metrics: HistoryMetrics::default(),
        }
    }
    
    /// Perform aggressive memory cleanup for long-running sessions
    pub async fn cleanup_memory(&mut self) -> Result<MemoryCleanupReport> {
        let start_time = std::time::Instant::now();
        let initial_memory = self.estimate_memory_usage();
        
        // Clean up search engine caches
        let search_cleanup = self.search_engine.cleanup_caches().await?;
        
        // Clean up pattern analyzer data
        let pattern_cleanup = self.pattern_analyzer.cleanup_old_patterns();
        
        // Clean up context matcher caches
        let context_cleanup = self.context_matcher.cleanup_similarity_cache().await?;
        
        // Prune old entries more aggressively if needed
        let entries_removed = self.aggressive_history_prune();
        
        // Update metrics and clear old performance data
        self.cleanup_metrics();
        
        let final_memory = self.estimate_memory_usage();
        let cleanup_time = start_time.elapsed();
        
        let report = MemoryCleanupReport {
            initial_memory_bytes: initial_memory,
            final_memory_bytes: final_memory,
            memory_freed_bytes: initial_memory.saturating_sub(final_memory),
            entries_removed,
            search_cache_cleared: search_cleanup.entries_cleared,
            pattern_data_cleared: pattern_cleanup,
            similarity_cache_cleared: context_cleanup,
            cleanup_duration: cleanup_time,
        };
        
        tracing::info!("Memory cleanup completed: freed {} bytes in {:?}", 
                      report.memory_freed_bytes, cleanup_time);
        
        Ok(report)
    }
    
    /// Estimate current memory usage
    pub fn estimate_memory_usage(&self) -> usize {
        let mut total = 0;
        
        // History entries
        for entry in &self.history {
            total += std::mem::size_of_val(entry);
            total += entry.command.capacity();
            total += entry.normalized_command.capacity();
            for part in &entry.command_parts {
                total += std::mem::size_of_val(part);
                total += part.text.capacity();
            }
            if let Some(ref embedding) = entry.ai_insights.embedding {
                total += embedding.len() * std::mem::size_of::<f32>();
            }
        }
        
        // Search engine memory
        total += self.search_engine.estimate_memory_usage();
        
        // Pattern analyzer memory
        total += self.pattern_analyzer.estimate_memory_usage();
        
        // Context matcher memory
        total += self.context_matcher.estimate_memory_usage();
        
        // Metrics memory
        total += self.metrics.memory_usage;
        
        total
    }
    
    /// Perform aggressive history pruning based on age, relevance, and frequency
    fn aggressive_history_prune(&mut self) -> usize {
        let initial_len = self.history.len();
        let now = chrono::Utc::now();
        let old_threshold = now - chrono::Duration::hours(24); // Keep only last 24 hours in aggressive mode
        
        // Remove old entries with low usage stats
        self.history.retain(|entry| {
            let recent_enough = entry.context.time_context.timestamp > old_threshold;
            let frequently_used = entry.usage_stats.usage_count > 3;
            let successful = entry.usage_stats.success_rate > 0.5;
            
            recent_enough || (frequently_used && successful)
        });
        
        // Ensure we don't go below minimum entries
        let min_entries = (self.config.max_entries / 4).max(50); // Keep at least 25% or 50 entries
        while self.history.len() > min_entries {
            // Remove entries with lowest combined score (frequency + recency + success)
            if let Some(worst_index) = self.find_worst_entry_index() {
                let entry = self.history.remove(worst_index).unwrap();
                self.search_engine.remove_entry(&entry.id);
            } else {
                break;
            }
        }
        
        initial_len.saturating_sub(self.history.len())
    }
    
    /// Find the index of the entry with the worst relevance score
    fn find_worst_entry_index(&self) -> Option<usize> {
        if self.history.is_empty() {
            return None;
        }
        
        let now = chrono::Utc::now();
        let mut worst_index = 0;
        let mut worst_score = f32::INFINITY;
        
        for (index, entry) in self.history.iter().enumerate() {
            let age_hours = (now - entry.context.time_context.timestamp).num_hours() as f32;
            let age_penalty = (age_hours / 24.0).min(10.0); // Max penalty of 10x
            
            let frequency_score = entry.usage_stats.usage_count as f32;
            let success_score = entry.usage_stats.success_rate * 10.0;
            let recency_score = (1.0 / (age_penalty + 1.0)) * 10.0;
            
            let combined_score = frequency_score + success_score + recency_score;
            
            if combined_score < worst_score {
                worst_score = combined_score;
                worst_index = index;
            }
        }
        
        Some(worst_index)
    }
    
    /// Clean up old metrics data
    fn cleanup_metrics(&mut self) {
        // Keep only recent performance data
        let max_samples = 100;
        
        if self.metrics.search_times.len() > max_samples {
            let excess = self.metrics.search_times.len() - max_samples;
            for _ in 0..excess {
                self.metrics.search_times.pop_front();
            }
        }
        
        if self.metrics.index_update_times.len() > max_samples {
            let excess = self.metrics.index_update_times.len() - max_samples;
            for _ in 0..excess {
                self.metrics.index_update_times.pop_front();
            }
        }
        
        if self.metrics.ai_analysis_times.len() > max_samples {
            let excess = self.metrics.ai_analysis_times.len() - max_samples;
            for _ in 0..excess {
                self.metrics.ai_analysis_times.pop_front();
            }
        }
        
        // Update memory usage estimate
        self.metrics.memory_usage = self.estimate_memory_usage();
    }
    
    /// Add command to history
    pub async fn add_command(&mut self, command: String, context: CommandContext) -> Result<()> {
        let entry = self.create_history_entry(command, context).await?;
        
        // Add to history
        self.history.push_front(entry.clone());
        
        // Maintain size limit
        if self.history.len() > self.config.max_entries {
            if let Some(old_entry) = self.history.pop_back() {
                self.search_engine.remove_entry(&old_entry.id);
            }
        }
        
        // Update indexes
        self.search_engine.add_entry(&entry).await?;
        
        // Update patterns
        self.pattern_analyzer.analyze_command(&entry);
        
        // Persist if enabled
        if self.config.persist_history {
            self.persist_entry(&entry).await?;
        }
        
        Ok(())
    }
    
    /// Search command history
    pub async fn search(&self, query: SearchQuery) -> Result<SearchResults> {
        let start_time = std::time::Instant::now();
        
        // Analyze query
        let query_analysis = self.analyze_query(&query).await?;
        
        // Perform search based on type
        let mut results = match query.search_type {
            SearchType::Exact => self.search_engine.exact_search(&query).await?,
            SearchType::Fuzzy => self.search_engine.fuzzy_search(&query).await?,
            SearchType::Semantic => self.search_engine.semantic_search(&query).await?,
            SearchType::Hybrid => self.search_engine.hybrid_search(&query).await?,
            SearchType::Contextual => self.context_matcher.contextual_search(&query).await?,
        };
        
        // Apply filters
        results = self.apply_filters(results, &query.filters);
        
        // Sort results
        results = self.sort_results(results, &query.sort);
        
        // Generate suggestions
        let suggestions = self.generate_search_suggestions(&query, &results).await?;
        
        let search_duration = start_time.elapsed();
        
        Ok(SearchResults {
            entries: results.entries,
            scores: results.scores,
            total_matches: results.total_matches,
            metadata: SearchMetadata {
                duration: search_duration,
                query_analysis,
                index_stats: results.metadata.index_stats,
            },
            suggestions,
        })
    }
    
    /// Get contextual suggestions based on current state
    pub async fn get_suggestions(&self, context: &CommandContext, limit: usize) -> Result<Vec<HistoryEntry>> {
        self.context_matcher.get_contextual_suggestions(context, limit).await
    }
    
    /// Get command completion suggestions
    pub async fn get_completions(&self, partial_command: &str, context: &CommandContext) -> Result<Vec<String>> {
        // Find similar commands in history
        let query = SearchQuery {
            query: partial_command.to_string(),
            search_type: SearchType::Fuzzy,
            filters: SearchFilters {
                directory: Some(context.working_directory.clone()),
                ..Default::default()
            },
            sort: SortOptions {
                field: SortField::Frequency,
                direction: SortDirection::Descending,
                secondary: None,
            },
            context: Some(context.clone()),
        };
        
        let results = self.search(query).await?;
        
        // Extract completions
        let completions: Vec<String> = results.entries
            .into_iter()
            .filter_map(|entry| {
                if entry.command.starts_with(partial_command) {
                    Some(entry.command)
                } else {
                    None
                }
            })
            .take(10)
            .collect();
        
        Ok(completions)
    }
    
    /// Record command error for learning purposes
    pub async fn record_error(&mut self, command: &str, exit_code: i32, output: &str) {
        let error_context = CommandContext {
            working_directory: std::env::current_dir().unwrap_or_default(),
            environment: std::env::vars().collect(),
            shell: std::env::var("SHELL").unwrap_or_else(|_| "unknown".to_string()),
            terminal_size: None,
            git_context: None,
            project_context: None,
            time_context: TimeContext {
                timestamp: Utc::now(),
                day_of_week: chrono::Weekday::Mon, // This would be calculated
                hour_of_day: 12, // This would be calculated
            },
        };
        
        let execution_metadata = ExecutionMetadata {
            exit_code: Some(exit_code),
            duration: None,
            output_summary: Some(output.to_string()),
            error_info: if exit_code != 0 { Some(output.to_string()) } else { None },
            resource_usage: None,
        };
        
        // Add the failed command to history for learning
        if let Ok(mut entry) = self.create_history_entry(command.to_string(), error_context).await {
            entry.execution = execution_metadata;
            self.history.push_front(entry.clone());
            
            // Update search index
            let _ = self.search_engine.add_entry(&entry).await;
        }
    }
    
    /// Update command with execution results
    pub async fn update_command_result(&mut self, command_id: &str, metadata: ExecutionMetadata) -> Result<()> {
        if let Some(entry) = self.history.iter_mut().find(|e| e.id == command_id) {
            entry.execution = metadata;
            entry.usage_stats.usage_count += 1;
            entry.usage_stats.last_used = Utc::now();
            
            // Update success rate
            if let Some(exit_code) = entry.execution.exit_code {
                let success = exit_code == 0;
                let old_rate = entry.usage_stats.success_rate;
                let count = entry.usage_stats.usage_count as f32;
                entry.usage_stats.success_rate = (old_rate * (count - 1.0) + if success { 1.0 } else { 0.0 }) / count;
            }
            
            // Update average execution time
            if let Some(duration) = entry.execution.duration {
                if let Some(avg) = entry.usage_stats.avg_execution_time {
                    let count = entry.usage_stats.usage_count as f32;
                    let new_avg = Duration::from_nanos(
                        ((avg.as_nanos() as f32 * (count - 1.0)) + duration.as_nanos() as f32) / count
                    ) as u64);
                    entry.usage_stats.avg_execution_time = Some(Duration::from_nanos(new_avg));
                } else {
                    entry.usage_stats.avg_execution_time = Some(duration);
                }
            }
            
            // Update search index
            self.search_engine.update_entry(entry).await?;
        }
        
        Ok(())
    }
    
    /// Get usage patterns and insights
    pub fn get_insights(&self) -> HistoryInsights {
        HistoryInsights {
            total_commands: self.history.len(),
            unique_commands: self.get_unique_command_count(),
            most_used_commands: self.get_most_used_commands(10),
            success_rate: self.get_overall_success_rate(),
            avg_execution_time: self.get_average_execution_time(),
            command_patterns: self.pattern_analyzer.get_patterns(),
            temporal_insights: self.get_temporal_insights(),
            context_insights: self.get_context_insights(),
        }
    }
    
    // Private helper methods
    
    async fn create_history_entry(&self, command: String, context: CommandContext) -> Result<HistoryEntry> {
        let id = self.generate_entry_id(&command, &context);
        let normalized_command = self.normalize_command(&command);
        let command_parts = self.parse_command(&command).await?;
        
        let mut entry = HistoryEntry {
            id,
            command: command.clone(),
            normalized_command,
            command_parts,
            execution: ExecutionMetadata {
                exit_code: None,
                duration: None,
                output_summary: None,
                error_info: None,
                resource_usage: None,
            },
            context,
            ai_insights: HistoryAiInsights::default(),
            usage_stats: CommandUsageStats {
                usage_count: 1,
                last_used: Utc::now(),
                ..Default::default()
            },
            tags: Vec::new(),
            annotations: Vec::new(),
        };
        
        // AI analysis if enabled
        if self.config.ai_analysis_enabled {
            if let Some(ref ai) = self.ai_integration {
                let insights = self.analyze_with_ai(&command, ai).await?;
                entry.ai_insights = insights;
            }
        }
        
        Ok(entry)
    }
    
    fn generate_entry_id(&self, command: &str, context: &CommandContext) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        command.hash(&mut hasher);
        context.working_directory.hash(&mut hasher);
        context.time_context.timestamp.timestamp().hash(&mut hasher);
        
        format!("hist_{:x}", hasher.finish())
    }
    
    fn normalize_command(&self, command: &str) -> String {
        // Basic normalization: lowercase, trim whitespace, collapse spaces
        command.to_lowercase().trim().split_whitespace().collect::<Vec<_>>().join(" ")
    }
    
    async fn parse_command(&self, command: &str) -> Result<Vec<CommandPart>> {
        // Basic command parsing - in a real implementation, this would be more sophisticated
        let parts: Vec<CommandPart> = command
            .split_whitespace()
            .enumerate()
            .map(|(i, part)| {
                let part_type = if i == 0 {
                    CommandPartType::Command
                } else if part.starts_with('-') {
                    if part.starts_with("--") {
                        CommandPartType::LongFlag
                    } else {
                        CommandPartType::Flag
                    }
                } else {
                    CommandPartType::Argument
                };
                
                CommandPart {
                    text: part.to_string(),
                    part_type,
                    semantic_type: self.infer_semantic_type(part, &part_type),
                    confidence: 0.8, // Placeholder
                }
            })
            .collect();
        
        Ok(parts)
    }
    
    fn infer_semantic_type(&self, part: &str, part_type: &CommandPartType) -> Option<SemanticType> {
        match part_type {
            CommandPartType::Command => {
                match part {
                    "git" => Some(SemanticType::GitOperation),
                    "docker" => Some(SemanticType::Docker),
                    "ls" | "find" | "grep" => Some(SemanticType::FileManagement),
                    "curl" | "wget" => Some(SemanticType::NetworkRequest),
                    "ps" | "top" | "kill" => Some(SemanticType::ProcessManagement),
                    "sudo" => Some(SemanticType::SystemAdmin),
                    _ => None,
                }
            }
            _ => None,
        }
    }
    
    async fn analyze_with_ai(&self, command: &str, ai: &WarpAiIntegration) -> Result<HistoryAiInsights> {
        // Placeholder for AI analysis
        Ok(HistoryAiInsights::default())
    }
    
    async fn analyze_query(&self, query: &SearchQuery) -> Result<QueryAnalysis> {
        // Basic query analysis - would be enhanced with AI
        Ok(QueryAnalysis {
            intent: None,
            entities: Vec::new(),
            complexity: 0.5,
            confidence: 0.7,
        })
    }
    
    fn apply_filters(&self, results: SearchResults, filters: &SearchFilters) -> SearchResults {
        // Apply filters to search results
        results // Placeholder - would filter based on criteria
    }
    
    fn sort_results(&self, results: SearchResults, sort: &SortOptions) -> SearchResults {
        // Sort results based on criteria
        results // Placeholder - would sort based on field and direction
    }
    
    async fn generate_search_suggestions(&self, query: &SearchQuery, results: &SearchResults) -> Result<Vec<String>> {
        // Generate suggestions for improving search
        Ok(Vec::new()) // Placeholder
    }
    
    async fn persist_entry(&self, entry: &HistoryEntry) -> Result<()> {
        // Persist entry to storage
        Ok(()) // Placeholder
    }
    
    fn get_unique_command_count(&self) -> usize {
        let mut commands = std::collections::HashSet::new();
        for entry in &self.history {
            commands.insert(&entry.normalized_command);
        }
        commands.len()
    }
    
    fn get_most_used_commands(&self, limit: usize) -> Vec<(String, u64)> {
        let mut command_counts = HashMap::new();
        for entry in &self.history {
            *command_counts.entry(entry.command.clone()).or_insert(0u64) += entry.usage_stats.usage_count;
        }
        
        let mut sorted: Vec<_> = command_counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(limit);
        sorted
    }
    
    fn get_overall_success_rate(&self) -> f32 {
        if self.history.is_empty() {
            return 0.0;
        }
        
        let total_success: f32 = self.history.iter()
            .map(|entry| entry.usage_stats.success_rate * entry.usage_stats.usage_count as f32)
            .sum();
        let total_count: u64 = self.history.iter()
            .map(|entry| entry.usage_stats.usage_count)
            .sum();
        
        if total_count > 0 {
            total_success / total_count as f32
        } else {
            0.0
        }
    }
    
    fn get_average_execution_time(&self) -> Option<Duration> {
        let times: Vec<Duration> = self.history.iter()
            .filter_map(|entry| entry.usage_stats.avg_execution_time)
            .collect();
        
        if times.is_empty() {
            return None;
        }
        
        let total_nanos: u128 = times.iter().map(|d| d.as_nanos()).sum();
        Some(Duration::from_nanos((total_nanos / times.len() as u128) as u64))
    }
    
    fn get_temporal_insights(&self) -> TemporalInsights {
        // Analyze temporal patterns
        TemporalInsights::default() // Placeholder
    }
    
    fn get_context_insights(&self) -> ContextInsights {
        // Analyze context patterns
        ContextInsights::default() // Placeholder
    }
}

/// History insights and analytics
#[derive(Debug, Clone)]
pub struct HistoryInsights {
    pub total_commands: usize,
    pub unique_commands: usize,
    pub most_used_commands: Vec<(String, u64)>,
    pub success_rate: f32,
    pub avg_execution_time: Option<Duration>,
    pub command_patterns: Vec<CommandSequence>,
    pub temporal_insights: TemporalInsights,
    pub context_insights: ContextInsights,
}

/// Temporal analysis insights
#[derive(Debug, Clone, Default)]
pub struct TemporalInsights {
    pub peak_hours: Vec<u8>,
    pub peak_days: Vec<u8>,
    pub seasonal_trends: Vec<f32>,
    pub productivity_patterns: HashMap<String, f32>,
}

/// Context analysis insights
#[derive(Debug, Clone, Default)]
pub struct ContextInsights {
    pub directory_usage: HashMap<PathBuf, u32>,
    pub project_preferences: HashMap<String, f32>,
    pub environment_dependencies: Vec<String>,
    pub error_hotspots: Vec<String>,
}

// Implementation details for supporting structures...

impl HistorySearchEngine {
    fn new() -> Self {
        Self {
            text_index: TextIndex::new(),
            semantic_index: SemanticIndex::new(),
            context_index: ContextIndex::new(),
            search_config: SearchConfig::default(),
        }
    }
    
    /// Cleanup caches and free memory
    async fn cleanup_caches(&mut self) -> Result<SearchCacheCleanupReport> {
        let initial_memory = self.estimate_memory_usage();
        
        // Clean up semantic similarity cache
        let similarity_entries_cleared = self.semantic_index.cleanup_similarity_cache();
        
        // Clean up text index n-gram cache if it's too large
        let ngram_entries_cleared = self.text_index.cleanup_ngram_cache();
        
        // Clean up context index old time buckets
        let time_buckets_cleared = self.context_index.cleanup_old_time_buckets();
        
        let final_memory = self.estimate_memory_usage();
        
        Ok(SearchCacheCleanupReport {
            entries_cleared: similarity_entries_cleared + ngram_entries_cleared + time_buckets_cleared,
            memory_freed: initial_memory.saturating_sub(final_memory),
        })
    }
    
    /// Estimate memory usage of search engine
    fn estimate_memory_usage(&self) -> usize {
        self.text_index.estimate_memory_usage() +
        self.semantic_index.estimate_memory_usage() +
        self.context_index.estimate_memory_usage()
    }
    
    async fn add_entry(&mut self, entry: &HistoryEntry) -> Result<()> {
        self.text_index.add_entry(entry);
        self.semantic_index.add_entry(entry).await?;
        self.context_index.add_entry(entry);
        Ok(())
    }
    
    fn remove_entry(&mut self, entry_id: &str) {
        self.text_index.remove_entry(entry_id);
        self.semantic_index.remove_entry(entry_id);
        self.context_index.remove_entry(entry_id);
    }
    
    async fn update_entry(&mut self, entry: &HistoryEntry) -> Result<()> {
        self.remove_entry(&entry.id);
        self.add_entry(entry).await
    }
    
    async fn exact_search(&self, query: &SearchQuery) -> Result<SearchResults> {
        // Placeholder implementation
        Ok(SearchResults {
            entries: Vec::new(),
            scores: Vec::new(),
            total_matches: 0,
            metadata: SearchMetadata {
                duration: Duration::from_millis(1),
                query_analysis: QueryAnalysis {
                    intent: None,
                    entities: Vec::new(),
                    complexity: 0.0,
                    confidence: 0.0,
                },
                index_stats: IndexStats::default(),
            },
            suggestions: Vec::new(),
        })
    }
    
    async fn fuzzy_search(&self, query: &SearchQuery) -> Result<SearchResults> {
        self.exact_search(query).await // Placeholder
    }
    
    async fn semantic_search(&self, query: &SearchQuery) -> Result<SearchResults> {
        self.exact_search(query).await // Placeholder
    }
    
    async fn hybrid_search(&self, query: &SearchQuery) -> Result<SearchResults> {
        self.exact_search(query).await // Placeholder
    }
}

impl TextIndex {
    fn new() -> Self {
        Self {
            forward_index: HashMap::new(),
            inverted_index: HashMap::new(),
            ngram_index: HashMap::new(),
            token_frequencies: HashMap::new(),
        }
    }
    
    fn add_entry(&mut self, entry: &HistoryEntry) {
        // Add entry to text index
        let tokens = self.tokenize(&entry.command);
        self.forward_index.insert(entry.id.clone(), tokens.clone());
        
        for token in tokens {
            self.inverted_index.entry(token.clone()).or_insert_with(Vec::new).push(entry.id.clone());
            *self.token_frequencies.entry(token).or_insert(0.0) += 1.0;
        }
    }
    
    fn remove_entry(&mut self, entry_id: &str) {
        if let Some(tokens) = self.forward_index.remove(entry_id) {
            for token in tokens {
                if let Some(entries) = self.inverted_index.get_mut(&token) {
                    entries.retain(|id| id != entry_id);
                    if entries.is_empty() {
                        self.inverted_index.remove(&token);
                        self.token_frequencies.remove(&token);
                    }
                }
            }
        }
    }
    
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|s| s.to_lowercase())
            .collect()
    }
    
    /// Clean up n-gram cache to free memory
    fn cleanup_ngram_cache(&mut self) -> usize {
        let initial_size = self.ngram_index.len();
        
        // Remove n-grams with very low frequency (less than 2 occurrences)
        self.ngram_index.retain(|_, entries| entries.len() >= 2);
        
        initial_size.saturating_sub(self.ngram_index.len())
    }
    
    /// Estimate memory usage of text index
    fn estimate_memory_usage(&self) -> usize {
        let mut total = 0;
        
        // Forward index
        for (key, tokens) in &self.forward_index {
            total += key.capacity();
            total += tokens.capacity() * std::mem::size_of::<String>();
            for token in tokens {
                total += token.capacity();
            }
        }
        
        // Inverted index
        for (key, entries) in &self.inverted_index {
            total += key.capacity();
            total += entries.capacity() * std::mem::size_of::<String>();
            for entry in entries {
                total += entry.capacity();
            }
        }
        
        // N-gram index
        for (key, entries) in &self.ngram_index {
            total += key.capacity();
            total += entries.capacity() * std::mem::size_of::<String>();
        }
        
        // Token frequencies
        for (key, _) in &self.token_frequencies {
            total += key.capacity() + std::mem::size_of::<f32>();
        }
        
        total
    }
}

impl SemanticIndex {
    fn new() -> Self {
        Self {
            embeddings: HashMap::new(),
            similarity_cache: HashMap::new(),
            clusters: HashMap::new(),
        }
    }
    
    async fn add_entry(&mut self, entry: &HistoryEntry) -> Result<()> {
        // Generate embedding for the command
        if let Some(embedding) = &entry.ai_insights.embedding {
            self.embeddings.insert(entry.id.clone(), embedding.clone());
        }
        Ok(())
    }
    
    fn remove_entry(&mut self, entry_id: &str) {
        self.embeddings.remove(entry_id);
        // Also clean up similarity cache entries involving this ID
        self.similarity_cache.retain(|(a, b), _| a != entry_id && b != entry_id);
    }
    
    /// Clean up similarity cache to free memory
    fn cleanup_similarity_cache(&mut self) -> usize {
        let initial_size = self.similarity_cache.len();
        
        // Keep only cache entries for entries that still exist
        let valid_entry_ids: std::collections::HashSet<String> = self.embeddings.keys().cloned().collect();
        
        self.similarity_cache.retain(|(a, b), _| {
            valid_entry_ids.contains(a) && valid_entry_ids.contains(b)
        });
        
        // Also limit cache size to prevent unbounded growth
        const MAX_CACHE_SIZE: usize = 10000;
        if self.similarity_cache.len() > MAX_CACHE_SIZE {
            // Remove oldest entries (this is a simplified approach)
            let to_remove = self.similarity_cache.len() - MAX_CACHE_SIZE;
            let mut keys_to_remove: Vec<_> = self.similarity_cache.keys().take(to_remove).cloned().collect();
            for key in keys_to_remove {
                self.similarity_cache.remove(&key);
            }
        }
        
        initial_size.saturating_sub(self.similarity_cache.len())
    }
    
    /// Estimate memory usage of semantic index
    fn estimate_memory_usage(&self) -> usize {
        let mut total = 0;
        
        // Embeddings
        for (key, embedding) in &self.embeddings {
            total += key.capacity();
            total += embedding.capacity() * std::mem::size_of::<f32>();
        }
        
        // Similarity cache
        for ((a, b), _) in &self.similarity_cache {
            total += a.capacity() + b.capacity() + std::mem::size_of::<f32>();
        }
        
        // Clusters
        for (key, cluster) in &self.clusters {
            total += key.capacity();
            total += cluster.capacity() * std::mem::size_of::<String>();
            for entry in cluster {
                total += entry.capacity();
            }
        }
        
        total
    }
}

impl ContextIndex {
    fn new() -> Self {
        Self {
            directory_index: HashMap::new(),
            time_index: HashMap::new(),
            project_index: HashMap::new(),
            git_index: HashMap::new(),
        }
    }
    
    fn add_entry(&mut self, entry: &HistoryEntry) {
        // Add to directory index
        self.directory_index
            .entry(entry.context.working_directory.clone())
            .or_insert_with(Vec::new)
            .push(entry.id.clone());
        
        // Add to time index
        let time_bucket = self.get_time_bucket(&entry.context.time_context.timestamp);
        self.time_index
            .entry(time_bucket)
            .or_insert_with(Vec::new)
            .push(entry.id.clone());
        
        // Add to project index if available
        if let Some(ref project_context) = entry.context.project_context {
            if let Some(ref project_name) = project_context.project_name {
                self.project_index
                    .entry(project_name.clone())
                    .or_insert_with(Vec::new)
                    .push(entry.id.clone());
            }
        }
        
        // Add to git index if available
        if let Some(ref git_context) = entry.context.git_context {
            if let Some(ref branch) = git_context.branch {
                self.git_index
                    .entry(branch.clone())
                    .or_insert_with(Vec::new)
                    .push(entry.id.clone());
            }
        }
    }
    
    fn remove_entry(&mut self, entry_id: &str) {
        // Remove from all indexes
        for entries in self.directory_index.values_mut() {
            entries.retain(|id| id != entry_id);
        }
        for entries in self.time_index.values_mut() {
            entries.retain(|id| id != entry_id);
        }
        for entries in self.project_index.values_mut() {
            entries.retain(|id| id != entry_id);
        }
        for entries in self.git_index.values_mut() {
            entries.retain(|id| id != entry_id);
        }
    }
    
    fn get_time_bucket(&self, timestamp: &DateTime<Utc>) -> String {
        // Create time bucket (e.g., "2023-12-25-14" for hour-based buckets)
        format!("{}-{:02}", timestamp.format("%Y-%m-%d"), timestamp.hour())
    }
    
    /// Clean up old time buckets to free memory
    fn cleanup_old_time_buckets(&mut self) -> usize {
        let now = chrono::Utc::now();
        let cutoff = now - chrono::Duration::days(7); // Keep only last 7 days
        let cutoff_bucket = self.get_time_bucket(&cutoff);
        
        let initial_size = self.time_index.len();
        
        // Remove time buckets older than cutoff
        self.time_index.retain(|time_bucket, _| time_bucket >= &cutoff_bucket);
        
        // Clean up empty entries in other indexes
        self.directory_index.retain(|_, entries| !entries.is_empty());
        self.project_index.retain(|_, entries| !entries.is_empty());
        self.git_index.retain(|_, entries| !entries.is_empty());
        
        initial_size.saturating_sub(self.time_index.len())
    }
    
    /// Estimate memory usage of context index
    fn estimate_memory_usage(&self) -> usize {
        let mut total = 0;
        
        // Directory index
        for (key, entries) in &self.directory_index {
            total += key.as_os_str().len();
            total += entries.capacity() * std::mem::size_of::<String>();
            for entry in entries {
                total += entry.capacity();
            }
        }
        
        // Time index
        for (key, entries) in &self.time_index {
            total += key.capacity();
            total += entries.capacity() * std::mem::size_of::<String>();
            for entry in entries {
                total += entry.capacity();
            }
        }
        
        // Project index
        for (key, entries) in &self.project_index {
            total += key.capacity();
            total += entries.capacity() * std::mem::size_of::<String>();
            for entry in entries {
                total += entry.capacity();
            }
        }
        
        // Git index
        for (key, entries) in &self.git_index {
            total += key.capacity();
            total += entries.capacity() * std::mem::size_of::<String>();
            for entry in entries {
                total += entry.capacity();
            }
        }
        
        total
    }
}

impl CommandPatternAnalyzer {
    fn new() -> Self {
        Self {
            command_sequences: HashMap::new(),
            temporal_patterns: HashMap::new(),
            context_patterns: HashMap::new(),
            error_patterns: HashMap::new(),
        }
    }
    
    fn analyze_command(&mut self, entry: &HistoryEntry) {
        // Analyze command patterns - placeholder implementation
        self.update_temporal_patterns(entry);
        self.update_context_patterns(entry);
        
        if let Some(exit_code) = entry.execution.exit_code {
            if exit_code != 0 {
                self.update_error_patterns(entry);
            }
        }
    }
    
    fn update_temporal_patterns(&mut self, entry: &HistoryEntry) {
        // Update temporal usage patterns
    }
    
    fn update_context_patterns(&mut self, entry: &HistoryEntry) {
        // Update context-based patterns
    }
    
    fn update_error_patterns(&mut self, entry: &HistoryEntry) {
        // Update error recovery patterns
    }
    
    fn get_patterns(&self) -> Vec<CommandSequence> {
        self.command_sequences.values().cloned().collect()
    }
    
    /// Clean up old pattern data
    fn cleanup_old_patterns(&mut self) -> usize {
        let now = chrono::Utc::now();
        let cutoff = now - chrono::Duration::days(30); // Keep patterns from last 30 days
        
        let mut removed_count = 0;
        
        // Clean up temporal patterns
        let initial_temporal_size = self.temporal_patterns.len();
        self.temporal_patterns.retain(|_, pattern| {
            // Keep pattern if it's still relevant (this is a simplified check)
            pattern.time_distribution.values().any(|&freq| freq > 0.1)
        });
        removed_count += initial_temporal_size.saturating_sub(self.temporal_patterns.len());
        
        // Clean up command sequences with low frequency
        let initial_seq_size = self.command_sequences.len();
        self.command_sequences.retain(|_, seq| seq.frequency >= 2);
        removed_count += initial_seq_size.saturating_sub(self.command_sequences.len());
        
        // Clean up error patterns that are too old or infrequent
        let initial_error_size = self.error_patterns.len();
        self.error_patterns.retain(|_, pattern| {
            pattern.success_rate > 0.1 // Keep patterns that have some success
        });
        removed_count += initial_error_size.saturating_sub(self.error_patterns.len());
        
        removed_count
    }
    
    /// Estimate memory usage
    fn estimate_memory_usage(&self) -> usize {
        let mut total = 0;
        
        // Command sequences
        for (key, seq) in &self.command_sequences {
            total += key.capacity();
            total += std::mem::size_of_val(seq);
            for cmd in &seq.commands {
                total += cmd.capacity();
            }
        }
        
        // Temporal patterns
        for (key, pattern) in &self.temporal_patterns {
            total += key.capacity();
            total += std::mem::size_of_val(pattern);
            total += pattern.time_distribution.capacity() * (std::mem::size_of::<u8>() + std::mem::size_of::<f32>());
            total += pattern.day_distribution.capacity() * (std::mem::size_of::<u8>() + std::mem::size_of::<f32>());
            total += pattern.seasonal_trends.capacity() * std::mem::size_of::<f32>();
        }
        
        // Context patterns
        for (key, pattern) in &self.context_patterns {
            total += key.capacity();
            total += std::mem::size_of_val(pattern);
            for env_dep in &pattern.environment_dependencies {
                total += env_dep.capacity();
            }
        }
        
        // Error patterns
        for (key, pattern) in &self.error_patterns {
            total += key.capacity();
            total += std::mem::size_of_val(pattern);
            for recovery_cmd in &pattern.recovery_commands {
                total += recovery_cmd.capacity();
            }
        }
        
        total
    }
}

impl ContextMatcher {
    fn new() -> Self {
        Self {
            context_analyzer: ContextAnalyzer::new(),
            similarity_calculator: ContextSimilarityCalculator::new(),
            suggestion_ranker: SuggestionRanker::new(),
        }
    }
    
    async fn contextual_search(&self, query: &SearchQuery) -> Result<SearchResults> {
        // Placeholder implementation
        Ok(SearchResults {
            entries: Vec::new(),
            scores: Vec::new(),
            total_matches: 0,
            metadata: SearchMetadata {
                duration: Duration::from_millis(1),
                query_analysis: QueryAnalysis {
                    intent: None,
                    entities: Vec::new(),
                    complexity: 0.0,
                    confidence: 0.0,
                },
                index_stats: IndexStats::default(),
            },
            suggestions: Vec::new(),
        })
    }
    
    async fn get_contextual_suggestions(&self, context: &CommandContext, limit: usize) -> Result<Vec<HistoryEntry>> {
        // Get suggestions based on current context
        Ok(Vec::new()) // Placeholder
    }
    
    /// Clean up similarity cache
    async fn cleanup_similarity_cache(&mut self) -> Result<usize> {
        // This would clean up cached similarity calculations in the context analyzer
        // For now, return 0 as this is a placeholder implementation
        Ok(0)
    }
    
    /// Estimate memory usage
    fn estimate_memory_usage(&self) -> usize {
        // This would estimate the memory used by context analysis caches
        // For now, return a reasonable estimate
        std::mem::size_of::<ContextMatcher>() + 1024 // Base size + estimated cache
    }
}

impl ContextSimilarityCalculator {
    fn new() -> Self {
        Self {
            weights: ContextWeights {
                directory: 0.3,
                time: 0.1,
                project: 0.25,
                git: 0.15,
                environment: 0.1,
                recent_commands: 0.1,
            },
        }
    }
}

impl SuggestionRanker {
    fn new() -> Self {
        Self {
            algorithm: RankingAlgorithm::Hybrid,
            learning_model: None,
        }
    }
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_entries: 10000,
            persist_history: true,
            history_file: Some(dirs::data_dir()
                .unwrap_or_else(|| std::env::current_dir().unwrap())
                .join("openagent-terminal")
                .join("history.db")),
            ai_analysis_enabled: true,
            auto_dedup: true,
            privacy_mode: false,
            min_command_length: 3,
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_results: 50,
            fuzzy_threshold: 0.7,
            semantic_threshold: 0.8,
            context_weight: 0.3,
            recency_bias: 0.2,
            frequency_bias: 0.3,
        }
    }
}

/// Memory cleanup report
#[derive(Debug, Clone)]
pub struct MemoryCleanupReport {
    pub initial_memory_bytes: usize,
    pub final_memory_bytes: usize,
    pub memory_freed_bytes: usize,
    pub entries_removed: usize,
    pub search_cache_cleared: usize,
    pub pattern_data_cleared: usize,
    pub similarity_cache_cleared: usize,
    pub cleanup_duration: Duration,
}

/// Search cache cleanup report
#[derive(Debug, Clone)]
pub struct SearchCacheCleanupReport {
    pub entries_cleared: usize,
    pub memory_freed: usize,
}

/// Type alias for compatibility with IDE manager
pub type WarpHistory = WarpHistoryManager;

/// Create a new WarpHistory with default config
impl WarpHistory {
    pub fn new() -> Self {
        Self::with_config(HistoryConfig::default())
    }
    
    pub fn with_config(config: HistoryConfig) -> Self {
        WarpHistoryManager::new(config)
    }
}
