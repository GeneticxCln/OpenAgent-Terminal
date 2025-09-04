//! Simple demo plugin using the convenient simple_plugin macro
//! This demonstrates the most basic plugin setup possible.

use plugin_sdk::{simple_plugin, log, LogLevel};

// Define a minimal plugin with just metadata - all functions use defaults
simple_plugin!(
    "simple-demo",
    "1.0.0",
    "OpenAgent Terminal Team",
    "A very simple plugin demonstrating the simple_plugin macro"
);

// The simple_plugin macro automatically generates:
// - plugin_init() that logs initialization
// - plugin_get_metadata() that returns the specified metadata
// - plugin_cleanup() that logs cleanup
// - No event handler (uses default)
