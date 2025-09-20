//! Code quality validation agent with heuristic checks and security analysis.
//! Provides lightweight quality validation leveraging Security Lens outputs and static analysis.

use crate::agents::types::{
    IndentStyle, PerformanceThresholds, QualityCheck, QualityConfig, Severity, StyleRules,
};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::warn;

/// Quality validation agent for code analysis
#[derive(Debug)]
pub struct QualityValidationAgent {
    config: QualityConfig,
    security_patterns: SecurityPatternMatcher,
    performance_analyzer: PerformanceAnalyzer,
    style_checker: StyleChecker,
    complexity_analyzer: ComplexityAnalyzer,
}

/// Result of quality validation analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    pub overall_score: f64,
    pub issues: Vec<QualityIssue>,
    pub suggestions: Vec<QualityFix>,
    pub security_warnings: Vec<SecurityIssue>,
    pub metrics: QualityMetrics,
    pub analyzed_files: Vec<String>,
    pub analysis_timestamp: DateTime<Utc>,
}

/// Individual quality issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityIssue {
    pub severity: Severity,
    pub category: QualityCategory,
    pub message: String,
    pub file_path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub rule_id: String,
    pub suggestion: Option<String>,
}

/// Quality issue fix suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityFix {
    pub issue_id: String,
    pub description: String,
    pub suggested_code: Option<String>,
    pub confidence: f64,
    pub auto_fixable: bool,
}

/// Security issue details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityIssue {
    pub vulnerability_type: String,
    pub severity: Severity,
    pub description: String,
    pub file_path: String,
    pub line: Option<usize>,
    pub cwe_id: Option<String>,
    pub owasp_category: Option<String>,
    pub fix_suggestion: Option<String>,
}

/// Quality metrics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub cyclomatic_complexity: f64,
    pub lines_of_code: usize,
    pub code_coverage: f64,
    pub technical_debt_ratio: f64,
    pub maintainability_index: f64,
    pub security_score: f64,
    pub performance_score: f64,
    pub style_compliance: f64,
}

/// Quality issue categories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualityCategory {
    Security,
    Performance,
    Style,
    Complexity,
    Documentation,
    Testing,
    Dependencies,
    Maintainability,
    Reliability,
}

/// Security pattern matcher for vulnerability detection
#[derive(Debug)]
struct SecurityPatternMatcher {
    patterns: HashMap<String, Vec<SecurityPattern>>,
}

/// Security vulnerability pattern
#[derive(Debug, Clone)]
struct SecurityPattern {
    id: String,
    pattern: Regex,
    vulnerability_type: String,
    severity: Severity,
    cwe_id: Option<String>,
    owasp_category: Option<String>,
    description: String,
    fix_suggestion: String,
}

/// Performance analyzer for code efficiency
#[derive(Debug)]
struct PerformanceAnalyzer {
    thresholds: PerformanceThresholds,
}

/// Style checker for code formatting and conventions
#[derive(Debug)]
struct StyleChecker {
    rules: StyleRules,
}

/// Complexity analyzer for code maintainability
#[derive(Debug)]
struct ComplexityAnalyzer {
    max_complexity: u32,
}

/// Code analysis context
#[derive(Debug)]
struct AnalysisContext {
    file_path: String,
    content: String,
    language: Option<String>,
    line_count: usize,
}

/// Function complexity measurement
#[derive(Debug)]
struct FunctionComplexity {
    name: String,
    complexity: u32,
    line_start: usize,
    line_end: usize,
}

impl QualityValidationAgent {
    /// Create a new quality validation agent
    pub fn new(config: QualityConfig) -> Self {
        Self {
            security_patterns: SecurityPatternMatcher::new(),
            performance_analyzer: PerformanceAnalyzer::new(&config.performance_thresholds),
            style_checker: StyleChecker::new(&config.style_rules),
            complexity_analyzer: ComplexityAnalyzer::new(
                config.performance_thresholds.max_complexity,
            ),
            config,
        }
    }

    /// Analyze code quality for a single file
    pub async fn analyze_file<P: AsRef<Path>>(&self, file_path: P) -> Result<QualityReport> {
        let path = file_path.as_ref();
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read file {:?}: {}", path, e))?;

        let language = self.detect_language(path);
        let context = AnalysisContext {
            file_path: path.to_string_lossy().to_string(),
            content: content.clone(),
            language,
            line_count: content.lines().count(),
        };

        self.analyze_context(&context).await
    }

