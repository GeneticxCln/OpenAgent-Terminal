# Plugin API

This crate defines the minimal, versioned API for plugins.

Versioning

- Crate version (SemVer) communicates compatibility
- Minor versions may add optional interfaces; patch versions fix bugs only
- Major versions introduce breaking changes; see the root CHANGELOG for migration notes

Stability expectations

- The core types and traits used by the current examples are considered stable
- Experimental additions are added behind feature flags where appropriate

Getting started

- See examples/plugins/hello-wasi for a minimal sample
- Host integration lives in crates/plugin-loader

Changelog

- See the repository-level CHANGELOG.md for updates affecting the plugin API

