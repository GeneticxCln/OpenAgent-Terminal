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
#[derive(Debug)]
pub enum NotifyError {
    SendFailed,
}

pub trait Notify {
    /// Notify that an escape sequence should be written to the PTY.
    fn notify<B: Into<Cow<'static, [u8]>>>(&self, _: B);

    /// Fallible form of notify; default bridges to infallible notify and returns Ok(()).
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
