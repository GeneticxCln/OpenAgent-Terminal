# Dependency Consolidation Plan

## Priority 1: Critical Duplicates (Immediate Impact)

### 1. Base64 Versions (0.21.7 vs 0.22.1)
**Root Cause:** metrics-exporter-prometheus uses old version
**Action:** 
```toml
# In Cargo.toml workspace.dependencies
base64 = "0.22.1"

# Force resolution
[workspace.dependencies.metrics-exporter-prometheus]
version = "0.13.1" 
default-features = false
features = ["http-listener"]
```
**Impact:** -15 duplicate crates

### 2. SQLx Version Alignment  
**Root Cause:** Mixed sqlx-core dependencies
**Action:**
```toml
[workspace.dependencies]
sqlx = { version = "0.8.1", default-features = false, features = ["runtime-tokio-rustls", "sqlite", "derive"] }
# Remove individual sqlx-* crate specifications
```
**Impact:** -12 duplicate crates

### 3. Rustix Versions (0.38.44 vs 1.1.2)
**Root Cause:** wasmtime uses newer version
**Action:**
```toml
# Force newer version workspace-wide
[workspace.dependencies]
rustix = "1.1.2"
```
**Impact:** -8 duplicate crates

## Priority 2: Feature Reduction

### 4. Disable Unused Features
```toml
# Reduce WGPU features to essentials
wgpu = { version = "26.0", default-features = false, features = ["wgsl", "dx12", "vulkan"] }

# Limit SQLx database support  
sqlx = { version = "0.8.1", features = ["sqlite", "runtime-tokio-rustls"] }
# Remove mysql/postgres unless needed

# Reduce wasmtime features
wasmtime = { version = "36.0", default-features = false, features = ["cranelift", "pooling-allocator"] }
```
**Impact:** -50+ transitive dependencies

## Priority 3: Alternative Libraries

### 5. Replace Heavy Dependencies
```bash
# Replace reqwest with lighter alternative for simple HTTP
# Consider: ureq or basic hyper client

# Replace full tokio with tokio-lite for some crates
# Use std::thread for simple async tasks where possible

# Replace criterion with lighter benchmarking in dev deps
```

## Implementation Order

1. **Week 1:** Fix version conflicts (base64, rustix, sqlx)
2. **Week 2:** Feature reduction and unused dependency removal  
3. **Week 3:** Consider alternative libraries for heavy dependencies
4. **Week 4:** Measure impact and fine-tune

## Expected Results

- **Before:** 927 total dependencies
- **After:** ~650-700 dependencies (25-30% reduction)
- **Build time improvement:** 15-25%
- **Binary size reduction:** 10-15%