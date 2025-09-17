use std::borrow::Cow;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

use crate::term::ClipboardType;
use crate::vte::ansi::Rgb;

/// Command block lifecycle events (e.g. OSC 133 integration).
#[derive(Clone, Debug)]
pub enum CommandBlockEvent {
    /// Prompt start (OSC 133;A).
    PromptStart,
    /// Command start (OSC 133;B). Optional raw command string.
    CommandStart { cmd: Option<String> },
    /// Command end (OSC 133;C). Optional exit code and working directory.
    CommandEnd {
        exit: Option<i32>,
        cwd: Option<String>,
    },
    /// Prompt end (OSC 133;D).
    PromptEnd,
}

/// Terminal event.
///
/// These events instruct the UI over changes that can't be handled by the terminal emulation layer
/// itself.
#[derive(Clone)]
pub enum Event {
    /// Grid has changed possibly requiring a mouse cursor shape change.
    MouseCursorDirty,

    /// Window title change.
    Title(String),

    /// Reset to the default window title.
    ResetTitle,

    /// Request to store a text string in the clipboard.
    ClipboardStore(ClipboardType, String),

    /// Request to write the contents of the clipboard to the PTY.
    ///
    /// The attached function is a formatter which will correctly transform the clipboard content
    /// into the expected escape sequence format.
    ClipboardLoad(
        ClipboardType,
        Arc<dyn Fn(&str) -> String + Sync + Send + 'static>,
    ),

    /// Request to write the RGB value of a color to the PTY.
    ///
    /// The attached function is a formatter which will correctly transform the RGB color into the
    /// expected escape sequence format.
    ColorRequest(usize, Arc<dyn Fn(Rgb) -> String + Sync + Send + 'static>),

    /// Write some text to the PTY.
    PtyWrite(String),

    /// Request to write the text area size.
    TextAreaSizeRequest(Arc<dyn Fn(WindowSize) -> String + Sync + Send + 'static>),

    /// Cursor blinking state has changed.
    CursorBlinkingChange,

    /// New terminal content available.
    Wakeup,

    /// Terminal bell ring.
    Bell,

    /// Shutdown request.
    Exit,

    /// Child process exited with an error code.
    ChildExit(i32),

    /// Command block lifecycle notification (OSC 133 derived).
    CommandBlock(CommandBlockEvent),
}

impl Debug for Event {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Event::ClipboardStore(ty, text) => write!(f, "ClipboardStore({ty:?}, {text})"),
            Event::ClipboardLoad(ty, _) => write!(f, "ClipboardLoad({ty:?})"),
            Event::TextAreaSizeRequest(_) => write!(f, "TextAreaSizeRequest"),
            Event::ColorRequest(index, _) => write!(f, "ColorRequest({index})"),
            Event::PtyWrite(text) => write!(f, "PtyWrite({text})"),
            Event::Title(title) => write!(f, "Title({title})"),
            Event::CursorBlinkingChange => write!(f, "CursorBlinkingChange"),
            Event::MouseCursorDirty => write!(f, "MouseCursorDirty"),
            Event::ResetTitle => write!(f, "ResetTitle"),
            Event::Wakeup => write!(f, "Wakeup"),
            Event::Bell => write!(f, "Bell"),
            Event::Exit => write!(f, "Exit"),
            Event::ChildExit(code) => write!(f, "ChildExit({code})"),
            Event::CommandBlock(ev) => write!(f, "CommandBlock({ev:?})"),
        }
    }
}

/// Byte sequences are sent to a `Notify` in response to some events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyError {
    /// Failed to send bytes to the notification channel.
    SendFailed,
    /// The notification channel is disconnected.
    Disconnected,
    /// The notification payload is too large.
    PayloadTooLarge(usize),
    /// The notification system is temporarily unavailable.
    Unavailable,
}

impl std::fmt::Display for NotifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotifyError::SendFailed => write!(f, "Failed to send notification bytes"),
            NotifyError::Disconnected => write!(f, "Notification channel is disconnected"),
            NotifyError::PayloadTooLarge(size) => {
                write!(f, "Notification payload too large: {} bytes", size)
            }
            NotifyError::Unavailable => write!(f, "Notification system is temporarily unavailable"),
        }
    }
}

impl std::error::Error for NotifyError {}

