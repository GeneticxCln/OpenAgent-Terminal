# Security Lens Feature Gates

## What This Provides

This implementation adds **conditional compilation** for your Security Lens features, allowing you to control what gets built and tested.

## Feature Flags Added

In `openagent-terminal/Cargo.toml`:

- `security-lens` - Core Security Lens functionality  
- `security-lens-extended` - Enhanced patterns (cloud, containers, databases)
- `security-lens-platform` - Platform-specific patterns (Linux, macOS, Windows)
- `security-lens-advanced` - Rate limiting and advanced features
- `security-lens-full` - Complete Security Lens suite
- `security-lens-dev` - Development tools and enhanced testing

## Usage

### Build with different Security Lens levels:

```bash
# Core only
cargo build --features security-lens

# Extended patterns 
cargo build --features security-lens-extended

# All features
cargo build --features security-lens-full

# Build WITHOUT Security Lens (stub implementation used)
cargo build --no-default-features --features "wayland,x11"
```

### Test specific Security Lens features:

```bash
# Test core Security Lens
cargo test --features security-lens security

# Test all Security Lens features
cargo test --features security-lens-full security
```

## How It Works

1. **Conditional Compilation**: The `src/security/mod.rs` module uses `#[cfg(feature = "security-lens")]` to include/exclude Security Lens code
2. **Stub Implementation**: When Security Lens features are disabled, stub types are used that return "safe" results
3. **CI Testing**: The CI now includes a `security-lens` feature set to ensure it builds correctly

## File Structure

```
src/
├── security/
│   ├── mod.rs              # Feature-gated module entry
│   └── security_lens.rs    # Your full Security Lens implementation
├── lib.rs                  # Updated to use security module
└── ...
```

When `security-lens` feature is **disabled**: stub implementations are used
When `security-lens` feature is **enabled**: full Security Lens code is compiled

This lets you:
- Build faster during development by excluding Security Lens
- Test only Security Lens components when needed  
- Include Security Lens selectively in different build configurations

## Testing the Feature Gates

Run `./test-features.sh` to verify the feature gates work correctly.
