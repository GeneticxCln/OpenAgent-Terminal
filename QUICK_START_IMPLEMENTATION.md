# Quick Start Implementation Guide

## Immediate Actions (Day 1-7)

This guide provides actionable steps to immediately begin the v1.0 release implementation based on the analysis of current issues.

---

## Day 1: Emergency Code Quality Fixes

### Fix Compiler Warnings (2-3 hours)

**Priority: CRITICAL** - These warnings indicate technical debt that needs immediate attention.

#### Step 1: Fix Unused Imports
```bash
# Navigate to project root
cd /home/sasha/OpenAgent-Terminal

# Fix specific warnings identified in analysis
```

**File: `openagent-terminal/src/display/confirm_overlay.rs`**
```rust
// Remove unused imports on line 7
// OLD:
use crate::security_lens::{CommandRisk, RiskLevel};

// NEW: Remove entirely or comment out until used
// use crate::security_lens::{CommandRisk, RiskLevel};
```

**File: `openagent-terminal/src/input/keyboard.rs`**
```rust
// Remove unused imports on line 1650
// OLD:
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// NEW: Remove entirely or comment out until used
// use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
```

**File: `openagent-terminal/src/display/palette.rs`**
```rust
// Fix unused assignment on line 815
// OLD:
col_cursor += 3;

// NEW: Use the value or mark as intentional
let _col_cursor = col_cursor + 3; // Explicitly mark as unused
```

#### Step 2: Fix Unused Variables
```rust
// File: openagent-terminal/src/input/keyboard.rs line 183
// OLD:
let clip = self.ctx.clipboard_mut().load(ClipboardType::Clipboard);

// NEW: Either use the variable or prefix with underscore
let _clip = self.ctx.clipboard_mut().load(ClipboardType::Clipboard);
```

#### Step 3: Fix Dead Code
```rust
// File: openagent-terminal/src/display/mod.rs line 503
// Either use the field or mark with allow attribute
#[allow(dead_code)]
pub(crate) palette_last_active: bool,
```

#### Verification:
```bash
# Run checks to verify fixes
cargo check --workspace
cargo clippy --workspace

# Should see zero warnings
```

---

## Day 2: Git Hygiene Setup

### Establish Proper Commit Standards (1-2 hours)

The recent commit history shows poor practices that need immediate correction.

#### Step 1: Install Commit Tools
```bash
# Install commitizen globally
npm install -g commitizen cz-conventional-changelog

# Configure commitizen
echo '{"path": "cz-conventional-changelog"}' > ~/.czrc
```

#### Step 2: Create Commit Template
```bash
# Create .gitmessage template
cat > .gitmessage << EOF
# <type>(<scope>): <subject>
#
# <body>
#
# <footer>
#
# Type should be one of:
# - feat: A new feature
# - fix: A bug fix
# - docs: Documentation only changes
# - style: Changes that do not affect the meaning of the code
# - refactor: A code change that neither fixes a bug nor adds a feature
# - perf: A code change that improves performance
# - test: Adding missing tests or correcting existing tests
# - build: Changes that affect the build system or external dependencies
# - ci: Changes to our CI configuration files and scripts
# - chore: Other changes that don't modify src or test files
# - revert: Reverts a previous commit
#
# Scope is optional and should be the area of change (ai, config, ui, etc.)
# Subject should use imperative mood ("add feature" not "added feature")
# Body should explain what and why vs. how
# Footer should contain breaking changes and issue references
EOF

# Configure git to use template
git config commit.template .gitmessage
```

#### Step 3: Set Up Pre-commit Hooks
```bash
# Install pre-commit
pip install pre-commit

# Create pre-commit configuration
cat > .pre-commit-config.yaml << EOF
repos:
  - repo: https://github.com/pre-commit/pre-commit-hooks
    rev: v4.4.0
    hooks:
      - id: trailing-whitespace
      - id: end-of-file-fixer
      - id: check-yaml
      - id: check-toml
      - id: check-merge-conflict

  - repo: local
    hooks:
      - id: cargo-check
        name: cargo check
        entry: cargo check --workspace
        language: system
        types: [rust]
        pass_filenames: false

      - id: cargo-clippy
        name: cargo clippy
        entry: cargo clippy --workspace -- -D warnings
        language: system
        types: [rust]
        pass_filenames: false

      - id: cargo-fmt
        name: cargo fmt
        entry: cargo fmt --all -- --check
        language: system
        types: [rust]
        pass_filenames: false
EOF

# Install hooks
pre-commit install
```

---

## Day 3-4: Dependency Cleanup

