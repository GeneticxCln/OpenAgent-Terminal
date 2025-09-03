# Security Lens Organization Policy Bundles

The Security Lens in OpenAgent Terminal provides comprehensive command risk assessment with support for organization-specific security policies. This document explains how to configure and deploy Security Lens policies for enterprise environments.

## Table of Contents

- [Overview](#overview)
- [Policy Configuration](#policy-configuration)
- [Organization Policy Bundles](#organization-policy-bundles)
- [Custom Pattern Development](#custom-pattern-development)
- [Platform-Specific Configurations](#platform-specific-configurations)
- [Deployment Guide](#deployment-guide)
- [Compliance Considerations](#compliance-considerations)

## Overview

Security Lens analyzes terminal commands in real-time to detect potential security risks across multiple categories:

### Built-in Risk Categories

- **Filesystem**: Mass deletion, permission exposure, system ownership changes
- **Networking**: Remote execution, firewall manipulation, network scanning
- **Package Managers**: Global installs, untrusted sources, auto-removal
- **Container/Kubernetes**: Privileged operations, production deletions
- **Cloud CLIs**: AWS/GCP/Azure resource deletion and manipulation
- **Database**: DROP operations, data wipes, privilege management
- **Infrastructure as Code**: Terraform/Pulumi destroy operations
- **Version Control**: Force pushes, destructive resets, VCS removal
- **System Services**: Critical service manipulation
- **Cryptography**: Weak key generation, certificate operations
- **Environment**: History manipulation, sensitive data exposure

### Risk Levels

- **Safe**: No detected risks
- **Caution**: Minor risks that may require awareness
- **Warning**: Moderate risks requiring confirmation
- **Critical**: High risks that can cause severe damage

## Policy Configuration

### Basic Configuration

```toml
[security_lens]
enabled = true
block_critical = false
docs_base_url = "https://your-org.com/security"
gate_paste_events = true

[security_lens.require_confirmation]
Warning = true
Critical = true

[security_lens.require_reason]
Critical = true

[security_lens.rate_limit]
max_detections = 5
window_seconds = 60
enabled = true
```

### Configuration Options

| Option | Type | Description |
|--------|------|-------------|
| `enabled` | boolean | Enable/disable Security Lens entirely |
| `block_critical` | boolean | Completely block critical-level commands |
| `docs_base_url` | string | Base URL for mitigation documentation links |
| `gate_paste_events` | boolean | Analyze pasted content for security risks |
| `require_confirmation` | map | Which risk levels require user confirmation |
| `require_reason` | map | Which risk levels require reason text |
| `rate_limit` | object | Rate limiting configuration |

## Organization Policy Bundles

We provide several pre-configured policy bundles for common industry requirements:

### Available Bundles

1. **General Enterprise** (`examples/org-security-policy.toml`)
   - Standard corporate security patterns
   - CI/CD protection
   - Cloud resource safeguards

2. **Financial Services** (`examples/fintech-security-policy.toml`)
   - PCI DSS compliance patterns
   - SOX audit trail protection
   - Payment system safeguards
   - Trading system protection

3. **Healthcare** (`examples/healthcare-security-policy.toml`)
   - HIPAA compliance patterns
   - PHI protection
   - Medical device security
   - EHR/EMR system protection

### Using a Policy Bundle

1. Copy the appropriate bundle to your configuration directory:
   ```bash
   cp examples/fintech-security-policy.toml ~/.config/openagent/security.toml
   ```

2. Customize the patterns for your specific environment:
   - Update `docs_base_url` to your internal documentation
   - Modify patterns to match your service names
   - Adjust risk levels based on your requirements

3. Restart OpenAgent Terminal to load the new policy

## Custom Pattern Development

### Pattern Structure

```toml
[[security_lens.custom_patterns]]
pattern = "(?i)your-service\\s+(stop|restart)"
risk_level = "Warning"
message = "Operation affecting your critical service"
```

### Pattern Development Guidelines

1. **Use Extended Regular Expressions (ERE)**
   - Case-insensitive matching: `(?i)`
   - Word boundaries: `\\b`
   - Character classes: `[a-z]`, `\\s+`, `\\d+`

2. **Risk Level Assignment**
   - **Critical**: System destruction, data loss, security bypass
   - **Warning**: Resource deletion, configuration changes
   - **Caution**: Potentially risky but common operations

3. **Message Guidelines**
   - Be specific about the risk
   - Mention compliance implications if applicable
   - Suggest the impact of the operation

### Example Patterns

```toml
# Detect production database connections
[[security_lens.custom_patterns]]
pattern = "(?i)psql\\s+.*-h\\s+prod-db"
risk_level = "Warning"
message = "Direct connection to production database"

# Corporate VPN manipulation
[[security_lens.custom_patterns]]
pattern = "(?i)systemctl\\s+(stop|disable)\\s+corp-vpn"
risk_level = "Warning"
message = "Disconnecting from corporate VPN"

# Deployment tool usage
[[security_lens.custom_patterns]]
pattern = "(?i)deploy-tool\\s+--env=production"
risk_level = "Warning"
message = "Production deployment operation"
```

## Platform-Specific Configurations

Platform-specific patterns allow different rules for Linux, macOS, and Windows:

```toml
[[security_lens.platform_groups]]
enabled = true
platform = "Linux"
patterns = [
  { pattern = "corp-firewall\\s+disable", risk_level = "Warning", message = "Disabling corporate firewall" }
]

[[security_lens.platform_groups]]
enabled = true
platform = "MacOS"
patterns = [
  { pattern = "jamf\\s+remove", risk_level = "Warning", message = "Removing MDM management" }
]

[[security_lens.platform_groups]]
enabled = true
platform = "Windows"
patterns = [
  { pattern = "Disable-CorporateEDR", risk_level = "Warning", message = "Disabling EDR protection" }
]
```

## Deployment Guide

### 1. Assessment Phase

Before deploying Security Lens policies:

1. **Audit Current Usage**
   ```bash
   # Review command history for patterns
   history | grep -E "(rm|delete|drop|stop)" | head -20
   ```

2. **Identify Critical Operations**
   - Production deployments
   - Database operations
   - Infrastructure changes
   - Service management

3. **Review Existing Toolchains**
   - CI/CD pipelines
   - Deployment scripts
   - Administrative procedures

### 2. Configuration Development

1. **Start with Base Bundle**
   - Choose appropriate industry bundle
   - Copy to configuration directory

2. **Customize for Environment**
   - Update service names
   - Modify production identifiers
   - Adjust risk levels

3. **Test Configuration**
   ```bash
   # Test with known safe commands
   echo "ls -la" | openagent-terminal --test-security-lens
   
   # Test with risky patterns
   echo "rm -rf /" | openagent-terminal --test-security-lens
   ```

### 3. Rollout Strategy

1. **Pilot Deployment**
   - Deploy to development teams first
   - Gather feedback on false positives
   - Refine patterns based on usage

2. **Phased Production Rollout**
   - Start with monitoring mode (confirmations only)
   - Gradually enable blocking for critical operations
   - Monitor logs for pattern effectiveness

3. **Full Deployment**
   - Enable all protection levels
   - Provide training to users
   - Establish exception processes

### 4. Monitoring and Maintenance

```bash
# Monitor Security Lens activity
journalctl -u openagent-terminal | grep "Security Lens"

# Review detection statistics
openagent-terminal --security-stats

# Update patterns based on new threats
# Edit security configuration and reload
```

## Compliance Considerations

### Financial Services (PCI DSS, SOX)

- **Cardholder Data Environment**: Strict controls on CHD access
- **Audit Trail Protection**: Prevent log tampering
- **Change Management**: Require justification for production changes
- **Separation of Duties**: Different policies for different roles

### Healthcare (HIPAA, HITECH)

- **PHI Protection**: Prevent unauthorized PHI access/export
- **Audit Logging**: Comprehensive logging of PHI operations
- **Medical Device Security**: Special handling of medical device operations
- **Research Data**: Additional controls for clinical trial data

### General Enterprise

- **Principle of Least Privilege**: Minimal necessary permissions
- **Change Control**: Approval processes for infrastructure changes
- **Data Classification**: Different rules for different data types
- **Incident Response**: Clear escalation procedures for violations

## Advanced Configuration

### Rate Limiting

Prevent abuse and ensure availability:

```toml
[security_lens.rate_limit]
max_detections = 5      # Maximum detections
window_seconds = 60     # Time window
enabled = true          # Enable rate limiting
```

### Custom Documentation URLs

Link to your internal security documentation:

```toml
[security_lens]
docs_base_url = "https://internal.company.com/security"
```

### Multi-Environment Policies

Different configurations per environment:

```bash
# Development environment - relaxed
cp examples/dev-security-policy.toml ~/.config/openagent/security.toml

# Production environment - strict
cp examples/prod-security-policy.toml ~/.config/openagent/security.toml

# CI/CD environment - automated
cp examples/cicd-security-policy.toml ~/.config/openagent/security.toml
```

## Troubleshooting

### Common Issues

1. **False Positives**
   - Review pattern specificity
   - Add exclusion patterns
   - Adjust risk levels

2. **Missing Detections**
   - Test patterns with regex validators
   - Check case sensitivity
   - Verify pattern compilation

3. **Performance Issues**
   - Review pattern complexity
   - Enable rate limiting
   - Monitor detection frequency

### Debug Commands

```bash
# Test specific patterns
openagent-terminal --test-pattern "your-pattern"

# Validate configuration
openagent-terminal --validate-security-config

# View active patterns
openagent-terminal --list-security-patterns
```

## Contributing

To contribute new patterns or improve existing ones:

1. **Submit Pattern Proposals**
   - Include risk rationale
   - Provide test cases
   - Consider false positive impact

2. **Industry-Specific Bundles**
   - Government/defense
   - Education
   - Technology sector
   - Manufacturing

3. **Pattern Database**
   - Community pattern sharing
   - Threat intelligence integration
   - Automated pattern updates

For more information, see the [Security Lens Development Guide](./SECURITY_LENS_DEVELOPMENT.md).
