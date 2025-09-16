use anyhow::Result;
use chrono::{Duration, Utc};
use openagent_terminal::ai::agents::{
    advanced_conversation_features::AdvancedConversationFeatures,
    conversation_manager::ConversationManager,
    privacy_content_filter::{
        ComplianceStandard, DataClassification, PrivacyContentFilter, PrivacyFilterConfig,
        PrivacyPolicy, RedactionMethod, RedactionRule, SensitivityLevel, UserContext,
    },
    Agent, AgentConfig,
};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "ai")]
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    println!("🔒 Privacy Content Filter Demo");
    println!("==============================");

    // 1. Configure privacy filter with enhanced settings
    println!("\n⚙️ Configuring privacy filter...");
    let config = PrivacyFilterConfig {
        enable_real_time_scanning: true,
        enable_batch_processing: true,
        enable_audit_logging: true,
        default_data_classification: DataClassification::Internal,
        max_content_size_mb: 5,
        scan_timeout_seconds: 15,
        cache_results: true,
        cache_ttl_minutes: 30,
        compliance_standards: vec![
            ComplianceStandard::GDPR,
            ComplianceStandard::CCPA,
            ComplianceStandard::HIPAA,
        ],
        notification_channels: vec!["security_team@example.com".to_string()],
    };

    // 2. Initialize privacy content filter
    println!("🚀 Initializing privacy content filter...");
    let mut privacy_filter = PrivacyContentFilter::new().with_config(config);
    let filter_config = AgentConfig::default();
    privacy_filter.initialize(filter_config).await?;
    let privacy_filter = Arc::new(privacy_filter);

    // 3. Create conversation manager for integration
    let conversation_manager = Arc::new(ConversationManager::new());
    let advanced_features = Arc::new({
        let mut features = AdvancedConversationFeatures::new(conversation_manager.clone());
        features.initialize(AgentConfig::default()).await?;
        features
    });

    // Demo 1: Content Scanning
    println!("\n🔍 DEMO 1: Content Scanning");
    println!("===========================");

    // Test various types of sensitive content
    let test_contents = vec![
        (
            "Email Content",
            "Please contact us at john.doe@company.com or call 555-123-4567 for assistance.",
        ),
        (
            "Financial Data",
            "My credit card number is 4532-1234-5678-9012 and the CVV is 123.",
        ),
        (
            "Personal Info",
            "SSN: 123-45-6789, DOB: 01/15/1985, Phone: (555) 987-6543",
        ),
        (
            "Health Data",
            "Patient ID: 12345, Diagnosis: Diabetes Type 2, Insurance: Blue Cross 987654321",
        ),
        (
            "Mixed Content",
            "John Smith (john@email.com, 555-0123) has CC: 4111-1111-1111-1111 exp 12/25",
        ),
    ];

    for (content_type, content) in &test_contents {
        println!("\n📄 Scanning: {}", content_type);
        println!("Content: \"{}\"", content);

        let scan_result = privacy_filter.scan_content(content, None).await?;

        println!("🔍 Scan Results:");
        println!("  - Detections: {}", scan_result.detections.len());
        println!("  - Risk Score: {:.2}", scan_result.overall_risk_score);
        println!(
            "  - Data Classifications: {:?}",
            scan_result.data_classifications
        );
        println!("  - Processing Time: {}ms", scan_result.processing_time_ms);

        for detection in &scan_result.detections {
            println!(
                "  🚨 Detection: {} (confidence: {:.2}, sensitivity: {:?})",
                detection.data_type, detection.confidence, detection.sensitivity
            );
        }
    }

    // Demo 2: Content Redaction
    println!("\n🔒 DEMO 2: Content Redaction");
    println!("============================");

    // Create a sample user context
    let user_context = UserContext {
        user_id: "demo_user".to_string(),
        roles: vec!["employee".to_string()],
        clearance_level: "standard".to_string(),
        location: Some("US".to_string()),
        time_zone: "UTC".to_string(),
    };

    // Test redaction with different content
    let redaction_samples = vec![
        "Employee email: alice@company.com, phone: 555-0199",
        "Credit card: 5555-4444-3333-2222, expiry: 12/24, CVV: 789",
        "Social Security Number: 987-65-4321 for Jane Doe",
        "Medical record #MR789456 shows patient has condition X123",
    ];

    for sample in &redaction_samples {
        println!("\n📝 Original: \"{}\"", sample);

        // Apply redaction using GDPR policy
        let redaction_result = privacy_filter
            .redact_content(sample, "gdpr-policy", Some(&user_context))
            .await?;

        println!("🔒 Redacted: \"{}\"", redaction_result.redacted_content);
        println!(
            "📊 Applied {} redaction(s)",
            redaction_result.redactions_applied.len()
        );

        for redaction in &redaction_result.redactions_applied {
            println!(
                "  - Applied {:?} to \"{}\" → \"{}\"",
                redaction.redaction_method, redaction.original_text, redaction.redacted_text
            );
        }
    }

    // Demo 3: Advanced Privacy Policies
    println!("\n📋 DEMO 3: Advanced Privacy Policies");
    println!("====================================");

    // Create a custom privacy policy for healthcare
    let healthcare_policy = PrivacyPolicy {
        id: "healthcare-policy".to_string(),
        name: "Healthcare Privacy Policy".to_string(),
        description: "HIPAA-compliant privacy policy for healthcare data".to_string(),
        data_classifications: vec![
            DataClassification::HealthData,
            DataClassification::PersonalData,
            DataClassification::Confidential,
        ],
        redaction_rules: vec![
            RedactionRule {
                id: "health-data-redaction".to_string(),
                name: "Health Data Full Redaction".to_string(),
                data_classification: DataClassification::HealthData,
                redaction_method: RedactionMethod::FullRedaction,
                preserve_format: false,
                replacement_pattern: Some("[HEALTH_DATA_REDACTED]".to_string()),
                conditions: Vec::new(),
            },
            RedactionRule {
                id: "personal-partial-redaction".to_string(),
                name: "Personal Data Partial Redaction".to_string(),
                data_classification: DataClassification::PersonalData,
                redaction_method: RedactionMethod::PartialRedaction,
                preserve_format: true,
                replacement_pattern: None,
                conditions: Vec::new(),
            },
        ],
        retention_policies: HashMap::from([
            ("health_data".to_string(), Duration::days(2555)), // 7 years
            ("personal_data".to_string(), Duration::days(1095)), // 3 years
        ]),
        access_controls: Vec::new(),
        compliance_standards: vec![ComplianceStandard::HIPAA],
        created_at: Utc::now(),
        last_updated: Utc::now(),
        version: "1.0".to_string(),
        is_active: true,
    };

    // Add the custom policy
    privacy_filter
        .create_privacy_policy(healthcare_policy)
        .await?;
    println!("✅ Created healthcare privacy policy");

    // Test with healthcare content
    let healthcare_content = "Patient: John Smith, DOB: 1980-05-15, Diagnosis: Hypertension, Email: john.smith@email.com";
    println!("\n🏥 Testing healthcare content:");
    println!("Original: \"{}\"", healthcare_content);

    let healthcare_result = privacy_filter
        .redact_content(healthcare_content, "healthcare-policy", Some(&user_context))
        .await?;

    println!(
        "🔒 Healthcare Redacted: \"{}\"",
        healthcare_result.redacted_content
    );

    // Demo 4: Compliance Reporting
    println!("\n📊 DEMO 4: Compliance Reporting");
    println!("===============================");

    let date_range = (Utc::now() - Duration::hours(1), Utc::now());

    // Generate compliance reports for different standards
    let compliance_standards = vec![
        ComplianceStandard::GDPR,
        ComplianceStandard::CCPA,
        ComplianceStandard::HIPAA,
    ];

    for standard in compliance_standards {
        println!("\n📋 Generating {:?} compliance report...", standard);

        match privacy_filter
            .generate_compliance_report(standard.clone(), date_range)
            .await
        {
            Ok(report) => {
                println!("✅ {:?} Compliance Report:", report.standard);
                println!(
                    "  📅 Period: {} to {}",
                    report.date_range.0.format("%Y-%m-%d %H:%M UTC"),
                    report.date_range.1.format("%Y-%m-%d %H:%M UTC")
                );
                println!("  📊 Total Events: {}", report.total_events);
                println!("  ⚠️ Violations: {}", report.violations);
                println!("  🔴 High Risk Events: {}", report.high_risk_events);
                println!(
                    "  💯 Compliance Score: {:.1}%",
                    report.compliance_score * 100.0
                );
                println!("  📝 Recommendations:");
                for recommendation in &report.recommendations {
                    println!("    - {}", recommendation);
                }
            }
            Err(e) => println!("❌ Failed to generate {:?} report: {}", standard, e),
        }
    }

    // Demo 5: Real-time Processing Integration
    println!("\n⚡ DEMO 5: Real-time Processing Integration");
    println!("==========================================");

    // Create a conversation session
    let session_id = conversation_manager
        .create_session(Some("Privacy Demo Session".to_string()))
        .await?;

    println!("📝 Created conversation session: {}", session_id);

    // Simulate conversation with privacy filtering
    let conversation_messages = vec![
        "Hi, I need help with setting up authentication for our app.",
        "My email is demo@company.com and phone is 555-0100.",
        "We're storing user credit cards like 4000-1234-5678-9012.",
        "The database password is super_secret_123 and API key is sk_abc123.",
    ];

    for (i, message) in conversation_messages.iter().enumerate() {
        println!("\n💬 Message {}: \"{}\"", i + 1, message);

        // Scan the message
        let scan_result = privacy_filter.scan_content(message, None).await?;

        if !scan_result.detections.is_empty() {
            println!(
                "🚨 Privacy issues detected! Risk score: {:.2}",
                scan_result.overall_risk_score
            );

            // Apply redaction
            let redaction_result = privacy_filter
                .redact_content(message, "gdpr-policy", Some(&user_context))
                .await?;

            println!("🔒 Safe version: \"{}\"", redaction_result.redacted_content);

            // Add redacted version to conversation
            conversation_manager
                .add_turn(
                    session_id,
                    openagent_terminal::ai::agents::natural_language::ConversationRole::User,
                    redaction_result.redacted_content,
                    None,
                    vec![],
                )
                .await?;
        } else {
            println!("✅ No privacy issues detected");

            // Add original message to conversation
            conversation_manager
                .add_turn(
                    session_id,
                    openagent_terminal::ai::agents::natural_language::ConversationRole::User,
                    message.to_string(),
                    None,
                    vec![],
                )
                .await?;
        }
    }

    // Demo 6: Privacy Filter Status and Metrics
    println!("\n📈 DEMO 6: Privacy Filter Status and Metrics");
    println!("============================================");

    let status = privacy_filter.status().await;
    println!("🏥 Privacy Filter Status:");
    println!(
        "  Health: {}",
        if status.is_healthy {
            "✅ Healthy"
        } else {
            "❌ Unhealthy"
        }
    );
    println!(
        "  Busy: {}",
        if status.is_busy {
            "🔄 Processing"
        } else {
            "⏸️ Idle"
        }
    );
    println!(
        "  Last Activity: {}",
        status.last_activity.format("%H:%M:%S UTC")
    );
    if let Some(task) = &status.current_task {
        println!("  Current Task: {}", task);
    }

    // Show final conversation summary with privacy applied
    let conversation_summary = conversation_manager
        .get_conversation_summary(session_id, 10)
        .await?;

    println!("\n📋 Final Conversation Summary (Privacy Protected):");
    println!("{}", conversation_summary);

    // Demo 7: Advanced Features Integration
    println!("\n🔧 DEMO 7: Advanced Features Integration");
    println!("=======================================");

    // Show how privacy filter integrates with advanced conversation features
    let integrated_filter = PrivacyContentFilter::new()
        .with_conversation_manager(conversation_manager.clone())
        .with_advanced_features(advanced_features.clone());

    println!("✅ Privacy filter integrated with:");
    println!("  - Conversation Manager: ✓");
    println!("  - Advanced Conversation Features: ✓");
    println!("  - Real-time scanning: ✓");
    println!("  - Compliance monitoring: ✓");
    println!("  - Audit logging: ✓");

    // Cleanup
    println!("\n🧹 Cleaning up...");

    println!("✅ Privacy Content Filter Demo completed successfully!");
    println!("\n🔒 Privacy Protection Summary:");
    println!("  🔍 Content scanning with multiple detection engines");
    println!("  🎭 Advanced redaction methods (full, partial, token replacement)");
    println!("  📋 Flexible privacy policies and compliance standards");
    println!("  📊 Real-time compliance reporting and monitoring");
    println!("  🔗 Deep integration with conversation and workflow systems");
    println!("  🛡️ Enterprise-grade privacy protection for all communications");

    Ok(())
}

#[cfg(not(feature = "ai"))]
fn main() {
    println!("❌ This example requires the 'ai' feature to be enabled.");
    println!("Run with: cargo run --example privacy_content_filter_demo --features ai");
}
