use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use uuid::Uuid;
// Needed for sha256 digest and base64 encoding helpers
use base64::Engine;
use sha2::Digest;

use super::advanced_conversation_features::AdvancedConversationFeatures;
use super::conversation_manager::ConversationManager;
use super::*;

/// Privacy content filter for protecting sensitive information
pub struct PrivacyContentFilter {
    id: String,
    conversation_manager: Option<Arc<ConversationManager>>,
    advanced_features: Option<Arc<AdvancedConversationFeatures>>,
    privacy_policies: Arc<RwLock<HashMap<String, PrivacyPolicy>>>,
    content_scanners: Arc<RwLock<Vec<ContentScanner>>>,
    redaction_engine: Arc<RedactionEngine>,
    data_classifier: Arc<DataClassifier>,
    privacy_audit: Arc<RwLock<PrivacyAuditLog>>,
    config: PrivacyFilterConfig,
    is_initialized: bool,
}

/// Privacy policy definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    pub id: String,
    pub name: String,
    pub description: String,
    pub data_classifications: Vec<DataClassification>,
    pub redaction_rules: Vec<RedactionRule>,
    pub retention_policies: HashMap<String, Duration>,
    pub access_controls: Vec<AccessControl>,
    pub compliance_standards: Vec<ComplianceStandard>,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub version: String,
    pub is_active: bool,
}

/// Data classification levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DataClassification {
    Public,         // No restrictions
    Internal,       // Internal use only
    Confidential,   // Limited access required
    Restricted,     // Highly restricted access
    TopSecret,      // Maximum security required
    PersonalData,   // PII/personal information
    FinancialData,  // Financial/payment information
    HealthData,     // Medical/health information
    LegalData,      // Legal/attorney-client privileged
    Custom(String), // Custom classification
}

/// Content scanner for detecting sensitive information
#[derive(Debug, Clone)]
pub struct ContentScanner {
    pub id: String,
    pub name: String,
    pub scanner_type: ScannerType,
    pub patterns: Vec<ScanPattern>,
    pub confidence_threshold: f32,
    pub data_classification: DataClassification,
    pub is_enabled: bool,
}

/// Types of content scanners
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScannerType {
    RegexPattern,    // Regular expression matching
    KeywordList,     // Keyword/phrase matching
    ContextualNLP,   // Natural language processing
    MachineLearning, // ML-based classification
    HashComparison,  // Hash-based matching
    Custom(String),  // Custom scanner implementation
}

/// Pattern for content scanning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanPattern {
    pub pattern: String,
    pub pattern_type: PatternType,
    pub sensitivity: SensitivityLevel,
    pub context_requirements: Vec<String>,
    pub false_positive_filters: Vec<String>,
}

/// Types of scan patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    Regex,
    Keyword,
    PhoneticMatch,
    SemanticSimilarity,
    StructuralMatch,
}

/// Sensitivity levels for detected content
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SensitivityLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Redaction rule for content modification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionRule {
    pub id: String,
    pub name: String,
    pub data_classification: DataClassification,
    pub redaction_method: RedactionMethod,
    pub preserve_format: bool,
    pub replacement_pattern: Option<String>,
    pub conditions: Vec<RedactionCondition>,
}

/// Methods for redacting content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RedactionMethod {
    FullRedaction,    // Complete removal: [REDACTED]
    PartialRedaction, // Partial masking: jo**@ex***.com
    TokenReplacement, // Token replacement: [EMAIL_ADDRESS]
    Anonymization,    // Anonymize while preserving structure
    Encryption,       // Encrypt sensitive parts
    Hashing,          // Replace with hash
    Custom(String),   // Custom redaction logic
}

/// Conditions for applying redaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionCondition {
    pub condition_type: ConditionType,
    pub value: String,
    pub operator: ComparisonOperator,
}

/// Types of redaction conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionType {
    Context,        // Based on surrounding content
    UserRole,       // Based on user permissions
    DataType,       // Based on data classification
    Confidence,     // Based on detection confidence
    TimeOfDay,      // Based on access time
    Location,       // Based on user location
    Custom(String), // Custom condition logic
}

/// Comparison operators for conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Contains,
    NotContains,
    Matches,
    NotMatches,
}

/// Access control definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControl {
    pub id: String,
    pub name: String,
    pub data_classification: DataClassification,
    pub allowed_roles: Vec<String>,
    pub denied_roles: Vec<String>,
    pub time_restrictions: Vec<TimeRestriction>,
    pub location_restrictions: Vec<String>,
    pub approval_required: bool,
    pub audit_required: bool,
}

/// Time-based access restrictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRestriction {
    pub days_of_week: Vec<u8>, // 0=Sunday, 6=Saturday
    pub start_time: String,    // HH:MM format
    pub end_time: String,      // HH:MM format
    pub timezone: String,
}

/// Compliance standards
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComplianceStandard {
    GDPR,           // General Data Protection Regulation
    CCPA,           // California Consumer Privacy Act
    HIPAA,          // Health Insurance Portability and Accountability Act
    SOX,            // Sarbanes-Oxley Act
    PciDss,         // Payment Card Industry Data Security Standard
    FISMA,          // Federal Information Security Management Act
    ISO27001,       // ISO/IEC 27001
    NIST,           // NIST Cybersecurity Framework
    Custom(String), // Custom compliance standard
}

/// Redaction engine for content modification
pub struct RedactionEngine {
    redaction_cache: RwLock<HashMap<String, RedactionResult>>,
    anonymization_mappings: RwLock<HashMap<String, String>>,
    encryption_key: Option<String>,
}

impl RedactionEngine {
    pub fn set_encryption_key(&mut self, key: Option<String>) {
        self.encryption_key = key;
    }

    pub async fn get_cached(&self, key: &str) -> Option<RedactionResult> {
        self.redaction_cache.read().await.get(key).cloned()
    }

    pub async fn put_cached(&self, key: String, value: RedactionResult) {
        self.redaction_cache.write().await.insert(key, value);
    }

    pub async fn anonymize_token(&self, token: &str) -> String {
        if let Some(mapped) = self.anonymization_mappings.read().await.get(token) {
            return mapped.clone();
        }
        let anon = format!(
            "anon_{}",
            sha2::Sha256::digest(token.as_bytes())
                .iter()
                .take(8)
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        );
        self.anonymization_mappings.write().await.insert(token.to_string(), anon.clone());
        anon
    }