    /// Analyze code quality for multiple files
    pub async fn analyze_files<P: AsRef<Path>>(&self, file_paths: &[P]) -> Result<QualityReport> {
        let mut all_issues = Vec::new();
        let mut all_suggestions = Vec::new();
        let mut all_security_warnings = Vec::new();
        let mut analyzed_files = Vec::new();
        let mut combined_metrics = QualityMetrics::default();

        for path in file_paths {
            match self.analyze_file(path).await {
                Ok(report) => {
                    all_issues.extend(report.issues);
                    all_suggestions.extend(report.suggestions);
                    all_security_warnings.extend(report.security_warnings);
                    analyzed_files.extend(report.analyzed_files);
                    combined_metrics = self.combine_metrics(&combined_metrics, &report.metrics);
                }
                Err(e) => {
                    warn!("Failed to analyze file {:?}: {}", path.as_ref(), e);
                }
            }
        }

        let overall_score = self.calculate_overall_score(&all_issues, &combined_metrics);

        Ok(QualityReport {
            overall_score,
            issues: all_issues,
            suggestions: all_suggestions,
            security_warnings: all_security_warnings,
            metrics: combined_metrics,
            analyzed_files,
            analysis_timestamp: Utc::now(),
        })
    }

    /// Analyze code quality for a directory
    pub async fn analyze_directory<P: AsRef<Path>>(&self, dir_path: P) -> Result<QualityReport> {
        let dir = dir_path.as_ref();
        let mut file_paths = Vec::new();

        self.collect_source_files(dir, &mut file_paths)?;
        self.analyze_files(&file_paths).await
    }

    /// Analyze a single context
    async fn analyze_context(&self, context: &AnalysisContext) -> Result<QualityReport> {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();
        let mut security_warnings = Vec::new();

        // Run enabled checks
        if self.config.enabled_checks.contains(&QualityCheck::Security) {
            let security_results = self.security_patterns.analyze(context);
            security_warnings.extend(security_results.issues);
            issues.extend(security_results.quality_issues);
            suggestions.extend(security_results.suggestions);
        }

        if self.config.enabled_checks.contains(&QualityCheck::Performance) {
            let perf_results = self.performance_analyzer.analyze(context)?;
            issues.extend(perf_results.issues);
            suggestions.extend(perf_results.suggestions);
        }

        if self.config.enabled_checks.contains(&QualityCheck::Style) {
            let style_results = self.style_checker.analyze(context)?;
            issues.extend(style_results.issues);
            suggestions.extend(style_results.suggestions);
        }

        if self.config.enabled_checks.contains(&QualityCheck::Complexity) {
            let complexity_results = self.complexity_analyzer.analyze(context)?;
            issues.extend(complexity_results.issues);
            suggestions.extend(complexity_results.suggestions);
        }

        // Run custom rules
        let custom_results = self.analyze_custom_rules(context)?;
        issues.extend(custom_results.issues);
        suggestions.extend(custom_results.suggestions);

        // Calculate metrics
        let metrics = self.calculate_metrics(context, &issues, &security_warnings)?;
        let overall_score = self.calculate_overall_score(&issues, &metrics);

        Ok(QualityReport {
            overall_score,
            issues,
            suggestions,
            security_warnings,
            metrics,
            analyzed_files: vec![context.file_path.clone()],
            analysis_timestamp: Utc::now(),
        })
    }

    /// Detect programming language from file extension
    fn detect_language(&self, path: &Path) -> Option<String> {
        path.extension().and_then(|ext| ext.to_str()).map(|ext| {
            match ext.to_lowercase().as_str() {
                "rs" => "rust",
                "js" | "mjs" => "javascript",
                "ts" => "typescript",
                "py" => "python",
                "go" => "go",
                "java" => "java",
                "cpp" | "cxx" | "cc" => "cpp",
                "c" => "c",
                "cs" => "csharp",
                "rb" => "ruby",
                "php" => "php",
                "sh" | "bash" => "shell",
                _ => "unknown",
            }
            .to_string()
        })
    }

