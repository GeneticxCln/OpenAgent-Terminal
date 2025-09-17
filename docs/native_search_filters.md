# Native Search: Supported Filters and Semantics

This document enumerates the filters currently supported by the built-in native search engine and their semantics. It also lists planned filters for future work.

Supported filters
- TextFilter { pattern: String, case_sensitive: bool }
  - Includes results whose content contains pattern (case-insensitive when case_sensitive=false).
- RegexFilter { regex: Regex }
  - Includes results whose content matches regex.
- DateFilter { from: Option<Instant>, to: Option<Instant> }
  - Includes results with timestamp within [from, to]. Missing bound is open.
- CreatedAfter { at: Instant }
  - Includes results whose created_at >= at. Results without created_at do not match.
- CreatedBefore { at: Instant }
  - Includes results whose created_at <= at. Results without created_at do not match.
- TypeFilter { types: HashSet<String> }
  - Matches results with metadata["type"], useful for file entries (e.g., "Regular", "Directory").
- SizeFilter { min_size: Option<usize>, max_size: Option<usize> }
  - Evaluates metadata["size"]. Accepts numeric bytes or human units (e.g., "2KB").
- ScoreFilter { min_score: f64 }
  - Includes only results with relevance_score >= min_score.
- ContextFilter { contexts: HashSet<SearchContext> }
  - Restricts to certain contexts: Global, CommandHistory, AllBlocks, FileSystem, etc.
- HasTag { tag: String }
  - Case-insensitive match against a comma-separated metadata["tags"].
- ExitCode { codes: HashSet<i32> }
  - For command results; matches when metadata["exit_code"] parses into any of the provided codes.
- StatusFilter { success: bool }
  - For command results; success=true matches exit_code==0; success=false matches exit_code!=0.
- Shell { kinds: HashSet<String> }
  - For command results; case-insensitive match against metadata["shell_kind"] (e.g., bash, zsh, fish).
- Custom { name: String, predicate: Arc<dyn Fn(&SearchResult) -> bool + Send + Sync> }
  - User-provided predicate.

Notes
- For file and block results, tags (if present) are exposed as a comma-separated string under metadata["tags"].
- For command results, metadata includes: exit_code, duration, working_dir, optional shell_kind, and optional tags.
- Unsupported metadata yields no match: if a filter requires a field (e.g., created_at or shell_kind) and it is absent, the result will not match that filter.

Planned filters (not yet implemented)
- HasTag for files/blocks with richer tag indexing.
- Status and ExitCode for block/aggregated execution contexts.

Examples (pseudo)
- Text + tag: TextFilter("build", false) AND HasTag("ci")
- Only successful commands: StatusFilter { success: true }
- Files >= 1MB: SizeFilter { min_size: Some(1_048_576), max_size: None }