    pub fn encrypt_if_enabled(&self, data: &str) -> String {
        if let Some(key) = &self.encryption_key {
            let mut hasher = sha2::Sha256::new();
            hasher.update(key.as_bytes());
            hasher.update(data.as_bytes());
            let hash = hasher.finalize();
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(hash)
        } else {
            data.to_string()
        }
    }
}

/// Result of content redaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionResult {
    pub original_content: String,
    pub redacted_content: String,
    pub redactions_applied: Vec<RedactionApplication>,
    pub data_classifications: Vec<DataClassification>,
    pub confidence_score: f32,
    pub processing_time_ms: u64,
    pub created_at: DateTime<Utc>,
}

/// Individual redaction application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionApplication {
    pub rule_id: String,
    pub start_position: usize,
    pub end_position: usize,
    pub original_text: String,
    pub redacted_text: String,
    pub redaction_method: RedactionMethod,
    pub confidence: f32,
}

/// Data classifier for content analysis
pub struct DataClassifier {
    classification_models: RwLock<HashMap<String, ClassificationModel>>,
    entity_extractors: RwLock<Vec<EntityExtractor>>,
    context_analyzers: RwLock<Vec<ContextAnalyzer>>,
}

impl DataClassifier {
    pub async fn add_model(&self, model: ClassificationModel) {
        self.classification_models.write().await.insert(model.model_id.clone(), model);
    }
    pub async fn add_extractor(&self, ex: EntityExtractor) {
        self.entity_extractors.write().await.push(ex);
    }
    pub async fn add_context_analyzer(&self, an: ContextAnalyzer) {
        self.context_analyzers.write().await.push(an);
    }
}

/// Machine learning model for classification
pub struct ClassificationModel {
    pub model_id: String,
    pub model_type: ModelType,
    pub accuracy: f32,
    pub last_trained: DateTime<Utc>,
    pub feature_extractors: Vec<String>,
}

/// Types of classification models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelType {
    NaiveBayes,
    SVM,
    RandomForest,
    NeuralNetwork,
    TransformerBased,
    Custom(String),
}

/// Entity extractor for identifying specific data types
#[derive(Debug, Clone)]
pub struct EntityExtractor {
    pub extractor_id: String,
    pub entity_type: String,
    pub extraction_patterns: Vec<Regex>,
    pub validation_rules: Vec<ValidationRule>,
    pub confidence_calculator: ConfidenceCalculator,
}

/// Validation rule for extracted entities
#[derive(Debug, Clone)]
pub struct ValidationRule {
    pub rule_type: ValidationType,
    pub parameters: HashMap<String, String>,
    pub weight: f32,
}

/// Types of validation
#[derive(Debug, Clone)]
pub enum ValidationType {
    FormatValidation,
    ChecksumValidation,
    ContextValidation,
    ExternalVerification,
    Custom(String),
}

/// Confidence calculation logic
#[derive(Debug, Clone)]
pub struct ConfidenceCalculator {
    pub base_confidence: f32,
    pub context_boost: f32,
    pub validation_weight: f32,
    pub false_positive_penalty: f32,
}

/// Context analyzer for understanding content context
#[derive(Debug, Clone)]
pub struct ContextAnalyzer {
    pub analyzer_id: String,
    pub context_types: Vec<ContextType>,
    pub window_size: usize,
    pub relevance_threshold: f32,
}

/// Types of context analysis
#[derive(Debug, Clone)]
pub enum ContextType {
    TopicalContext,  // Subject matter context
    TemporalContext, // Time-based context
    SpatialContext,  // Location-based context
    SocialContext,   // Social/relationship context
    SecurityContext, // Security classification context
    Custom(String),  // Custom context type
}

/// Privacy audit logging system
pub struct PrivacyAuditLog {
    audit_entries: Vec<AuditEntry>,
    retention_period: Duration,
    encryption_enabled: bool,
}

impl PrivacyAuditLog {
    pub fn set_encryption(&mut self, enabled: bool) {
        self.encryption_enabled = enabled;
    }
}

/// Individual audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub user_id: Option<String>,
    pub session_id: Option<Uuid>,
    pub data_classification: DataClassification,
    pub action_taken: String,
    pub content_hash: Option<String>,
    pub policy_id: Option<String>,
    pub compliance_tags: Vec<String>,
    pub severity: AuditSeverity,
}

/// Types of privacy audit events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditEventType {
    ContentScanned,
    DataClassified,
    ContentRedacted,
    AccessGranted,
    AccessDenied,
    PolicyViolation,
    ComplianceCheck,
    DataRetention,
    DataDeletion,
    EncryptionApplied,
    Custom(String),
}

/// Severity levels for audit events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum AuditSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Configuration for privacy content filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyFilterConfig {
    pub enable_real_time_scanning: bool,
    pub enable_batch_processing: bool,
    pub enable_audit_logging: bool,
    pub default_data_classification: DataClassification,
    pub max_content_size_mb: usize,
    pub scan_timeout_seconds: u64,
    pub cache_results: bool,
    pub cache_ttl_minutes: u64,
    pub compliance_standards: Vec<ComplianceStandard>,
    pub notification_channels: Vec<String>,
}

impl Default for PrivacyFilterConfig {
    fn default() -> Self {
        Self {
            enable_real_time_scanning: true,
            enable_batch_processing: true,
            enable_audit_logging: true,
            default_data_classification: DataClassification::Internal,
            max_content_size_mb: 10,
            scan_timeout_seconds: 30,
            cache_results: true,
            cache_ttl_minutes: 60,
            compliance_standards: vec![ComplianceStandard::GDPR],
            notification_channels: Vec::new(),
        }
    }
}

impl PrivacyContentFilter {
    pub fn new() -> Self {
        Self {
            id: "privacy-content-filter".to_string(),
            conversation_manager: None,
            advanced_features: None,
            privacy_policies: Arc::new(RwLock::new(HashMap::new())),
            content_scanners: Arc::new(RwLock::new(Vec::new())),
            redaction_engine: Arc::new(RedactionEngine::new()),
            data_classifier: Arc::new(DataClassifier::new()),
            privacy_audit: Arc::new(RwLock::new(PrivacyAuditLog::new())),
            config: PrivacyFilterConfig::default(),
            is_initialized: false,
        }
    }
}