    /// Collect source files from directory recursively
    fn collect_source_files(&self, dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
        let entries =
            fs::read_dir(dir).map_err(|e| anyhow!("Failed to read directory {:?}: {}", dir, e))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && self.is_source_file(&path) {
                files.push(path);
            } else if path.is_dir() && !self.should_skip_directory(&path) {
                self.collect_source_files(&path, files)?;
            }
        }

        Ok(())
    }

    /// Check if file is a source code file
    fn is_source_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            matches!(
                ext.to_lowercase().as_str(),
                "rs" | "js"
                    | "ts"
                    | "py"
                    | "go"
                    | "java"
                    | "cpp"
                    | "c"
                    | "cs"
                    | "rb"
                    | "php"
                    | "sh"
                    | "bash"
                    | "scala"
                    | "kt"
                    | "swift"
            )
        } else {
            false
        }
    }

    /// Check if directory should be skipped during analysis
    fn should_skip_directory(&self, path: &Path) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            matches!(
                name,
                ".git"
                    | "node_modules"
                    | "target"
                    | "build"
                    | "dist"
                    | ".vscode"
                    | ".idea"
                    | "__pycache__"
                    | ".pytest_cache"
            )
        } else {
            false
        }
    }

    /// Analyze custom rules
    fn analyze_custom_rules(&self, context: &AnalysisContext) -> Result<AnalysisResults> {
        let mut issues = Vec::new();
        let suggestions = Vec::new();

        for rule in &self.config.custom_rules {
            if let Ok(pattern) = Regex::new(&rule.pattern) {
                for (line_num, line) in context.content.lines().enumerate() {
                    if pattern.is_match(line) {
                        issues.push(QualityIssue {
                            severity: rule.severity.clone(),
                            category: QualityCategory::Style, // Default category for custom rules
                            message: rule.message.clone(),
                            file_path: context.file_path.clone(),
                            line: Some(line_num + 1),
                            column: None,
                            rule_id: format!("custom_{}", rule.name),
                            suggestion: None,
                        });
                    }
                }
            }
        }

        Ok(AnalysisResults { issues, suggestions })
    }

    /// Calculate overall quality metrics
    fn calculate_metrics(
        &self,
        context: &AnalysisContext,
        issues: &[QualityIssue],
        security_warnings: &[SecurityIssue],
    ) -> Result<QualityMetrics> {
        let complexity = self.complexity_analyzer.calculate_average_complexity(context)?;
        let security_score = self.calculate_security_score(security_warnings);
        let performance_score = self.calculate_performance_score(issues);
        let style_compliance = self.calculate_style_compliance(issues);

        Ok(QualityMetrics {
            cyclomatic_complexity: complexity,
            lines_of_code: context.line_count,
            code_coverage: 0.0, // Would require external tooling
            technical_debt_ratio: self.calculate_technical_debt(issues),
            maintainability_index: self
                .calculate_maintainability_index(complexity, context.line_count),
            security_score,
            performance_score,
            style_compliance,
        })
    }

    /// Calculate overall quality score
    fn calculate_overall_score(&self, issues: &[QualityIssue], metrics: &QualityMetrics) -> f64 {
        let issue_penalty = issues
            .iter()
            .map(|issue| match issue.severity {
                Severity::Critical => 10.0,
                Severity::Error => 5.0,
                Severity::Warning => 2.0,
                Severity::Info => 0.5,
            })
            .sum::<f64>();

        let base_score = 100.0;
        let final_score = (base_score - issue_penalty).max(0.0).min(100.0);

        // Weight with metrics
        let weighted_score = (final_score * 0.6)
            + (metrics.security_score * 0.2)
            + (metrics.performance_score * 0.1)
            + (metrics.style_compliance * 0.1);

        weighted_score.max(0.0).min(100.0)
    }

    /// Calculate security score from security warnings
    fn calculate_security_score(&self, warnings: &[SecurityIssue]) -> f64 {
        if warnings.is_empty() {
            return 100.0;
        }

        let penalty = warnings
            .iter()
            .map(|warning| match warning.severity {
                Severity::Critical => 25.0,
                Severity::Error => 15.0,
                Severity::Warning => 10.0,
                Severity::Info => 5.0,
            })
            .sum::<f64>();

        (100.0 - penalty).max(0.0)
    }

    /// Calculate performance score from performance issues
    fn calculate_performance_score(&self, issues: &[QualityIssue]) -> f64 {
        let perf_issues: Vec<_> = issues
            .iter()
            .filter(|issue| matches!(issue.category, QualityCategory::Performance))
            .collect();

        if perf_issues.is_empty() {
            return 100.0;
        }

        let penalty = perf_issues
            .iter()
            .map(|issue| match issue.severity {
                Severity::Critical => 20.0,
                Severity::Error => 12.0,
                Severity::Warning => 8.0,
                Severity::Info => 3.0,
            })
            .sum::<f64>();

        (100.0 - penalty).max(0.0)
    }

    /// Calculate style compliance score
    fn calculate_style_compliance(&self, issues: &[QualityIssue]) -> f64 {
        let style_issues: Vec<_> = issues
            .iter()
            .filter(|issue| matches!(issue.category, QualityCategory::Style))
            .collect();

        if style_issues.is_empty() {
            return 100.0;
        }

        let penalty = style_issues.len() as f64 * 2.0;
        (100.0 - penalty).max(0.0)
    }

    /// Calculate technical debt ratio
    fn calculate_technical_debt(&self, issues: &[QualityIssue]) -> f64 {
        let debt_issues =
            issues.iter().filter(|issue| !matches!(issue.severity, Severity::Info)).count();

        debt_issues as f64 / issues.len().max(1) as f64
    }

    /// Calculate maintainability index
    fn calculate_maintainability_index(&self, complexity: f64, lines_of_code: usize) -> f64 {
        // Simplified maintainability index calculation
        let halstead_volume = lines_of_code as f64 * 0.1; // Placeholder
        let mi = 171.0
            - 5.2 * complexity.ln()
            - 0.23 * halstead_volume
            - 16.2 * (lines_of_code as f64).ln();
        mi.max(0.0).min(100.0)
    }

    /// Combine metrics from multiple files
    fn combine_metrics(&self, base: &QualityMetrics, new: &QualityMetrics) -> QualityMetrics {
        QualityMetrics {
            cyclomatic_complexity: (base.cyclomatic_complexity + new.cyclomatic_complexity) / 2.0,
            lines_of_code: base.lines_of_code + new.lines_of_code,
            code_coverage: (base.code_coverage + new.code_coverage) / 2.0,
            technical_debt_ratio: (base.technical_debt_ratio + new.technical_debt_ratio) / 2.0,
            maintainability_index: (base.maintainability_index + new.maintainability_index) / 2.0,
            security_score: (base.security_score + new.security_score) / 2.0,
            performance_score: (base.performance_score + new.performance_score) / 2.0,
            style_compliance: (base.style_compliance + new.style_compliance) / 2.0,
        }
    }
}