pub trait Notify {
    /// Notify that an escape sequence should be written to the PTY.
    /// 
    /// This is the infallible variant that will log errors internally rather than
    /// propagating them. Use `try_notify` for explicit error handling.
    fn notify<B: Into<Cow<'static, [u8]>>>(&self, _: B);

    /// Fallible form of notify that returns detailed error information.
    /// 
    /// This should be the preferred method when error handling is important.
    /// The default implementation bridges to the infallible `notify` method,
    /// but implementations should override this to provide proper error reporting.
    /// 
    /// # Errors
    /// 
    /// Returns `NotifyError` variants based on the specific failure:
    /// - `SendFailed`: Generic send failure
    /// - `Disconnected`: The notification channel is no longer available
    /// - `PayloadTooLarge`: The payload exceeds size limits
    /// - `Unavailable`: The notification system is temporarily unavailable
    fn try_notify<B: Into<Cow<'static, [u8]>>>(&self, bytes: B) -> Result<(), NotifyError> {
        self.notify(bytes);
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct WindowSize {
    pub num_lines: u16,
    pub num_cols: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

/// Types that are interested in when the display is resized.
pub trait OnResize {
    fn on_resize(&mut self, window_size: WindowSize);
}

/// Event Loop for notifying the renderer about terminal events.
pub trait EventListener {
    fn send_event(&self, _event: Event) {}
}

/// Null sink for events.
pub struct VoidListener;

impl EventListener for VoidListener {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use std::sync::mpsc;

    /// Mock notifier that always fails for testing error paths
    struct FailingNotifier;

    impl Notify for FailingNotifier {
        fn notify<B: Into<Cow<'static, [u8]>>>(&self, _bytes: B) {
            // This implementation does nothing, simulating a failure scenario
            // where notifications are dropped
        }

        fn try_notify<B: Into<Cow<'static, [u8]>>>(&self, _bytes: B) -> Result<(), NotifyError> {
            Err(NotifyError::SendFailed)
        }
    }

    /// Mock notifier that tracks notifications for testing
    struct TrackingNotifier {
        sender: mpsc::Sender<Vec<u8>>,
    }

    impl TrackingNotifier {
        fn new() -> (Self, mpsc::Receiver<Vec<u8>>) {
            let (sender, receiver) = mpsc::channel();
            (Self { sender }, receiver)
        }
    }

    impl Notify for TrackingNotifier {
        fn notify<B: Into<Cow<'static, [u8]>>>(&self, bytes: B) {
            let _ = self.try_notify(bytes);
        }