impl Default for PrivacyContentFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivacyContentFilter {
    pub fn with_config(mut self, config: PrivacyFilterConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_conversation_manager(mut self, manager: Arc<ConversationManager>) -> Self {
        self.conversation_manager = Some(manager);
        self
    }

    pub fn with_advanced_features(mut self, features: Arc<AdvancedConversationFeatures>) -> Self {
        self.advanced_features = Some(features);
        self
    }

    /// Scan content for sensitive information
    pub async fn scan_content(&self, content: &str, context: Option<&str>) -> Result<ScanResult> {
        if !self.config.enable_real_time_scanning {
            return Ok(ScanResult::default());
        }

        let start_time = std::time::Instant::now();
        let mut detections: Vec<SensitiveDataDetection> = Vec::new();
        let mut data_classifications = HashSet::new();

        // Run all enabled scanners in parallel
        let scanners_guard = self.content_scanners.read().await;
        let enabled_scanners: Vec<ContentScanner> =
            scanners_guard.iter().filter(|s| s.is_enabled).cloned().collect();
        drop(scanners_guard);

        let mut joinset: JoinSet<(ContentScanner, Vec<SensitiveDataDetection>)> = JoinSet::new();
        let content_owned = content.to_string();
        let context_owned = context.map(|s| s.to_string());

        for scanner in enabled_scanners {
            let content_clone = content_owned.clone();
            let context_clone = context_owned.clone();
            let scanner_clone = scanner.clone();
            joinset.spawn(async move {
                let dets = PrivacyContentFilter::run_scanner_internal(
                    &scanner_clone,
                    &content_clone,
                    context_clone.as_deref(),
                );
                (scanner_clone, dets)
            });
        }

        while let Some(res) = joinset.join_next().await {
            if let Ok((scanner, mut dets)) = res {
                // Apply threshold filtering and collect classifications
                dets.retain(|d| d.confidence >= scanner.confidence_threshold);
                if !dets.is_empty() {
                    data_classifications.insert(scanner.data_classification.clone());
                }
                detections.extend(dets);
            }
        }

        // Sort detections by start position for deterministic order
        detections.sort_by_key(|d| d.start_position);

        // Classify overall content
        let overall_classification = self.classify_content(content, &detections).await?;
        data_classifications.insert(overall_classification);

        let processing_time = start_time.elapsed();

        // Log audit event
        if self.config.enable_audit_logging {
            let data_classifications_refs: HashSet<&DataClassification> =
                data_classifications.iter().collect();
            self.log_audit_event(
                AuditEventType::ContentScanned,
                None,
                None,
                &data_classifications_refs,
            )
            .await?;
        }

        let risk_score = self.calculate_risk_score(&detections);

        Ok(ScanResult {
            content: content.to_string(),
            detections,
            data_classifications: data_classifications.into_iter().collect(),
            overall_risk_score: risk_score,
            processing_time_ms: processing_time.as_millis() as u64,
            timestamp: Utc::now(),
        })
    }

    /// Apply redaction to content based on privacy policies
    pub async fn redact_content(
        &self,
        content: &str,
        policy_id: &str,
        user_context: Option<&UserContext>,
    ) -> Result<RedactionResult> {
        // Get privacy policy
        let policies = self.privacy_policies.read().await;
        let policy = policies
            .get(policy_id)
            .ok_or_else(|| anyhow!("Privacy policy not found: {}", policy_id))?;

        if !policy.is_active {
            return Err(anyhow!("Privacy policy is not active: {}", policy_id));
        }

        // First scan content to identify sensitive data
        let scan_result = self.scan_content(content, None).await?;

        let mut redacted_content = content.to_string();
        let mut redactions_applied = Vec::new();

        // Apply redaction rules
        for rule in &policy.redaction_rules {
            if self.should_apply_redaction(rule, &scan_result, user_context).await? {
                let rule_redactions = self
                    .apply_redaction_rule(&redacted_content, rule, &scan_result.detections)
                    .await?;

                for redaction in rule_redactions {
                    // Apply the redaction to content
                    redacted_content = self
                        .redaction_engine
                        .apply_redaction(&redacted_content, &redaction)
                        .await?;
                    redactions_applied.push(redaction);
                }
            }
        }

        let result = RedactionResult {
            original_content: content.to_string(),
            redacted_content,
            redactions_applied,
            data_classifications: scan_result.data_classifications,
            confidence_score: scan_result.overall_risk_score,
            processing_time_ms: scan_result.processing_time_ms,
            created_at: Utc::now(),
        };

        // Log audit event
        if self.config.enable_audit_logging {
            self.log_audit_event(
                AuditEventType::ContentRedacted,
                user_context.map(|c| c.user_id.clone()),
                None,
                &result.data_classifications.iter().collect(),
            )
            .await?;
        }

        Ok(result)
    }

    /// Create a new privacy policy
    pub async fn create_privacy_policy(&self, policy: PrivacyPolicy) -> Result<()> {
        let mut policies = self.privacy_policies.write().await;
        policies.insert(policy.id.clone(), policy);
        Ok(())
    }

    /// Add a content scanner
    pub async fn add_content_scanner(&self, scanner: ContentScanner) -> Result<()> {
        let mut scanners = self.content_scanners.write().await;
        scanners.push(scanner);
        Ok(())
    }

    /// Get privacy compliance report
    pub async fn generate_compliance_report(
        &self,
        standard: ComplianceStandard,
        date_range: (DateTime<Utc>, DateTime<Utc>),
    ) -> Result<ComplianceReport> {
        let _use_classifier = &self.data_classifier;
        let audit = self.privacy_audit.read().await;

        let relevant_entries: Vec<&AuditEntry> = audit
            .audit_entries
            .iter()
            .filter(|entry| {
                entry.timestamp >= date_range.0
                    && entry.timestamp <= date_range.1
                    && entry.compliance_tags.contains(&format!("{:?}", standard))
            })
            .collect();

        let compliance_score = self.calculate_compliance_score(&relevant_entries, &standard);
        let recommendations =
            self.generate_compliance_recommendations(&standard, &relevant_entries);

        let report = ComplianceReport {
            standard,
            date_range,
            total_events: relevant_entries.len(),
            violations: relevant_entries
                .iter()
                .filter(|e| e.event_type == AuditEventType::PolicyViolation)
                .count(),
            high_risk_events: relevant_entries
                .iter()
                .filter(|e| e.severity >= AuditSeverity::Error)
                .count(),
            data_processed: relevant_entries.len(), // Simplified
            compliance_score,
            recommendations,
            generated_at: Utc::now(),
        };

        Ok(report)
    }

    // Helper methods

    fn run_scanner_internal(
        scanner: &ContentScanner,
        content: &str,
        _context: Option<&str>,
    ) -> Vec<SensitiveDataDetection> {
        let mut detections = Vec::new();

        match scanner.scanner_type {
            ScannerType::RegexPattern => {
                for pattern in &scanner.patterns {
                    if let PatternType::Regex = pattern.pattern_type {
                        let regex = match Regex::new(&pattern.pattern) {
                            Ok(r) => r,
                            Err(_) => continue,
                        };
                        // Pre-compile false-positive filters as regex where possible
                        let fp_filters: Vec<Regex> = pattern
                            .false_positive_filters
                            .iter()
                            .filter_map(|p| Regex::new(p).ok())
                            .collect();
                        for mat in regex.find_iter(content) {
                            let matched_text = mat.as_str();

                            // Skip if false-positive filter matches the token itself
                            if fp_filters.iter().any(|re| re.is_match(matched_text)) {
                                continue;
                            }

                            // Optional simple context window check (use safe slicing on char boundaries)
                            let window_start = mat.start().saturating_sub(64);
                            let window_end = (mat.end() + 64).min(content.len());
                            let window =
                                content.get(window_start..window_end).unwrap_or(matched_text);
                            if !pattern.context_requirements.is_empty() {
                                let window_lower = window.to_ascii_lowercase();
                                if !pattern
                                    .context_requirements
                                    .iter()
                                    .any(|req| window_lower.contains(&req.to_ascii_lowercase()))
                                {
                                    // Require at least one context hint when specified
                                    continue;
                                }
                            }

                            // Heuristic confidence
                            let mut confidence = 0.8f32;

                            // Special handling for certain scanner IDs
                            match scanner.id.as_str() {
                                // Credit card numbers: validate with Luhn and normalize digits
                                "credit-card-scanner" => {
                                    let digits: String = matched_text
                                        .chars()
                                        .filter(|c| c.is_ascii_digit())
                                        .collect();
                                    let len = digits.len();
                                    if (13..=19).contains(&len) && Self::luhn_check(&digits) {
                                        confidence = 0.99;
                                    } else {
                                        // Do not record if checksum fails
                                        continue;
                                    }
                                }
                                // GitHub token, AWS AKID, JWT scanners get high confidence
                                "github-token-scanner" | "aws-akid-scanner" | "jwt-scanner" => {
                                    confidence = 0.95;
                                }
                                _ => {}
                            }

                            detections.push(SensitiveDataDetection {
                                detection_id: Uuid::new_v4(),
                                scanner_id: scanner.id.clone(),
                                data_type: format!("{:?}", scanner.data_classification),
                                start_position: mat.start(),
                                end_position: mat.end(),
                                matched_text: matched_text.to_string(),
                                confidence,
                                context: None,
                                sensitivity: pattern.sensitivity.clone(),
                            });
                        }
                    }
                }
            }
            ScannerType::KeywordList => {
                for pattern in &scanner.patterns {
                    if let PatternType::Keyword = pattern.pattern_type {
                        let content_lower = content.to_ascii_lowercase();
                        let needle = pattern.pattern.to_ascii_lowercase();
                        if content_lower.contains(&needle) {
                            if let Some(pos) = content_lower.find(&needle) {
                                detections.push(SensitiveDataDetection {
                                    detection_id: Uuid::new_v4(),
                                    scanner_id: scanner.id.clone(),
                                    data_type: format!("{:?}", scanner.data_classification),
                                    start_position: pos,
                                    end_position: pos + pattern.pattern.len(),
                                    matched_text: pattern.pattern.clone(),
                                    confidence: 0.7,
                                    context: None,
                                    sensitivity: pattern.sensitivity.clone(),
                                });
                            }
                        }
                    }
                }
            }
            _ => {
                // Other scanner types would be implemented here
            }
        }

        detections
    }

    fn luhn_check(digits: &str) -> bool {
        let mut sum = 0u32;
        let mut alt = false;
        // Process digits right-to-left
        for ch in digits.chars().rev() {
            if let Some(mut d) = ch.to_digit(10) {
                if alt {
                    d *= 2;
                    if d > 9 {
                        d -= 9;
                    }
                }
                sum += d;
                alt = !alt;
            } else {
                return false;
            }
        }
        sum % 10 == 0
    }

    async fn classify_content(
        &self,
        _content: &str,
        detections: &[SensitiveDataDetection],
    ) -> Result<DataClassification> {
        // Simple classification based on detections
        let highest_sensitivity = detections
            .iter()
            .map(|d| &d.sensitivity)
            .max()
            .cloned()
            .unwrap_or(SensitivityLevel::Low);

        Ok(match highest_sensitivity {
            SensitivityLevel::Critical => DataClassification::Restricted,
            SensitivityLevel::High => DataClassification::Confidential,
            SensitivityLevel::Medium => DataClassification::Internal,
            SensitivityLevel::Low => DataClassification::Public,
        })
    }

    fn calculate_risk_score(&self, detections: &[SensitiveDataDetection]) -> f32 {
        if detections.is_empty() {
            return 0.0;
        }

        let total_confidence: f32 = detections.iter().map(|d| d.confidence).sum();
        let avg_confidence = total_confidence / detections.len() as f32;

        // Weight by sensitivity
        let sensitivity_weight: f32 = detections
            .iter()
            .map(|d| match d.sensitivity {
                SensitivityLevel::Critical => 1.0,
                SensitivityLevel::High => 0.8,
                SensitivityLevel::Medium => 0.5,
                SensitivityLevel::Low => 0.2,
            })
            .sum::<f32>()
            / detections.len() as f32;

        (avg_confidence * sensitivity_weight).clamp(0.0, 1.0)
    }

    /// Scan large content by processing it in chunks with overlap. This avoids excessive memory use
    /// and handles matches that span chunk boundaries.
    pub async fn scan_content_chunked(
        &self,
        content: &str,
        context: Option<&str>,
        chunk_size: usize,
    ) -> Result<ScanResult> {
        if chunk_size == 0 {
            return self.scan_content(content, context).await;
        }

        let start_time = std::time::Instant::now();
        let overlap: usize = (chunk_size / 2).clamp(1, 64); // dynamic rolling context between chunks based on chunk size
        let mut detections: Vec<SensitiveDataDetection> = Vec::new();
        let mut data_classifications: HashSet<DataClassification> = HashSet::new();
        let mut seen: HashSet<(usize, usize, String)> = HashSet::new();

        let mut offset = 0usize;
        let len = content.len();
        while offset < len {
            let desired_end = (offset + chunk_size).min(len);
            // Adjust start and end to char boundaries to avoid slicing panics
            let mut start_idx = offset;
            while start_idx > 0 && !content.is_char_boundary(start_idx) {
                start_idx -= 1;
            }
            let mut end_idx = desired_end;
            while end_idx > start_idx && !content.is_char_boundary(end_idx) {
                end_idx -= 1;
            }
            if end_idx <= start_idx {
                // nothing sensible to scan
                if desired_end == len {
                    break;
                }
                // Ensure forward progress if boundaries collapse
                offset = desired_end;
                continue;
            }
            // Expand scanning window by overlap on both sides to capture matches across boundaries
            let win_start = start_idx.saturating_sub(overlap);
            let win_end = (end_idx + overlap).min(len);
            let chunk = &content[win_start..win_end];
            let base_offset = win_start;

            // Run parallel scanners on this chunk
            let scanners_guard = self.content_scanners.read().await;
            let enabled_scanners: Vec<ContentScanner> =
                scanners_guard.iter().filter(|s| s.is_enabled).cloned().collect();
            drop(scanners_guard);

            let mut joinset: JoinSet<(ContentScanner, Vec<SensitiveDataDetection>)> =
                JoinSet::new();
            let chunk_owned = chunk.to_string();
            for scanner in enabled_scanners {
                let chunk_clone = chunk_owned.clone();
                let scanner_clone = scanner.clone();
                let context_clone = context.map(|s| s.to_string());
                joinset.spawn(async move {
                    let dets = PrivacyContentFilter::run_scanner_internal(
                        &scanner_clone,
                        &chunk_clone,
                        context_clone.as_deref(),
                    );
                    (scanner_clone, dets)
                });
            }

            while let Some(res) = joinset.join_next().await {
                if let Ok((scanner, dets)) = res {
                    for mut d in dets.into_iter() {
                        if d.confidence < scanner.confidence_threshold {
                            continue;
                        }
                        // Adjust positions to global indices
                        d.start_position += base_offset;
                        d.end_position += base_offset;
                        let key = (d.start_position, d.end_position, d.matched_text.clone());
                        if seen.insert(key) {
                            data_classifications.insert(scanner.data_classification.clone());
                            detections.push(d);
                        }
                    }
                }
            }

            if end_idx == len {
                break;
            }
            // move to next chunk with overlap (ensure forward progress)
            let mut next_offset = end_idx.saturating_sub(overlap);
            if next_offset <= offset {
                next_offset = offset.saturating_add(1);
            }
            offset = next_offset;
        }

        // Sort detections
        detections.sort_by_key(|d| d.start_position);

        // Classify overall content
        let overall_classification = self.classify_content(content, &detections).await?;
        data_classifications.insert(overall_classification);

        let processing_time = start_time.elapsed();
        if self.config.enable_audit_logging {
            let refs: HashSet<&DataClassification> = data_classifications.iter().collect();
            self.log_audit_event(AuditEventType::ContentScanned, None, None, &refs).await?;
        }

        let risk_score = self.calculate_risk_score(&detections);
        Ok(ScanResult {
            content: content.to_string(),
            detections,
            data_classifications: data_classifications.into_iter().collect(),
            overall_risk_score: risk_score,
            processing_time_ms: processing_time.as_millis() as u64,
            timestamp: Utc::now(),
        })
    }

    /// Streaming scanning interface: pass sequential chunks, we maintain a rolling overlap
    /// and de-duplicate detections across chunk boundaries.
    pub async fn scan_content_streaming<I>(
        &self,
        chunks: I,
        context: Option<&str>,
    ) -> Result<ScanResult>
    where
        I: IntoIterator<Item = String>,
    {
        let start_time = std::time::Instant::now();
        let overlap: usize = 32; // reasonable default overlap for streaming
        let mut tail = String::new();
        let mut processed_len: usize = 0;

        let mut detections: Vec<SensitiveDataDetection> = Vec::new();
        let mut data_classifications: HashSet<DataClassification> = HashSet::new();
        let mut seen: HashSet<(usize, usize, String)> = HashSet::new();

        for chunk in chunks.into_iter() {
            let combined = format!("{}{}", tail, chunk);
            let base_offset = processed_len.saturating_sub(tail.len());

            // Parallel scanners per chunk
            let scanners_guard = self.content_scanners.read().await;
            let enabled_scanners: Vec<ContentScanner> =
                scanners_guard.iter().filter(|s| s.is_enabled).cloned().collect();
            drop(scanners_guard);

            let mut joinset: JoinSet<(ContentScanner, Vec<SensitiveDataDetection>)> =
                JoinSet::new();
            for scanner in enabled_scanners {
                let combined_clone = combined.clone();
                let scanner_clone = scanner.clone();
                let context_clone = context.map(|s| s.to_string());
                joinset.spawn(async move {
                    let dets = PrivacyContentFilter::run_scanner_internal(
                        &scanner_clone,
                        &combined_clone,
                        context_clone.as_deref(),
                    );
                    (scanner_clone, dets)
                });
            }

            while let Some(res) = joinset.join_next().await {
                if let Ok((scanner, dets)) = res {
                    for mut d in dets.into_iter() {
                        if d.confidence < scanner.confidence_threshold {
                            continue;
                        }
                        // Map local position to global index
                        d.start_position += base_offset;
                        d.end_position += base_offset;
                        let key = (d.start_position, d.end_position, d.matched_text.clone());
                        if seen.insert(key) {
                            data_classifications.insert(scanner.data_classification.clone());
                            detections.push(d);
                        }
                    }
                }
            }

            // Update processed length and tail
            processed_len += chunk.len();
            if combined.len() >= overlap {
                let mut start = combined.len() - overlap;
                while start < combined.len() && !combined.is_char_boundary(start) {
                    start += 1;
                }
                if start > combined.len() {
                    start = combined.len();
                }
                tail = combined.get(start..).unwrap_or("").to_string();
            } else {
                tail = combined;
            }
        }

        // Sort detections and classify
        detections.sort_by_key(|d| d.start_position);
        let overall_classification = self.classify_content("", &detections).await?;
        data_classifications.insert(overall_classification);

        let processing_time = start_time.elapsed();
        if self.config.enable_audit_logging {
            let refs: HashSet<&DataClassification> = data_classifications.iter().collect();
            self.log_audit_event(AuditEventType::ContentScanned, None, None, &refs).await?;
        }

        let risk_score = self.calculate_risk_score(&detections);
        Ok(ScanResult {
            content: String::new(),
            detections,
            data_classifications: data_classifications.into_iter().collect(),
            overall_risk_score: risk_score,
            processing_time_ms: processing_time.as_millis() as u64,
            timestamp: Utc::now(),
        })
    }

    async fn should_apply_redaction(
        &self,
        _rule: &RedactionRule,
        _scan_result: &ScanResult,
        _user_context: Option<&UserContext>,
    ) -> Result<bool> {
        // Implement rule evaluation logic
        Ok(true) // Simplified for now
    }

    async fn apply_redaction_rule(
        &self,
        _content: &str,
        rule: &RedactionRule,
        detections: &[SensitiveDataDetection],
    ) -> Result<Vec<RedactionApplication>> {
        let mut applications = Vec::new();

        for detection in detections {
            let redacted_text = match &rule.redaction_method {
                RedactionMethod::FullRedaction => "[REDACTED]".to_string(),
                RedactionMethod::PartialRedaction => self.partial_redact(&detection.matched_text),
                RedactionMethod::TokenReplacement => {
                    format!("[{}]", detection.data_type.to_uppercase())
                }
                _ => "[REDACTED]".to_string(), // Default fallback
            };

            applications.push(RedactionApplication {
                rule_id: rule.id.clone(),
                start_position: detection.start_position,
                end_position: detection.end_position,
                original_text: detection.matched_text.clone(),
                redacted_text,
                redaction_method: rule.redaction_method.clone(),
                confidence: detection.confidence,
            });
        }

        Ok(applications)
    }

    fn partial_redact(&self, text: &str) -> String {
        // Redaction strategy:
        // - For very short strings (<=4), fully redact
        // - For email-like strings (contain '@'), keep first 2 and last 2 characters, redact the middle
        // - Otherwise, keep only the last 3 characters visible, redact the rest
        let char_count = text.chars().count();
        if char_count <= 4 {
            return "*".repeat(char_count);
        }

        if text.contains('@') {
            let first_part: String = text.chars().take(2).collect();
            let last_part: String =
                text.chars().rev().take(2).collect::<Vec<char>>().into_iter().rev().collect();
            let star_count = char_count.saturating_sub(6).max(1);
            let middle_stars = "*".repeat(star_count);
            format!("{}{}{}", first_part, middle_stars, last_part)
        } else {
            let last_part: String =
                text.chars().rev().take(3).collect::<Vec<char>>().into_iter().rev().collect();
            let stars = "*".repeat(char_count.saturating_sub(3));
            format!("{}{}", stars, last_part)
        }
    }

    async fn log_audit_event(
        &self,
        event_type: AuditEventType,
        user_id: Option<String>,
        session_id: Option<Uuid>,
        data_classifications: &HashSet<&DataClassification>,
    ) -> Result<()> {
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type,
            user_id,
            session_id,
            data_classification: data_classifications
                .iter()
                .next()
                .cloned()
                .cloned()
                .unwrap_or(DataClassification::Internal),
            action_taken: "Content processed by privacy filter".to_string(),
            content_hash: None,
            policy_id: None,
            compliance_tags: self
                .config
                .compliance_standards
                .iter()
                .map(|s| format!("{:?}", s))
                .collect(),
            severity: AuditSeverity::Info,
        };

        let mut audit = self.privacy_audit.write().await;
        audit.audit_entries.push(entry);

        // Cleanup old entries if needed
        audit.cleanup_old_entries();

        Ok(())
    }