### Audit and Reduce Dependencies (4-6 hours)

Current: 738 dependencies - Target: <600 dependencies

#### Step 1: Install Analysis Tools
```bash
# Install dependency analysis tools
cargo install cargo-machete cargo-audit cargo-bloat cargo-tree
```

#### Step 2: Find Unused Dependencies
```bash
# Find unused dependencies
cargo machete

# Create cleanup plan
echo "# Dependency Cleanup Plan" > dependency_cleanup.md
echo "Generated: $(date)" >> dependency_cleanup.md
echo "" >> dependency_cleanup.md
cargo machete >> dependency_cleanup.md
```

#### Step 3: Security Audit
```bash
# Run security audit
cargo audit

# If vulnerabilities found, prioritize fixes
cargo audit --format json > security_audit.json
```

#### Step 4: Analyze Binary Size Impact
```bash
# Analyze what's taking up space
cargo bloat --release --crates

# Create size report
cargo bloat --release --crates > binary_size_analysis.txt
```

#### Step 5: Create Cleanup Script
```bash
cat > scripts/cleanup_dependencies.sh << 'EOF'
#!/bin/bash
set -e

echo "🧹 Starting dependency cleanup..."

# Backup current Cargo.toml files
find . -name "Cargo.toml" -exec cp {} {}.backup \;

# Remove unused dependencies (run cargo machete and manually review)
echo "📋 Running cargo machete to find unused dependencies..."
cargo machete

# Clean up known problematic dependencies
echo "🔧 Cleaning up specific dependencies..."

# Remove unnecessary dev-dependencies that aren't used
# (These would be identified from cargo machete output)

# Update dependency versions to latest compatible
echo "⬆️ Updating dependencies..."
cargo update

# Verify everything still compiles
echo "✅ Verifying compilation..."
cargo check --workspace

echo "✨ Dependency cleanup complete!"
echo "📊 New dependency count:"
grep -c "name = " Cargo.lock

EOF

chmod +x scripts/cleanup_dependencies.sh
```

---

## Day 5: Performance Baseline

### Establish Current Performance Metrics (2-3 hours)

#### Step 1: Create Benchmark Suite
```bash
# Create benchmarks directory
mkdir -p benches/baseline

# Startup time benchmark
cat > benches/baseline/startup_time.rs << 'EOF'
use criterion::{criterion_group, criterion_main, Criterion};
use std::process::Command;
use std::time::Duration;

fn benchmark_startup_time(c: &mut Criterion) {
    let mut group = c.benchmark_group("startup");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("cold_start", |b| {
        b.iter(|| {
            let output = Command::new("./target/release/openagent-terminal")
                .arg("--help")
                .output()
                .expect("Failed to execute command");
            assert!(output.status.success());
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_startup_time);
criterion_main!(benches);
EOF
```

#### Step 2: Memory Usage Benchmark
```bash
cat > benches/baseline/memory_usage.rs << 'EOF'
use criterion::{criterion_group, criterion_main, Criterion};
use std::process::{Command, Stdio};
use std::time::Duration;

fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("idle_memory", |b| {
        b.iter(|| {
            // Start the terminal in background
            let mut child = Command::new("./target/release/openagent-terminal")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("Failed to start terminal");

            // Give it time to initialize
            std::thread::sleep(Duration::from_millis(500));

            // Measure memory usage (simplified - would use more sophisticated measurement)
            let pid = child.id();
            let memory_output = Command::new("ps")
                .args(&["-o", "rss=", "-p", &pid.to_string()])
                .output()
                .expect("Failed to measure memory");

            child.kill().expect("Failed to kill process");

            let memory_kb: i32 = String::from_utf8(memory_output.stdout)
                .unwrap()
                .trim()
                .parse()
                .unwrap_or(0);

            // Convert to MB and assert it's within target
            let memory_mb = memory_kb / 1024;
            assert!(memory_mb < 50, "Memory usage {} MB exceeds 50 MB target", memory_mb);
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_memory_usage);
criterion_main!(benches);
EOF
```

#### Step 3: Run Baseline Measurements
```bash
# Build release binary first
cargo build --release

# Run benchmarks
cargo bench --bench startup_time
cargo bench --bench memory_usage

# Create baseline report
cat > PERFORMANCE_BASELINE.md << EOF
# Performance Baseline Report

Generated: $(date)

## Current Performance Metrics

### Startup Time
- Target: <100ms
- Current: [TBD - run benchmark]

### Memory Usage
- Target: <50MB base, <150MB with AI
- Current Base: [TBD - run benchmark]
- Current with AI: [TBD - run benchmark]

### Render Performance
- Target: <16ms (60 FPS)
- Current: [TBD - needs implementation]

## Action Items
- [ ] Implement render performance benchmark
- [ ] Optimize startup time if over target
- [ ] Optimize memory usage if over target
- [ ] Set up continuous performance monitoring

EOF
```

