# Security Lens Feature Implementation

## Overview

The Security Lens feature provides real-time risk assessment for terminal commands in the OpenAgent Terminal. It analyzes commands for potential security risks and provides warnings, mitigation suggestions, and contextual documentation to help users make informed decisions about command execution.

## Features Implemented

### 1. Core Risk Analysis
- **Risk Levels**: Safe, Caution, Warning, Critical
- **Pattern Matching**: Uses regex patterns to detect dangerous commands
- **Command Categories**: System destruction, remote execution, permission exposure, cloud operations, database operations, etc.
- **Risk Factors**: Detailed breakdown of why a command is considered risky

### 2. Platform Awareness
- **Platform Detection**: Automatically detects Linux, macOS, Windows, or Unknown
- **Platform-Specific Patterns**: Each platform has tailored security patterns
  - Linux: systemd masking, iptables flush, kernel module loading
  - macOS: SIP disable, Gatekeeper disable, keychain manipulation
  - Windows: PowerShell execution policy, Windows Defender disable, registry manipulation
- **Configurable Platform Groups**: Enable/disable platform-specific pattern groups via configuration

### 3. Rate Limiting
- **Detection Tracking**: Records security detections with timestamps
- **Rate Limiting**: Limits frequent high-risk detections to prevent spam
- **Configurable Windows**: Adjustable time windows and detection thresholds
- **Automatic Cleanup**: Removes old detection records automatically

### 4. Paste Event Gating
- **Paste Analysis**: Analyzes pasted content for security risks
- **Multi-line Support**: Evaluates multiple commands in pasted content
- **Confirmation Requirements**: Prompts user confirmation for risky paste content
- **Configurable Gating**: Can be enabled/disabled via configuration

### 5. Enhanced Mitigation Support
- **Context-Aware Mitigations**: Provides specific mitigation suggestions based on risk factors
- **Documentation Links**: Links to relevant security documentation and guides
- **Configurable Base URL**: Allow customization of documentation base URL
- **Rich Risk Display**: Formatted output with icons, colors, and structured information

### 6. Comprehensive Logging
- **Detection Logging**: Logs significant security detections with context
- **Debug Information**: Detailed debug logs for analysis and troubleshooting
- **Rate Limiting Logs**: Logs when rate limiting is triggered
- **Unique Detection IDs**: Each detection gets a unique identifier for tracking

## Configuration

The Security Lens can be configured through the `SecurityPolicy` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPolicy {
    pub enabled: bool,                                    // Enable/disable security lens
    pub block_critical: bool,                             // Block critical commands
    pub require_confirmation: HashMap<RiskLevel, bool>,   // Confirmation requirements by risk level
    pub require_reason: HashMap<RiskLevel, bool>,         // Reason requirements by risk level
    pub custom_patterns: Vec<CustomPattern>,              // Custom security patterns
    pub platform_groups: Vec<PlatformPatternGroup>,      // Platform-specific pattern groups
    pub gate_paste_events: bool,                          // Enable paste event gating
    pub rate_limit: RateLimitConfig,                      // Rate limiting configuration
    pub docs_base_url: String,                           // Base URL for documentation links
}
```

### Rate Limiting Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitConfig {
    pub max_detections: u32,        // Maximum detections per window
    pub window_seconds: u64,        // Time window in seconds
    pub enabled: bool,              // Enable rate limiting
}
```

## API Methods

### Core Analysis
- `analyze_command(&mut self, command: &str) -> CommandRisk` - Analyze a single command
- `analyze_paste_content(&mut self, content: &str) -> Option<CommandRisk>` - Analyze pasted content

### Utility Methods
- `should_block(&self, risk: &CommandRisk) -> bool` - Check if command should be blocked
- `format_risk_display(&self, risk: &CommandRisk) -> String` - Format risk for display

### Risk Information
```rust
pub struct CommandRisk {
    pub level: RiskLevel,                    // Risk level assessment
    pub factors: Vec<RiskFactor>,           // Contributing risk factors
    pub explanation: String,                 // Human-readable explanation
    pub mitigations: Vec<String>,           // Mitigation suggestions
    pub mitigation_links: Vec<MitigationLink>, // Documentation links
    pub requires_confirmation: bool,         // Whether user confirmation is needed
    pub platform_specific: bool,            // Whether detection is platform-specific
    pub detection_id: String,               // Unique detection identifier
}
```

## Built-in Patterns

### Critical Patterns
- `rm -rf /` - Filesystem destruction
- `dd if=/dev/zero of=/dev/sda` - Disk overwrite
- `:(){ :|:& };:` - Fork bomb
- Production namespace deletion in Kubernetes

### Warning Patterns
- `curl ... | sh` - Remote script execution
- `chmod 777` - Permission exposure
- AWS/GCP resource deletion
- Database DROP operations
- Infrastructure destruction (terraform destroy)

### Caution Patterns
- Recursive file operations
- Git hard reset
- Docker system cleanup

## Testing

The implementation includes comprehensive tests covering:
- Basic pattern matching for all risk levels
- Platform-specific pattern detection
- Rate limiting functionality
- Paste content analysis
- Custom pattern support
- Policy configuration validation
- Mitigation link generation
- Risk display formatting

## Integration Points

The Security Lens integrates with several parts of the terminal:

1. **Command Execution**: Commands are analyzed before execution
2. **Paste Events**: Clipboard content is analyzed when pasted
3. **Configuration System**: Policy settings are configurable via TOML
4. **UI Components**: Risk information is displayed in confirmation overlays
5. **Logging System**: Security events are logged for audit and analysis

## Next Steps

To complete the Security Lens integration:

1. **UI Integration**: Implement confirmation overlay UI components
2. **Event Handling**: Hook security analysis into command execution flow
3. **Configuration Loading**: Ensure security policy is loaded from user config
4. **Documentation**: Create user-facing documentation for security features
5. **Telemetry**: Add optional telemetry for security event aggregation

## Security Considerations

The Security Lens is designed with the following security principles:

- **Fail Open**: If analysis fails, commands are allowed (unless configured otherwise)
- **User Control**: Users can disable or customize all security features
- **Transparency**: All risk assessments are explained to the user
- **Privacy**: No command content is transmitted externally
- **Configurability**: All patterns and policies are user-configurable