// Analysis result types
struct AnalysisResults {
    issues: Vec<QualityIssue>,
    suggestions: Vec<QualityFix>,
}

struct SecurityAnalysisResults {
    issues: Vec<SecurityIssue>,
    quality_issues: Vec<QualityIssue>,
    suggestions: Vec<QualityFix>,
}

// Component implementations
impl SecurityPatternMatcher {
    fn new() -> Self {
        let mut patterns = HashMap::new();

        // Common security patterns for different languages
        Self::add_common_patterns(&mut patterns);
        Self::add_language_specific_patterns(&mut patterns);

        Self { patterns }
    }

    fn add_common_patterns(patterns: &mut HashMap<String, Vec<SecurityPattern>>) {
        let mut common_patterns = Vec::new();

        // SQL Injection patterns
        if let Ok(pattern) = Regex::new(r"(?i)(select|insert|update|delete).*\+.*\+") {
            common_patterns.push(SecurityPattern {
                id: "sql_injection_concat".to_string(),
                pattern,
                vulnerability_type: "SQL Injection".to_string(),
                severity: Severity::Critical,
                cwe_id: Some("CWE-89".to_string()),
                owasp_category: Some("A03:2021 – Injection".to_string()),
                description: "Potential SQL injection through string concatenation".to_string(),
                fix_suggestion: "Use parameterized queries or prepared statements".to_string(),
            });
        }

        // Password/secret patterns
        if let Ok(pattern) =
            Regex::new(r#"(?i)(password|secret|key|token)\s*[:=]\s*['"][^'"\s]{8,}['"]"#)
        {
            common_patterns.push(SecurityPattern {
                id: "hardcoded_secret".to_string(),
                pattern,
                vulnerability_type: "Hardcoded Credentials".to_string(),
                severity: Severity::Critical,
                cwe_id: Some("CWE-798".to_string()),
                owasp_category: Some(
                    "A07:2021 – Identification and Authentication Failures".to_string(),
                ),
                description: "Hardcoded credentials detected".to_string(),
                fix_suggestion: "Use environment variables or secure credential storage"
                    .to_string(),
            });
        }

        patterns.insert("common".to_string(), common_patterns);
    }

    fn add_language_specific_patterns(patterns: &mut HashMap<String, Vec<SecurityPattern>>) {
        // JavaScript/TypeScript patterns
        let mut js_patterns = Vec::new();

        if let Ok(pattern) = Regex::new(r"eval\s*\(") {
            js_patterns.push(SecurityPattern {
                id: "js_eval_usage".to_string(),
                pattern,
                vulnerability_type: "Code Injection".to_string(),
                severity: Severity::Error,
                cwe_id: Some("CWE-94".to_string()),
                owasp_category: Some("A03:2021 – Injection".to_string()),
                description: "Use of eval() can lead to code injection vulnerabilities".to_string(),
                fix_suggestion: "Avoid eval() and use safer alternatives like JSON.parse()"
                    .to_string(),
            });
        }

        patterns.insert("javascript".to_string(), js_patterns.clone());
        patterns.insert("typescript".to_string(), js_patterns);
    }

    fn analyze(&self, context: &AnalysisContext) -> SecurityAnalysisResults {
        let mut issues = Vec::new();
        let mut quality_issues = Vec::new();
        let mut suggestions = Vec::new();

        // Check common patterns
        if let Some(common_patterns) = self.patterns.get("common") {
            self.check_patterns(
                context,
                common_patterns,
                &mut issues,
                &mut quality_issues,
                &mut suggestions,
            );
        }

        // Check language-specific patterns
        if let Some(language) = &context.language {
            if let Some(lang_patterns) = self.patterns.get(language) {
                self.check_patterns(
                    context,
                    lang_patterns,
                    &mut issues,
                    &mut quality_issues,
                    &mut suggestions,
                );
            }
        }

        SecurityAnalysisResults { issues, quality_issues, suggestions }
    }

    fn check_patterns(
        &self,
        context: &AnalysisContext,
        patterns: &[SecurityPattern],
        issues: &mut Vec<SecurityIssue>,
        quality_issues: &mut Vec<QualityIssue>,
        suggestions: &mut Vec<QualityFix>,
    ) {
        for pattern in patterns {
            for (line_num, line) in context.content.lines().enumerate() {
                if pattern.pattern.is_match(line) {
                    issues.push(SecurityIssue {
                        vulnerability_type: pattern.vulnerability_type.clone(),
                        severity: pattern.severity.clone(),
                        description: pattern.description.clone(),
                        file_path: context.file_path.clone(),
                        line: Some(line_num + 1),
                        cwe_id: pattern.cwe_id.clone(),
                        owasp_category: pattern.owasp_category.clone(),
                        fix_suggestion: Some(pattern.fix_suggestion.clone()),
                    });

                    quality_issues.push(QualityIssue {
                        severity: pattern.severity.clone(),
                        category: QualityCategory::Security,
                        message: pattern.description.clone(),
                        file_path: context.file_path.clone(),
                        line: Some(line_num + 1),
                        column: None,
                        rule_id: pattern.id.clone(),
                        suggestion: Some(pattern.fix_suggestion.clone()),
                    });

                    suggestions.push(QualityFix {
                        issue_id: pattern.id.clone(),
                        description: pattern.fix_suggestion.clone(),
                        suggested_code: None,
                        confidence: 0.8,
                        auto_fixable: false,
                    });
                }
            }
        }
    }
}

impl PerformanceAnalyzer {
    fn new(thresholds: &PerformanceThresholds) -> Self {
        Self { thresholds: thresholds.clone() }
    }