    fn calculate_compliance_score(
        &self,
        _entries: &[&AuditEntry],
        _standard: &ComplianceStandard,
    ) -> f32 {
        // Simplified compliance score calculation
        0.85 // 85% compliance score
    }

    fn generate_compliance_recommendations(
        &self,
        _standard: &ComplianceStandard,
        _entries: &[&AuditEntry],
    ) -> Vec<String> {
        vec![
            "Consider implementing additional encryption for sensitive data".to_string(),
            "Review access controls for confidential information".to_string(),
            "Update data retention policies to align with compliance requirements".to_string(),
        ]
    }
}

// Supporting structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub content: String,
    pub detections: Vec<SensitiveDataDetection>,
    pub data_classifications: Vec<DataClassification>,
    pub overall_risk_score: f32,
    pub processing_time_ms: u64,
    pub timestamp: DateTime<Utc>,
}

impl Default for ScanResult {
    fn default() -> Self {
        Self {
            content: String::new(),
            detections: Vec::new(),
            data_classifications: vec![DataClassification::Public],
            overall_risk_score: 0.0,
            processing_time_ms: 0,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitiveDataDetection {
    pub detection_id: Uuid,
    pub scanner_id: String,
    pub data_type: String,
    pub start_position: usize,
    pub end_position: usize,
    pub matched_text: String,
    pub confidence: f32,
    pub context: Option<String>,
    pub sensitivity: SensitivityLevel,
}

#[derive(Debug, Clone)]
pub struct UserContext {
    pub user_id: String,
    pub roles: Vec<String>,
    pub clearance_level: String,
    pub location: Option<String>,
    pub time_zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub standard: ComplianceStandard,
    pub date_range: (DateTime<Utc>, DateTime<Utc>),
    pub total_events: usize,
    pub violations: usize,
    pub high_risk_events: usize,
    pub data_processed: usize,
    pub compliance_score: f32,
    pub recommendations: Vec<String>,
    pub generated_at: DateTime<Utc>,
}

// Implementation for helper structs

impl RedactionEngine {
    pub fn new() -> Self {
        Self {
            redaction_cache: RwLock::new(HashMap::new()),
            anonymization_mappings: RwLock::new(HashMap::new()),
            encryption_key: None,
        }
    }
}

impl Default for RedactionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RedactionEngine {
    pub async fn apply_redaction(
        &self,
        content: &str,
        redaction: &RedactionApplication,
    ) -> Result<String> {
        let mut result = content.to_string();

        // Apply redaction at specified position
        if redaction.end_position <= content.len() {
            result.replace_range(
                redaction.start_position..redaction.end_position,
                &redaction.redacted_text,
            );
        }

        Ok(result)
    }
}

impl DataClassifier {
    pub fn new() -> Self {
        Self {
            classification_models: RwLock::new(HashMap::new()),
            entity_extractors: RwLock::new(Vec::new()),
            context_analyzers: RwLock::new(Vec::new()),
        }
    }
}

impl Default for DataClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivacyAuditLog {
    pub fn new() -> Self {
        Self {
            audit_entries: Vec::new(),
            retention_period: Duration::days(365), // 1 year default
            encryption_enabled: false,
        }
    }
}

impl Default for PrivacyAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivacyAuditLog {
    pub fn cleanup_old_entries(&mut self) {
        let cutoff_date = Utc::now() - self.retention_period;
        self.audit_entries.retain(|entry| entry.timestamp > cutoff_date);
    }
}

#[async_trait]
impl Agent for PrivacyContentFilter {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Privacy Content Filter"
    }

    fn description(&self) -> &str {
        "Comprehensive privacy protection system with content scanning, data classification, redaction, and compliance monitoring"
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::SecurityAnalysis,
            AgentCapability::ContextManagement,
            AgentCapability::Custom("ContentScanning".to_string()),
            AgentCapability::Custom("DataClassification".to_string()),
            AgentCapability::Custom("ContentRedaction".to_string()),
            AgentCapability::Custom("ComplianceMonitoring".to_string()),
            AgentCapability::Custom("AuditLogging".to_string()),
        ]
    }

    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        let mut response = AgentResponse {
            request_id: request.id,
            agent_id: self.id.clone(),
            success: false,
            payload: serde_json::json!({}),
            artifacts: Vec::new(),
            next_actions: Vec::new(),
            metadata: HashMap::new(),
        };

        match request.request_type {
            AgentRequestType::Custom(ref custom_type) => match custom_type.as_str() {
                "ScanContent" => {
                    if let Some(content) = request.payload.get("content").and_then(|v| v.as_str()) {
                        match self.scan_content(content, None).await {
                            Ok(scan_result) => {
                                response.success = true;
                                response.payload = serde_json::to_value(scan_result)?;
                            }
                            Err(e) => {
                                response.payload = serde_json::json!({
                                    "error": e.to_string()
                                });
                            }
                        }
                    }
                }
                "RedactContent" => {
                    if let (Some(content), Some(policy_id)) = (
                        request.payload.get("content").and_then(|v| v.as_str()),
                        request.payload.get("policy_id").and_then(|v| v.as_str()),
                    ) {
                        match self.redact_content(content, policy_id, None).await {
                            Ok(redaction_result) => {
                                response.success = true;
                                response.payload = serde_json::to_value(redaction_result)?;
                            }
                            Err(e) => {
                                response.payload = serde_json::json!({
                                    "error": e.to_string()
                                });
                            }
                        }
                    }
                }
                _ => {
                    return Err(anyhow!("Unknown privacy filter request: {}", custom_type));
                }
            },
            _ => {
                return Err(anyhow!(
                    "Privacy Content Filter cannot handle request type: {:?}",
                    request.request_type
                ));
            }
        }

        Ok(response)
    }

