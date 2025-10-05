# Safe IpcClient Ownership Pattern - Quick Reference

## ❌ WRONG: Dangerous Patterns

### Don't wrap borrowed references in Arc
```rust
// WRONG! Borrowed reference in Arc
let mut client = IpcClient::new();
let client = Arc::new(Mutex::new(&mut client)); // DANGER!
```

### Don't use raw pointers
```rust
// WRONG! Raw pointer storage
pub struct SessionManager {
    ipc_client: *mut IpcClient, // UNSAFE!
}

unsafe impl Send for SessionManager {} // UNSAFE!
```

## ✅ CORRECT: Safe Pattern

### Own the client in Arc<Mutex>
```rust
// Create client
let mut client = IpcClient::new();
client.connect(&socket_path).await?;
client.initialize().await?;

// Transfer ownership to Arc<Mutex>
let client = Arc::new(Mutex::new(client)); // ✅ Safe!

// Clone Arc for consumers (cheap: just ref count increment)
let session_manager = SessionManager::new(Arc::clone(&client));
run_interactive_loop(Arc::clone(&client), &mut session_manager).await?;
```

### Store Arc clones in consumers
```rust
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SessionManager {
    ipc_client: Arc<Mutex<IpcClient>>, // ✅ Safe shared ownership
}

// No unsafe impl needed - Arc<Mutex> is already Send+Sync!

impl SessionManager {
    pub fn new(ipc_client: Arc<Mutex<IpcClient>>) -> Self {
        Self { ipc_client }
    }
}
```

### Access through async locks
```rust
pub async fn some_operation(&mut self) -> Result<()> {
    // Lock only for IPC operation
    let response = {
        let mut client = self.ipc_client.lock().await;
        client.send_request(request).await?
    }; // Lock dropped here automatically
    
    // Process response without holding lock
    process_response(response)
}
```

## Key Rules

1. **Own, don't borrow**: `Arc<Mutex<IpcClient>>`, not `Arc<Mutex<&mut IpcClient>>`
2. **Clone Arc, not client**: `Arc::clone(&client)` is cheap (atomic increment)
3. **Lock scope**: Keep `.lock().await` scopes minimal
4. **No unsafe**: Let Arc and Mutex handle synchronization
5. **Pass clones**: Give each consumer `Arc::clone(&client)`

## Benefits

- ✅ **Zero unsafe code**
- ✅ **Compiler-verified thread safety**
- ✅ **Safe across await points**
- ✅ **Clear ownership semantics**
- ✅ **Automatic cleanup**

## Cost

- **Memory**: ~16 bytes for Arc control block (negligible)
- **Clone**: 1 atomic increment (~1-5 CPU cycles)
- **Lock**: Minimal for uncontended locks

## Type Signature Reference

```rust
// Function parameters
async fn my_function(
    client: Arc<Mutex<IpcClient>>,  // ✅ Correct
    // NOT: client: &mut IpcClient,    // ❌ Wrong
    // NOT: Arc<Mutex<&mut IpcClient>> // ❌ Wrong
) -> Result<()>

// Struct fields
pub struct MyStruct {
    ipc_client: Arc<Mutex<IpcClient>>, // ✅ Correct
    // NOT: *mut IpcClient,              // ❌ Wrong
    // NOT: Option<*mut IpcClient>,      // ❌ Wrong
}

// Cloning
let clone = Arc::clone(&client); // ✅ Correct
// NOT: let clone = client.clone(); // Works but less clear
```

## Migration Checklist

When adding a new IpcClient consumer:

- [ ] Accept `Arc<Mutex<IpcClient>>` in constructor
- [ ] Store the Arc (not a reference, not a pointer)
- [ ] Use `.lock().await` to access
- [ ] Keep lock scopes minimal (use block `{}`)
- [ ] No unsafe code needed

## Example: New Component

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::ipc::IpcClient;

pub struct NewComponent {
    ipc_client: Arc<Mutex<IpcClient>>,
}

impl NewComponent {
    pub fn new(ipc_client: Arc<Mutex<IpcClient>>) -> Self {
        Self { ipc_client }
    }
    
    pub async fn do_work(&self) -> Result<()> {
        let response = {
            let mut client = self.ipc_client.lock().await;
            client.send_request(request).await?
        };
        Ok(())
    }
}

// Usage:
let component = NewComponent::new(Arc::clone(&client));
```

## Common Mistakes

### Mistake 1: Holding lock too long
```rust
// ❌ Bad: Lock held during processing
let mut client = self.ipc_client.lock().await;
let response = client.send_request(request).await?;
// ... lots of processing while holding lock ...
process(response)?; // Still holding lock!
```

```rust
// ✅ Good: Lock released before processing
let response = {
    let mut client = self.ipc_client.lock().await;
    client.send_request(request).await?
}; // Lock dropped
process(response)?; // Lock released
```

### Mistake 2: Wrapping borrowed reference
```rust
// ❌ Bad: Arc wraps borrowed reference
let mut client = IpcClient::new();
let wrapped = Arc::new(Mutex::new(&mut client)); // DANGER!
```

```rust
// ✅ Good: Arc owns the client
let client = IpcClient::new();
let wrapped = Arc::new(Mutex::new(client)); // Safe
```

### Mistake 3: Using raw pointers
```rust
// ❌ Bad: Raw pointer
pub struct Manager {
    client: *mut IpcClient, // UNSAFE!
}
```

```rust
// ✅ Good: Arc clone
pub struct Manager {
    client: Arc<Mutex<IpcClient>>, // Safe
}
```

## Verification Commands

```bash
# Check for unsafe code
grep -r "unsafe" src/
# Should return nothing

# Check for raw pointers
grep -r "\*mut\|\*const" src/
# Should return nothing

# Verify compilation
cargo build
# Should succeed without warnings
```