    fn analyze(&self, context: &AnalysisContext) -> Result<AnalysisResults> {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        // Check file size
        if context.line_count > self.thresholds.max_file_size_kb as usize * 10 {
            // rough estimate
            issues.push(QualityIssue {
                severity: Severity::Warning,
                category: QualityCategory::Performance,
                message: format!("File is too large ({} lines)", context.line_count),
                file_path: context.file_path.clone(),
                line: None,
                column: None,
                rule_id: "large_file".to_string(),
                suggestion: Some("Consider breaking this file into smaller modules".to_string()),
            });
        }

        // Check for performance anti-patterns
        self.check_performance_patterns(context, &mut issues, &mut suggestions);

        Ok(AnalysisResults { issues, suggestions })
    }

    fn check_performance_patterns(
        &self,
        context: &AnalysisContext,
        issues: &mut Vec<QualityIssue>,
        _suggestions: &mut Vec<QualityFix>,
    ) {
        // Language-specific performance patterns
        match context.language.as_deref() {
            Some("javascript" | "typescript") => {
                self.check_js_performance_patterns(context, issues);
            }
            Some("python") => {
                self.check_python_performance_patterns(context, issues);
            }
            _ => {}
        }
    }

    fn check_js_performance_patterns(
        &self,
        context: &AnalysisContext,
        issues: &mut Vec<QualityIssue>,
    ) {
        // Check for inefficient loops
        if let Ok(pattern) = Regex::new(r"document\.getElementById.*for\s*\(") {
            for (line_num, line) in context.content.lines().enumerate() {
                if pattern.is_match(line) {
                    issues.push(QualityIssue {
                        severity: Severity::Warning,
                        category: QualityCategory::Performance,
                        message: "DOM query inside loop can cause performance issues".to_string(),
                        file_path: context.file_path.clone(),
                        line: Some(line_num + 1),
                        column: None,
                        rule_id: "dom_query_in_loop".to_string(),
                        suggestion: Some("Cache DOM queries outside the loop".to_string()),
                    });
                }
            }
        }
    }