    fn can_handle(&self, request_type: &AgentRequestType) -> bool {
        matches!(request_type,
            AgentRequestType::Custom(custom_type)
            if custom_type == "ScanContent"
            || custom_type == "RedactContent"
            || custom_type == "GenerateComplianceReport"
            || custom_type == "CreatePrivacyPolicy"
        )
    }

    async fn status(&self) -> AgentStatus {
        let audit = self.privacy_audit.read().await;
        let scanners = self.content_scanners.read().await;
        let policies = self.privacy_policies.read().await;

        AgentStatus {
            is_healthy: self.is_initialized,
            is_busy: false, // Would track active scans in production
            last_activity: Utc::now(),
            current_task: Some(format!(
                "Monitoring privacy with {} scanners, {} policies, {} audit entries",
                scanners.len(),
                policies.len(),
                audit.audit_entries.len()
            )),
            error_message: None,
        }
    }

    async fn initialize(&mut self, _config: AgentConfig) -> Result<()> {
        // Initialize default scanners and policies
        self.initialize_default_scanners().await?;
        self.initialize_default_policies().await?;

        self.is_initialized = true;
        tracing::info!("Privacy Content Filter initialized");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Save audit logs and cleanup
        self.is_initialized = false;
        tracing::info!("Privacy Content Filter shut down");
        Ok(())
    }
}