---

## Day 6-7: Plugin System Analysis

### Evaluate Plugin System Complexity (3-4 hours)

#### Step 1: Analyze Current Plugin System
```bash
# Count plugin-related code
echo "📊 Plugin System Analysis" > PLUGIN_ANALYSIS.md
echo "========================" >> PLUGIN_ANALYSIS.md
echo "" >> PLUGIN_ANALYSIS.md

echo "## Code Statistics" >> PLUGIN_ANALYSIS.md
find . -name "*.rs" -path "*/plugin*" -exec wc -l {} + >> PLUGIN_ANALYSIS.md

echo "" >> PLUGIN_ANALYSIS.md
echo "## Dependency Count" >> PLUGIN_ANALYSIS.md
grep -r "wasmtime\|wasm" Cargo.toml >> PLUGIN_ANALYSIS.md

echo "" >> PLUGIN_ANALYSIS.md
echo "## Warnings Count" >> PLUGIN_ANALYSIS.md
cargo check 2>&1 | grep -i "plugin" | wc -l >> PLUGIN_ANALYSIS.md
```

#### Step 2: Create Simplification Proposal
```bash
cat > PLUGIN_SIMPLIFICATION_PROPOSAL.md << 'EOF'
# Plugin System Simplification Proposal

## Current State
- Complex WASM runtime with wasmtime
- 6+ warnings in plugin-loader
- Unused capabilities and metadata structures
- Over-engineered for v1.0 needs

## Proposed v1.0 Simplification

### New Simple Plugin Trait
```rust
/// Simplified plugin interface for v1.0
pub trait SimplePlugin: Send + Sync {
    /// Plugin identifier
    fn name(&self) -> &str;

    /// Plugin version
    fn version(&self) -> &str;

    /// Execute plugin with input, return result
    fn execute(&self, input: &str) -> Result<String, PluginError>;

    /// Optional: Plugin description
    fn description(&self) -> Option<&str> { None }
}

/// Simple error type
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}
```

### Migration Plan
1. Create new simple plugin trait
2. Update existing plugins to use new API
3. Remove WASM runtime dependencies
4. Keep plugin discovery mechanism
5. Document migration path to v2.0

### Benefits
- Reduce complexity significantly
- Eliminate warnings
- Faster compilation
- Easier testing
- Clear upgrade path

### Risks
- May disappoint users expecting advanced plugin features
- Need to communicate this is temporary simplification

## Implementation Steps
1. [ ] Create new plugin API in separate module
2. [ ] Migrate existing plugins
3. [ ] Remove WASM dependencies
4. [ ] Update documentation
5. [ ] Add feature flag for v2.0 migration

EOF
```

---

## Week Summary Checklist

### End of Week 1 Goals:
- [ ] All compiler warnings fixed
- [ ] Proper git commit standards established
- [ ] Pre-commit hooks working
- [ ] Dependency cleanup plan created
- [ ] Performance baseline established
- [ ] Plugin simplification proposal approved

### Verification Commands:
```bash
# Code quality check
cargo check --workspace  # Should be warning-free
cargo clippy --workspace # Should be warning-free

# Git setup check
git log --oneline -n 5    # Should see proper commit messages (if new commits made)

# Performance check
cargo bench              # Should have baseline numbers

# Documentation check
ls *.md                  # Should see new analysis documents
```

---

## Next Steps (Week 2)

1. **Implement Plugin Simplification** (2-3 days)
2. **Start Testing Infrastructure** (2-3 days)
3. **Begin AI Provider Stabilization** (2-3 days)

---

## Emergency Contacts & Resources

### If You Get Stuck:
1. **Rust Issues**: Check Rust documentation and forums
2. **Git Issues**: Refer to Git documentation
3. **Build Issues**: Check GitHub Actions logs
4. **Performance Issues**: Use `cargo flamegraph` for profiling

### Useful Commands:
```bash
# Quick health check
make check || cargo check --workspace

# Full clean rebuild
cargo clean && cargo build --release

# Generate documentation
cargo doc --open

# Profile application
cargo install flamegraph
cargo flamegraph --bin openagent-terminal
```

---

*This guide focuses on immediate, actionable steps to begin the cleanup and stabilization process. Each day builds upon the previous day's work to establish a solid foundation for the full v1.0 release.*
