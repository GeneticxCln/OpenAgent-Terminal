//! Context provider trait for AI runtime integration
//!
//! This module defines the trait for providing context information to the AI runtime
//! from various sources like Warp integration, terminal state, etc.

use openagent_terminal_core::tty::pty_manager::{PtyAiContext, ShellKind};
use std::path::PathBuf;

/// Trait for providing context to the AI runtime
pub trait AiContextProvider {
    /// Get the current working directory context
    fn get_working_directory(&self) -> Option<PathBuf>;

    /// Get the current shell type
    fn get_shell_kind(&self) -> Option<ShellKind>;

    /// Get the last executed command (if available)
    fn get_last_command(&self) -> Option<String>;

    /// Get the shell executable name
    fn get_shell_executable(&self) -> Option<String>;

    /// Get full PTY context (convenience method)
    fn get_pty_context(&self) -> Option<PtyAiContext>;

    /// Update context with a new command (for tracking)
    fn update_command_context(&mut self, command: &str);
}

/// Default implementation that provides no context (null pattern)
pub struct NullContextProvider;

impl AiContextProvider for NullContextProvider {
    fn get_working_directory(&self) -> Option<PathBuf> {
        std::env::current_dir().ok()
    }

    fn get_shell_kind(&self) -> Option<ShellKind> {
        // Try to detect from SHELL environment variable
        if let Ok(shell_path) = std::env::var("SHELL") {
            Some(ShellKind::from_shell_name(&shell_path))
        } else {
            None
        }
    }

    fn get_last_command(&self) -> Option<String> {
        None
    }

    fn get_shell_executable(&self) -> Option<String> {
        std::env::var("SHELL").ok()
    }

    fn get_pty_context(&self) -> Option<PtyAiContext> {
        Some(PtyAiContext {
            working_directory: self.get_working_directory()?,
            shell_kind: self.get_shell_kind().unwrap_or(ShellKind::Unknown),
            last_command: self.get_last_command(),
            shell_executable: self
                .get_shell_executable()
                .unwrap_or_else(|| "bash".to_string()),
        })
    }

    fn update_command_context(&mut self, _command: &str) {
        // Null provider doesn't track commands
    }
}

/// Warp integration context provider
///
/// This wraps a Warp integration instance and provides context from it
pub struct WarpContextProvider<'a> {
    pub warp_integration: &'a crate::workspace::warp_integration::WarpIntegration,
}

impl<'a> WarpContextProvider<'a> {
    pub fn new(warp_integration: &'a crate::workspace::warp_integration::WarpIntegration) -> Self {
        Self { warp_integration }
    }
}

impl<'a> AiContextProvider for WarpContextProvider<'a> {
    fn get_working_directory(&self) -> Option<PathBuf> {
        self.warp_integration
            .get_current_ai_context()
            .map(|ctx| ctx.working_directory)
    }

    fn get_shell_kind(&self) -> Option<ShellKind> {
        self.warp_integration
            .get_current_ai_context()
            .map(|ctx| ctx.shell_kind)
    }

    fn get_last_command(&self) -> Option<String> {
        self.warp_integration
            .get_current_ai_context()
            .and_then(|ctx| ctx.last_command)
    }

    fn get_shell_executable(&self) -> Option<String> {
        self.warp_integration
            .get_current_ai_context()
            .map(|ctx| ctx.shell_executable)
    }

    fn get_pty_context(&self) -> Option<PtyAiContext> {
        self.warp_integration.get_current_ai_context()
    }

    fn update_command_context(&mut self, _command: &str) {
        // The Warp integration handles command tracking internally
        // We can't mutate through the immutable reference, so this is a no-op
        // In practice, the workspace manager would call warp_integration.update_command_context()
    }
}

/// Mutable Warp integration context provider
///
/// This version allows mutation for command tracking
pub struct MutableWarpContextProvider<'a> {
    pub warp_integration: &'a mut crate::workspace::warp_integration::WarpIntegration,
}

impl<'a> MutableWarpContextProvider<'a> {
    pub fn new(
        warp_integration: &'a mut crate::workspace::warp_integration::WarpIntegration,
    ) -> Self {
        Self { warp_integration }
    }
}

impl<'a> AiContextProvider for MutableWarpContextProvider<'a> {
    fn get_working_directory(&self) -> Option<PathBuf> {
        self.warp_integration
            .get_current_ai_context()
            .map(|ctx| ctx.working_directory)
    }

    fn get_shell_kind(&self) -> Option<ShellKind> {
        self.warp_integration
            .get_current_ai_context()
            .map(|ctx| ctx.shell_kind)
    }

    fn get_last_command(&self) -> Option<String> {
        self.warp_integration
            .get_current_ai_context()
            .and_then(|ctx| ctx.last_command)
    }

    fn get_shell_executable(&self) -> Option<String> {
        self.warp_integration
            .get_current_ai_context()
            .map(|ctx| ctx.shell_executable)
    }

    fn get_pty_context(&self) -> Option<PtyAiContext> {
        self.warp_integration.get_current_ai_context()
    }

    fn update_command_context(&mut self, command: &str) {
        self.warp_integration.update_command_context(command);
    }
}

/// Convenience function to convert context to AI request parameters
pub fn context_to_ai_params(context: &Option<PtyAiContext>) -> (Option<String>, Option<String>) {
    if let Some(ctx) = context {
        let (working_dir, shell_kind) = ctx.to_strings();
        (Some(working_dir), Some(shell_kind))
    } else {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_context_provider() {
        let provider = NullContextProvider;

        // Should provide basic environment-based context
        assert!(provider.get_working_directory().is_some());
        assert!(provider.get_last_command().is_none());

        if let Some(context) = provider.get_pty_context() {
            assert!(!context.working_directory.as_os_str().is_empty());
            assert!(!context.shell_executable.is_empty());
        }
    }

    #[test]
    fn test_context_to_ai_params() {
        let context = PtyAiContext {
            working_directory: PathBuf::from("/home/user"),
            shell_kind: ShellKind::Bash,
            last_command: Some("ls -la".to_string()),
            shell_executable: "bash".to_string(),
        };

        let (working_dir, shell_kind) = context_to_ai_params(&Some(context));

        assert_eq!(working_dir, Some("/home/user".to_string()));
        assert_eq!(shell_kind, Some("bash".to_string()));
    }

    #[test]
    fn test_context_to_ai_params_none() {
        let (working_dir, shell_kind) = context_to_ai_params(&None);

        assert_eq!(working_dir, None);
        assert_eq!(shell_kind, None);
    }
}