impl PrivacyContentFilter {
    async fn initialize_default_scanners(&self) -> Result<()> {
        // Email scanner
        let email_scanner = ContentScanner {
            id: "email-scanner".to_string(),
            name: "Email Address Scanner".to_string(),
            scanner_type: ScannerType::RegexPattern,
            patterns: vec![ScanPattern {
                pattern: r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b".to_string(),
                pattern_type: PatternType::Regex,
                sensitivity: SensitivityLevel::Medium,
                context_requirements: Vec::new(),
                false_positive_filters: Vec::new(),
            }],
            confidence_threshold: 0.8,
            data_classification: DataClassification::PersonalData,
            is_enabled: true,
        };

        // Phone number scanner
        let phone_scanner = ContentScanner {
            id: "phone-scanner".to_string(),
            name: "Phone Number Scanner".to_string(),
            scanner_type: ScannerType::RegexPattern,
            patterns: vec![ScanPattern {
                pattern: r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b".to_string(),
                pattern_type: PatternType::Regex,
                sensitivity: SensitivityLevel::High,
                context_requirements: Vec::new(),
                false_positive_filters: Vec::new(),
            }],
            confidence_threshold: 0.7,
            data_classification: DataClassification::PersonalData,
            is_enabled: true,
        };

        // Credit card scanner (13-19 digits, allow spaces/dashes). Luhn validated during scanning.
        let cc_scanner = ContentScanner {
            id: "credit-card-scanner".to_string(),
            name: "Credit Card Scanner".to_string(),
            scanner_type: ScannerType::RegexPattern,
            patterns: vec![ScanPattern {
                pattern: r"\b(?:\d[ -]?){13,19}\b".to_string(),
                pattern_type: PatternType::Regex,
                sensitivity: SensitivityLevel::Critical,
                context_requirements: Vec::new(),
                false_positive_filters: vec![
                    // Common dummy/test numbers to reduce false positives
                    r"^0+$".to_string(),
                    r"^(?:1234[ -]?){3}1234$".to_string(),
                ],
            }],
            confidence_threshold: 0.9,
            data_classification: DataClassification::FinancialData,
            is_enabled: true,
        };

        // GitHub token scanner
        let github_token_scanner = ContentScanner {
            id: "github-token-scanner".to_string(),
            name: "GitHub Token Scanner".to_string(),
            scanner_type: ScannerType::RegexPattern,
            patterns: vec![ScanPattern {
                pattern: r"\bghp_[A-Za-z0-9]{36}\b".to_string(),
                pattern_type: PatternType::Regex,
                sensitivity: SensitivityLevel::High,
                context_requirements: Vec::new(),
                false_positive_filters: Vec::new(),
            }],
            confidence_threshold: 0.9,
            data_classification: DataClassification::Restricted,
            is_enabled: true,
        };

        // AWS Access Key ID scanner
        let aws_akid_scanner = ContentScanner {
            id: "aws-akid-scanner".to_string(),
            name: "AWS Access Key ID Scanner".to_string(),
            scanner_type: ScannerType::RegexPattern,
            patterns: vec![ScanPattern {
                pattern: r"\bAKIA[0-9A-Z]{16}\b".to_string(),
                pattern_type: PatternType::Regex,
                sensitivity: SensitivityLevel::High,
                context_requirements: Vec::new(),
                false_positive_filters: Vec::new(),
            }],
            confidence_threshold: 0.9,
            data_classification: DataClassification::Restricted,
            is_enabled: true,
        };

        // JWT scanner
        let jwt_scanner = ContentScanner {
            id: "jwt-scanner".to_string(),
            name: "JWT Scanner".to_string(),
            scanner_type: ScannerType::RegexPattern,
            patterns: vec![ScanPattern {
                pattern: r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b".to_string(),
                pattern_type: PatternType::Regex,
                sensitivity: SensitivityLevel::High,
                context_requirements: Vec::new(),
                false_positive_filters: Vec::new(),
            }],
            confidence_threshold: 0.9,
            data_classification: DataClassification::Confidential,
            is_enabled: true,
        };

        self.add_content_scanner(email_scanner).await?;
        self.add_content_scanner(phone_scanner).await?;
        self.add_content_scanner(cc_scanner).await?;
        self.add_content_scanner(github_token_scanner).await?;
        self.add_content_scanner(aws_akid_scanner).await?;
        self.add_content_scanner(jwt_scanner).await?;

        Ok(())
    }