        fn try_notify<B: Into<Cow<'static, [u8]>>>(&self, bytes: B) -> Result<(), NotifyError> {
            let bytes = bytes.into();
            self.sender
                .send(bytes.to_vec())
                .map_err(|_| NotifyError::SendFailed)
        }
    }

    /// Notifier that simulates various error conditions
    struct ConditionalNotifier {
        fail_mode: FailureMode,
        call_count: std::cell::RefCell<usize>,
    }

    #[derive(Debug, Clone, Copy)]
    enum FailureMode {
        Never,
        Always,
        PayloadTooLarge(usize),
        Disconnected,
        Unavailable,
        OnSecondCall,
    }

    impl ConditionalNotifier {
        fn new(fail_mode: FailureMode) -> Self {
            Self {
                fail_mode,
                call_count: std::cell::RefCell::new(0),
            }
        }
    }

    impl Notify for ConditionalNotifier {
        fn notify<B: Into<Cow<'static, [u8]>>>(&self, bytes: B) {
            // For infallible notify, we just ignore errors
            let _ = self.try_notify(bytes);
        }

        fn try_notify<B: Into<Cow<'static, [u8]>>>(&self, bytes: B) -> Result<(), NotifyError> {
            let mut count = self.call_count.borrow_mut();
            *count += 1;
            let call_count = *count;
            drop(count);

            let bytes = bytes.into();
            match self.fail_mode {
                FailureMode::Never => Ok(()),
                FailureMode::Always => Err(NotifyError::SendFailed),
                FailureMode::PayloadTooLarge(limit) => {
                    if bytes.len() > limit {
                        Err(NotifyError::PayloadTooLarge(bytes.len()))
                    } else {
                        Ok(())
                    }
                }
                FailureMode::Disconnected => Err(NotifyError::Disconnected),
                FailureMode::Unavailable => Err(NotifyError::Unavailable),
                FailureMode::OnSecondCall => {
                    if call_count >= 2 {
                        Err(NotifyError::SendFailed)
                    } else {
                        Ok(())
                    }
                }
            }
        }
    }

    #[test]
    fn test_notify_error_display() {
        assert_eq!(
            NotifyError::SendFailed.to_string(),
            "Failed to send notification bytes"
        );
        assert_eq!(
            NotifyError::Disconnected.to_string(),
            "Notification channel is disconnected"
        );
        assert_eq!(
            NotifyError::PayloadTooLarge(1024).to_string(),
            "Notification payload too large: 1024 bytes"
        );
        assert_eq!(
            NotifyError::Unavailable.to_string(),
            "Notification system is temporarily unavailable"
        );
    }

    #[test]
    fn test_notify_error_equality() {
        assert_eq!(NotifyError::SendFailed, NotifyError::SendFailed);
        assert_eq!(NotifyError::Disconnected, NotifyError::Disconnected);
        assert_eq!(NotifyError::PayloadTooLarge(100), NotifyError::PayloadTooLarge(100));
        assert_ne!(NotifyError::PayloadTooLarge(100), NotifyError::PayloadTooLarge(200));
        assert_ne!(NotifyError::SendFailed, NotifyError::Disconnected);
    }

    #[test]
    fn test_failing_notifier_try_notify() {
        let notifier = FailingNotifier;
        let result = notifier.try_notify(b"test".as_slice());
        assert!(matches!(result, Err(NotifyError::SendFailed)));
    }

    #[test]
    fn test_failing_notifier_infallible_notify() {
        let notifier = FailingNotifier;
        // This should not panic even though try_notify fails
        notifier.notify(b"test".as_slice());
    }

    #[test]
    fn test_tracking_notifier_success() {
        let (notifier, receiver) = TrackingNotifier::new();
        
        // Test successful notification
        let result = notifier.try_notify(b"hello".as_slice());
        assert!(result.is_ok());
        
        let received = receiver.recv().unwrap();
        assert_eq!(received, b"hello");
    }

    #[test]
    fn test_tracking_notifier_failure() {
        let (notifier, _receiver) = TrackingNotifier::new();
        
        // Drop the receiver to simulate a disconnected channel
        drop(_receiver);
        
        let result = notifier.try_notify(b"test".as_slice());
        assert!(matches!(result, Err(NotifyError::SendFailed)));
    }

    #[test]
    fn test_conditional_notifier_never_fails() {
        let notifier = ConditionalNotifier::new(FailureMode::Never);
        
        assert!(notifier.try_notify(b"test1").is_ok());
        assert!(notifier.try_notify(b"test2").is_ok());
    }

    #[test]
    fn test_conditional_notifier_always_fails() {
        let notifier = ConditionalNotifier::new(FailureMode::Always);
        
        let result = notifier.try_notify(b"test");
        assert!(matches!(result, Err(NotifyError::SendFailed)));
    }

    #[test]
    fn test_conditional_notifier_payload_too_large() {
        let notifier = ConditionalNotifier::new(FailureMode::PayloadTooLarge(10));
        
        // Small payload should succeed
        assert!(notifier.try_notify(b"small").is_ok());
        
        // Large payload should fail
        let large_payload = vec![0u8; 20];
        let result = notifier.try_notify(large_payload);
        assert!(matches!(result, Err(NotifyError::PayloadTooLarge(20))));
    }

    #[test]
    fn test_conditional_notifier_disconnected() {
        let notifier = ConditionalNotifier::new(FailureMode::Disconnected);
        
        let result = notifier.try_notify(b"test");
        assert!(matches!(result, Err(NotifyError::Disconnected)));
    }

    #[test]
    fn test_conditional_notifier_unavailable() {
        let notifier = ConditionalNotifier::new(FailureMode::Unavailable);
        
        let result = notifier.try_notify(b"test");
        assert!(matches!(result, Err(NotifyError::Unavailable)));
    }

    #[test]
    fn test_conditional_notifier_fails_on_second_call() {
        let notifier = ConditionalNotifier::new(FailureMode::OnSecondCall);
        
        // First call should succeed
        assert!(notifier.try_notify(b"first").is_ok());
        
        // Second call should fail
        let result = notifier.try_notify(b"second");
        assert!(matches!(result, Err(NotifyError::SendFailed)));
    }

    #[test]
    fn test_notify_with_different_input_types() {
        let (notifier, receiver) = TrackingNotifier::new();
        
        // Test with &[u8]
        notifier.try_notify(b"slice".as_slice()).unwrap();
        assert_eq!(receiver.recv().unwrap(), b"slice");
        
        // Test with Vec<u8>
        notifier.try_notify(b"vector".to_vec()).unwrap();
        assert_eq!(receiver.recv().unwrap(), b"vector");
        
        // Test with Cow::Owned (easier to manage lifetimes in tests)
        notifier.try_notify(Cow::Owned(b"cow_owned".to_vec())).unwrap();
        assert_eq!(receiver.recv().unwrap(), b"cow_owned");
    }

    #[test]
    fn test_empty_payload_handling() {
        let (notifier, receiver) = TrackingNotifier::new();
        
        // Empty payload should still be processed
        notifier.try_notify(b"").unwrap();
        assert_eq!(receiver.recv().unwrap(), b"");
    }

    #[test]
    fn test_large_payload_handling() {
        let (notifier, receiver) = TrackingNotifier::new();
        
        // Create a large payload (1MB)
        let large_payload = vec![0xAB; 1024 * 1024];
        notifier.try_notify(large_payload.clone()).unwrap();
        
        let received = receiver.recv().unwrap();
        assert_eq!(received.len(), 1024 * 1024);
        assert_eq!(received[0], 0xAB);
        assert_eq!(received[received.len() - 1], 0xAB);
    }

    #[test]
    fn test_error_is_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        
        assert_send::<NotifyError>();
        assert_sync::<NotifyError>();
    }

    #[test]
    fn test_error_source_chain() {
        let error = NotifyError::SendFailed;
        // NotifyError doesn't wrap other errors, so source should be None
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn test_default_try_notify_implementation() {
        struct SimpleNotifier {
            last_notified: std::cell::RefCell<Vec<u8>>,
        }
        
        impl Notify for SimpleNotifier {
            fn notify<B: Into<Cow<'static, [u8]>>>(&self, bytes: B) {
                *self.last_notified.borrow_mut() = bytes.into().to_vec();
            }
        }
        
        let notifier = SimpleNotifier {
            last_notified: std::cell::RefCell::new(Vec::new()),
        };
        
        // Default try_notify should call notify and return Ok
        let result = notifier.try_notify(b"test_default");
        assert!(result.is_ok());
        assert_eq!(*notifier.last_notified.borrow(), b"test_default");
    }

    #[test]
    fn test_event_debug_formatting() {
        // Test various Event types for proper Debug formatting
        let event = Event::Bell;
        assert_eq!(format!("{:?}", event), "Bell");
        
        let event = Event::Title("Test Title".to_string());
        assert_eq!(format!("{:?}", event), "Title(Test Title)");
        
        let event = Event::ChildExit(42);
        assert_eq!(format!("{:?}", event), "ChildExit(42)");
        
        let event = Event::PtyWrite("hello".to_string());
        assert_eq!(format!("{:?}", event), "PtyWrite(hello)");
    }

    #[test]
    fn test_command_block_event_debug() {
        let event = CommandBlockEvent::PromptStart;
        assert_eq!(format!("{:?}", event), "PromptStart");
        
        let event = CommandBlockEvent::CommandStart { cmd: Some("ls -la".to_string()) };
        assert_eq!(format!("{:?}", event), "CommandStart { cmd: Some(\"ls -la\") }");
        
        let event = CommandBlockEvent::CommandEnd {
            exit: Some(0),
            cwd: Some("/home/user".to_string()),
        };
        assert_eq!(
            format!("{:?}", event),
            "CommandEnd { exit: Some(0), cwd: Some(\"/home/user\") }"
        );
    }

    #[test]
    fn test_window_size() {
        let window_size = WindowSize {
            num_lines: 24,
            num_cols: 80,
            cell_width: 8,
            cell_height: 16,
        };
        
        // Test that the struct can be created and accessed
        assert_eq!(window_size.num_lines, 24);
        assert_eq!(window_size.num_cols, 80);
        assert_eq!(window_size.cell_width, 8);
        assert_eq!(window_size.cell_height, 16);
        
        // Test that it's Copy and Clone
        let copied = window_size;
        let cloned = window_size.clone();
        assert_eq!(copied.num_lines, cloned.num_lines);
    }
}