    fn check_python_performance_patterns(
        &self,
        context: &AnalysisContext,
        issues: &mut Vec<QualityIssue>,
    ) {
        // Check for inefficient string concatenation
        if let Ok(pattern) = Regex::new(r#"\+\s*=\s*['"].*['"]\s*\+"#) {
            for (line_num, line) in context.content.lines().enumerate() {
                if pattern.is_match(line) {
                    issues.push(QualityIssue {
                        severity: Severity::Warning,
                        category: QualityCategory::Performance,
                        message: "String concatenation with += in loop can be inefficient"
                            .to_string(),
                        file_path: context.file_path.clone(),
                        line: Some(line_num + 1),
                        column: None,
                        rule_id: "inefficient_string_concat".to_string(),
                        suggestion: Some(
                            "Use join() or f-strings for better performance".to_string(),
                        ),
                    });
                }
            }
        }
    }
}

impl StyleChecker {
    fn new(rules: &StyleRules) -> Self {
        Self { rules: rules.clone() }
    }

    fn analyze(&self, context: &AnalysisContext) -> Result<AnalysisResults> {
        let mut issues = Vec::new();
        let suggestions = Vec::new();

        // Check line length
        for (line_num, line) in context.content.lines().enumerate() {
            if line.len() > self.rules.max_line_length as usize {
                issues.push(QualityIssue {
                    severity: Severity::Warning,
                    category: QualityCategory::Style,
                    message: format!(
                        "Line exceeds maximum length of {} characters",
                        self.rules.max_line_length
                    ),
                    file_path: context.file_path.clone(),
                    line: Some(line_num + 1),
                    column: Some(self.rules.max_line_length as usize),
                    rule_id: "line_too_long".to_string(),
                    suggestion: Some("Break long lines into multiple shorter lines".to_string()),
                });
            }
        }

        // Check indentation (simplified)
        self.check_indentation(context, &mut issues);

        Ok(AnalysisResults { issues, suggestions })
    }