    async fn initialize_default_policies(&self) -> Result<()> {
        // GDPR compliance policy
        let gdpr_policy = PrivacyPolicy {
            id: "gdpr-policy".to_string(),
            name: "GDPR Compliance Policy".to_string(),
            description: "General Data Protection Regulation compliance policy".to_string(),
            data_classifications: vec![
                DataClassification::PersonalData,
                DataClassification::Confidential,
                DataClassification::Restricted,
            ],
            redaction_rules: vec![RedactionRule {
                id: "personal-data-redaction".to_string(),
                name: "Personal Data Redaction".to_string(),
                data_classification: DataClassification::PersonalData,
                redaction_method: RedactionMethod::PartialRedaction,
                preserve_format: true,
                replacement_pattern: None,
                conditions: Vec::new(),
            }],
            retention_policies: HashMap::from([
                ("personal_data".to_string(), Duration::days(730)), // 2 years
                ("financial_data".to_string(), Duration::days(2555)), // 7 years
            ]),
            access_controls: Vec::new(),
            compliance_standards: vec![ComplianceStandard::GDPR],
            created_at: Utc::now(),
            last_updated: Utc::now(),
            version: "1.0".to_string(),
            is_active: true,
        };

        self.create_privacy_policy(gdpr_policy).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_privacy_filter_creation() {
        let filter = PrivacyContentFilter::new();
        assert_eq!(filter.id(), "privacy-content-filter");
        assert_eq!(filter.name(), "Privacy Content Filter");
    }

    #[tokio::test]
    async fn test_content_scanning() {
        let filter = PrivacyContentFilter::new();

        // Initialize with default scanners
        let mut filter_mut = filter;
        filter_mut.initialize(AgentConfig::default()).await.unwrap();

        // Test email detection
        let result = filter_mut.scan_content("Contact us at test@example.com", None).await.unwrap();
        assert!(!result.detections.is_empty());
        assert!(result.data_classifications.contains(&DataClassification::PersonalData));
    }

    #[tokio::test]
    async fn test_luhn_credit_card_detection() {
        let mut filter = PrivacyContentFilter::new();
        filter.initialize(AgentConfig::default()).await.unwrap();
        // Valid test Visa number 4111 1111 1111 1111
        let res_valid = filter.scan_content("card 4111-1111-1111-1111", None).await.unwrap();
        assert!(res_valid.detections.iter().any(|d| d.scanner_id == "credit-card-scanner"));
        // Invalid (fails Luhn)
        let res_invalid = filter.scan_content("card 4111-1111-1111-1112", None).await.unwrap();
        assert!(!res_invalid.detections.iter().any(|d| d.scanner_id == "credit-card-scanner"));
    }

    #[tokio::test]
    async fn test_token_scanners() {
        let mut filter = PrivacyContentFilter::new();
        filter.initialize(AgentConfig::default()).await.unwrap();
        let res = filter
            .scan_content("token ghp_abcdefghijklmnopqrstuvwxyz0123456789", None)
            .await
            .unwrap();
        assert!(res.detections.iter().any(|d| d.scanner_id == "github-token-scanner"));
        let res2 = filter.scan_content("AWS key AKIAABCDEFGHIJKLMNOP", None).await.unwrap();
        assert!(res2.detections.iter().any(|d| d.scanner_id == "aws-akid-scanner"));
        let res3 = filter.scan_content("Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NSJ9.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c", None).await.unwrap();
        assert!(res3.detections.iter().any(|d| d.scanner_id == "jwt-scanner"));
    }

    #[tokio::test]
    async fn test_chunked_scanning_boundary() {
        let mut filter = PrivacyContentFilter::new();
        filter.initialize(AgentConfig::default()).await.unwrap();
        // Construct a  card split across boundary
        let content = format!("{}{}", "4111-1111-1111-", "1111 end");
        let res = filter.scan_content_chunked(&content, None, 10).await.unwrap();
        assert!(res.detections.iter().any(|d| d.scanner_id == "credit-card-scanner"));
    }

    #[test]
    fn test_partial_redaction() {
        let filter = PrivacyContentFilter::new();
        assert_eq!(filter.partial_redact("test@example.com"), "te**********om");
        assert_eq!(filter.partial_redact("short"), "**ort");
        assert_eq!(filter.partial_redact("a"), "*");
    }
}
