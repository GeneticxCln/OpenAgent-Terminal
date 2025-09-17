# Native Search: Supported Filters and Semantics

This document enumerates the filters currently supported by the built-in native search engine and their semantics. It also lists planned items and explicitly calls out unsupported cases.

Supported filters
- TextFilter { pattern: String, case_sensitive: bool }
  - Includes results whose content contains pattern.
  - Matching is case-insensitive when case_sensitive=false, and case-sensitive when true.
- RegexFilter { regex: Regex }
  - Includes results whose content matches regex (Rust regex syntax).
- DateFilter { from: Option<Instant>, to: Option<Instant> }
  - Includes results with timestamp within [from, to]. Missing bound is open (e.g., from=None means "-∞").
  - Boundaries are inclusive.
- CreatedAfter { at: Instant }
  - Includes results whose created_at >= at. Results without created_at do not match.
- CreatedBefore { at: Instant }
  - Includes results whose created_at <= at. Results without created_at do not match.
- TypeFilter { types: HashSet<String> }
  - Matches results with metadata["type"]. For file entries this is one of: "Regular", "Directory", "Symlink", "Executable", "Hidden".
  - A result without a "type" field will not match.
- SizeFilter { min_size: Option<usize>, max_size: Option<usize> }
  - Evaluates metadata["size"]. Accepts numeric bytes (e.g., "2048") or human units with base 1024 (e.g., "2KB", "3MB", "1G").
  - Bounds are inclusive. If size metadata is missing, the result only matches when both min_size and max_size are None.
- ScoreFilter { min_score: f64 }
  - Includes only results with relevance_score >= min_score.
- ContextFilter { contexts: HashSet<SearchContext> }
  - Restricts to certain contexts: Global, CommandHistory, AllBlocks, FileSystem, Terminal, Tabs, Splits, GitRepository.
- HasTag { tag: String }
  - Case-insensitive match against a comma-separated metadata["tags"]. Whitespace is ignored around commas.
  - Results without tags do not match this filter.
- ExitCode { codes: HashSet<i32> }
  - CommandHistory results only. Matches when metadata["exit_code"] parses into any of the provided codes.
  - Results without exit_code do not match.
- StatusFilter { success: bool }
  - CommandHistory results only. success=true matches exit_code==0; success=false matches exit_code!=0.
  - Results without exit_code do not match.
- Shell { kinds: HashSet<String> }
  - CommandHistory results only. Case-insensitive match against metadata["shell_kind"] (e.g., bash, zsh, fish).
  - Results without shell_kind do not match.
- Custom { name: String, predicate: Arc<dyn Fn(&SearchResult) -> bool + Send + Sync> }
  - User-provided predicate that decides per-result inclusion.

Notes
- For file and block results, tags (if present) are exposed as a comma-separated string under metadata["tags"]. Tag indexing is shallow; richer indexing is planned.
- For command results, metadata includes: exit_code, duration, working_dir, optional shell_kind, and optional tags.
- Missing required metadata yields no match. For example, CreatedAfter/Before require created_at; Shell requires shell_kind.

Planned/unsupported items
- Richer tag indexing for files/blocks (current implementation uses simple string matching in metadata["tags"]).
- Status and ExitCode for block or aggregated execution contexts (currently limited to CommandHistory entries).

Examples (pseudo)
- Text + tag: TextFilter("build", false) AND HasTag("ci")
- Only successful commands: StatusFilter { success: true }
- Files >= 1MB: SizeFilter { min_size: Some(1_048_576), max_size: None }
- Date range (last hour): DateFilter { from: Some(now - 3600s), to: Some(now) }
- Created after a point: CreatedAfter { at: some_instant }
- File types only: TypeFilter { types: {"Regular", "Directory"} }
- Command exit codes 0 or 2: ExitCode { codes: {0, 2} }
- Shell is zsh: Shell { kinds: {"zsh"} }
- Restrict to files and commands: ContextFilter { contexts: { FileSystem, CommandHistory } }
- Minimum relevance score: ScoreFilter { min_score: 0.5 }