    fn check_indentation(&self, context: &AnalysisContext, issues: &mut Vec<QualityIssue>) {
        for (line_num, line) in context.content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let leading_spaces = line.len() - line.trim_start().len();

            match self.rules.indent_style {
                IndentStyle::Spaces(size) => {
                    if leading_spaces % size as usize != 0 && leading_spaces > 0 {
                        issues.push(QualityIssue {
                            severity: Severity::Info,
                            category: QualityCategory::Style,
                            message: format!("Inconsistent indentation (expected {} spaces)", size),
                            file_path: context.file_path.clone(),
                            line: Some(line_num + 1),
                            column: Some(1),
                            rule_id: "inconsistent_indentation".to_string(),
                            suggestion: Some(format!("Use {} spaces for indentation", size)),
                        });
                    }
                }
                IndentStyle::Tabs => {
                    if line.starts_with(' ') {
                        issues.push(QualityIssue {
                            severity: Severity::Info,
                            category: QualityCategory::Style,
                            message: "Use tabs for indentation instead of spaces".to_string(),
                            file_path: context.file_path.clone(),
                            line: Some(line_num + 1),
                            column: Some(1),
                            rule_id: "spaces_instead_of_tabs".to_string(),
                            suggestion: Some("Replace leading spaces with tabs".to_string()),
                        });
                    }
                }
                IndentStyle::Mixed => {
                    // No specific rules for mixed indentation
                }
            }
        }
    }
}

impl ComplexityAnalyzer {
    fn new(max_complexity: u32) -> Self {
        Self { max_complexity }
    }

    fn analyze(&self, context: &AnalysisContext) -> Result<AnalysisResults> {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        let functions = self.extract_functions(context);

        for function in functions {
            if function.complexity > self.max_complexity {
                issues.push(QualityIssue {
                    severity: if function.complexity > self.max_complexity * 2 {
                        Severity::Error
                    } else {
                        Severity::Warning
                    },
                    category: QualityCategory::Complexity,
                    message: format!(
                        "Function '{}' has high cyclomatic complexity ({})",
                        function.name, function.complexity
                    ),
                    file_path: context.file_path.clone(),
                    line: Some(function.line_start),
                    column: None,
                    rule_id: "high_complexity".to_string(),
                    suggestion: Some(
                        "Consider breaking this function into smaller functions".to_string(),
                    ),
                });

                suggestions.push(QualityFix {
                    issue_id: "high_complexity".to_string(),
                    description: format!("Refactor '{}' to reduce complexity", function.name),
                    suggested_code: None,
                    confidence: 0.7,
                    auto_fixable: false,
                });
            }
        }

        Ok(AnalysisResults { issues, suggestions })
    }

    fn extract_functions(&self, context: &AnalysisContext) -> Vec<FunctionComplexity> {
        let mut functions = Vec::new();

        // Simplified function detection (would need proper parsing for accuracy)
        match context.language.as_deref() {
            Some("rust") => self.extract_rust_functions(context, &mut functions),
            Some("javascript" | "typescript") => self.extract_js_functions(context, &mut functions),
            Some("python") => self.extract_python_functions(context, &mut functions),
            _ => {}
        }

        functions
    }

    fn extract_rust_functions(
        &self,
        context: &AnalysisContext,
        functions: &mut Vec<FunctionComplexity>,
    ) {
        if let Ok(pattern) = Regex::new(r"fn\s+(\w+)\s*\(") {
            for (line_num, line) in context.content.lines().enumerate() {
                if let Some(captures) = pattern.captures(line) {
                    if let Some(name) = captures.get(1) {
                        let complexity = self.calculate_function_complexity(context, line_num);
                        functions.push(FunctionComplexity {
                            name: name.as_str().to_string(),
                            complexity,
                            line_start: line_num + 1,
                            line_end: line_num + 20, // Simplified
                        });
                    }
                }
            }
        }
    }

    fn extract_js_functions(
        &self,
        context: &AnalysisContext,
        functions: &mut Vec<FunctionComplexity>,
    ) {
        if let Ok(pattern) = Regex::new(r"function\s+(\w+)\s*\(|const\s+(\w+)\s*=\s*\(") {
            for (line_num, line) in context.content.lines().enumerate() {
                if let Some(captures) = pattern.captures(line) {
                    let name = captures
                        .get(1)
                        .or_else(|| captures.get(2))
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_else(|| "anonymous".to_string());

                    let complexity = self.calculate_function_complexity(context, line_num);
                    functions.push(FunctionComplexity {
                        name,
                        complexity,
                        line_start: line_num + 1,
                        line_end: line_num + 20, // Simplified
                    });
                }
            }
        }
    }

