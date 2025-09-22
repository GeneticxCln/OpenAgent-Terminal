# OpenAgent Terminal Documentation

Welcome to the OpenAgent Terminal docs hub. This page is the entry point for all project documentation.

Quick links
- Getting started
  - Installation: ../INSTALL.md
  - Configuration: ../openagent-terminal/docs/configuration.md
  - Features overview: ./features.md
  - AI CLI usage: ./guides/AI_CLI_USAGE.md
- Architecture & design
  - Architecture overview: ./ARCHITECTURE.md
  - ADRs (design decisions): ./adr/README.md
  - Development plan: ./implementation/DEVELOPMENT_PLAN.md
  - Implementation roadmap: ./implementation/IMPLEMENTATION_ROADMAP.md
  - Phase summaries: ./guides/PHASE2_SUMMARY.md, ./guides/PHASE3_COMPLETE.md, ./guides/STATUS.md
- Security & privacy
  - Security Lens: ./security_lens.md
  - Security Lens policies: ./SECURITY_LENS_POLICIES.md
  - AI environment security: ./AI_ENVIRONMENT_SECURITY.md
  - Plugins signing: ./plugins_signing.md
- Plugins
  - Plugins overview: ./plugins.md
  - Plugin Host API quickstart: ./guides/PLUGIN_HOST_API_QUICKSTART.md
- Plugin SDK/API crate docs: ../crates/plugin-sdk/README.md
  - Example plugin: ../examples/plugins/hello-wasi/README.md
- Guides
  - Contributing: ../CONTRIBUTING.md
  - Comparison analysis: ./guides/COMPARISON_ANALYSIS.md
  - Implementation progress: ./implementation/IMPLEMENTATION_PROGRESS.md
- Roadmaps & release notes
  - Roadmap: ./roadmaps/ROADMAP.md
  - Changelog: ../CHANGELOG.md

CI overview
- Snapshot gating and performance thresholds are enforced in CI.
  - Core CI: .github/workflows/ci.yml
  - Performance: .github/workflows/performance.yml
  - WGPU Nightly Snapshots: .github/workflows/wgpu-nightly.yml
- Highlights
- GPU snapshots: scenario comparisons with threshold >= 0.996 on Linux via xvfb
  - Perf checks: cold start (≤800ms) and render latency (≤16ms)
  - Coverage: llvm-cov with minimal threshold; clippy denies warnings; sanitizers run on subset

Navigation tips
- The docs/ folder is organized by topic. If a link is missing or 404s, please open an issue.
- For the most current project status, see ./guides/STATUS.md.
- For a full list of features and configuration options, see ./features.md and ../openagent-terminal/docs/configuration.md.