    fn extract_python_functions(
        &self,
        context: &AnalysisContext,
        functions: &mut Vec<FunctionComplexity>,
    ) {
        if let Ok(pattern) = Regex::new(r"def\s+(\w+)\s*\(") {
            for (line_num, line) in context.content.lines().enumerate() {
                if let Some(captures) = pattern.captures(line) {
                    if let Some(name) = captures.get(1) {
                        let complexity = self.calculate_function_complexity(context, line_num);
                        functions.push(FunctionComplexity {
                            name: name.as_str().to_string(),
                            complexity,
                            line_start: line_num + 1,
                            line_end: line_num + 20, // Simplified
                        });
                    }
                }
            }
        }
    }

    fn calculate_function_complexity(&self, context: &AnalysisContext, start_line: usize) -> u32 {
        // Simplified complexity calculation - count decision points
        let decision_patterns = [
            r"\bif\b",
            r"\belse\b",
            r"\bwhile\b",
            r"\bfor\b",
            r"\bswitch\b",
            r"\bmatch\b",
            r"\bcatch\b",
            r"&&",
            r"\|\|",
        ];

        let mut complexity = 1; // Base complexity
        let lines_to_check = context.content.lines().skip(start_line).take(50); // Check next 50 lines

        for line in lines_to_check {
            for pattern_str in &decision_patterns {
                if let Ok(pattern) = Regex::new(pattern_str) {
                    complexity += pattern.find_iter(line).count() as u32;
                }
            }
        }

        complexity
    }

    fn calculate_average_complexity(&self, context: &AnalysisContext) -> Result<f64> {
        let functions = self.extract_functions(context);

        if functions.is_empty() {
            return Ok(1.0); // No functions found, minimum complexity
        }

        let total_complexity: u32 = functions.iter().map(|f| f.complexity).sum();
        Ok(total_complexity as f64 / functions.len() as f64)
    }
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self {
            cyclomatic_complexity: 1.0,
            lines_of_code: 0,
            code_coverage: 0.0,
            technical_debt_ratio: 0.0,
            maintainability_index: 100.0,
            security_score: 100.0,
            performance_score: 100.0,
            style_compliance: 100.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_quality_validation_agent_creation() {
        let config = QualityConfig::default();
        let _agent = QualityValidationAgent::new(config);

        // Should not panic
    }

    #[tokio::test]
    async fn test_security_pattern_detection() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "password = 'hardcoded123'").unwrap();
        writeln!(temp_file, "query = 'SELECT * FROM users WHERE id=' + user_id").unwrap();

        let config = QualityConfig::default();
        let agent = QualityValidationAgent::new(config);

        let report = agent.analyze_file(temp_file.path()).await.unwrap();

        assert!(!report.security_warnings.is_empty());
        assert!(report.overall_score < 100.0);
    }

    #[test]
    fn test_language_detection() {
        let config = QualityConfig::default();
        let agent = QualityValidationAgent::new(config);

        assert_eq!(agent.detect_language(Path::new("test.rs")), Some("rust".to_string()));
        assert_eq!(agent.detect_language(Path::new("test.js")), Some("javascript".to_string()));
        assert_eq!(agent.detect_language(Path::new("test.py")), Some("python".to_string()));
    }

    #[test]
    fn test_complexity_calculation() {
        let config = QualityConfig::default();
        let analyzer = ComplexityAnalyzer::new(config.performance_thresholds.max_complexity);

        let context = AnalysisContext {
            file_path: "test.rs".to_string(),
            content: "fn test() { if x { while y { for z { } } } }".to_string(),
            language: Some("rust".to_string()),
            line_count: 1,
        };

        let complexity = analyzer.calculate_function_complexity(&context, 0);
        assert!(complexity > 1); // Should detect decision points
    }

    #[tokio::test]
    async fn test_style_checking() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "fn test() {{").unwrap();
        writeln!(temp_file, "      let x = 1; // 6 spaces instead of 4").unwrap();
        writeln!(temp_file, "}}").unwrap();

        let mut config = QualityConfig::default();
        config.enabled_checks.insert(QualityCheck::Style);

        let agent = QualityValidationAgent::new(config);
        let report = agent.analyze_file(temp_file.path()).await.unwrap();

        let style_issues: Vec<_> = report
            .issues
            .iter()
            .filter(|issue| matches!(issue.category, QualityCategory::Style))
            .collect();

        assert!(!style_issues.is_empty());
    }
}
