use log::{debug, warn};
use winit::raw_window_handle::RawDisplayHandle;

use openagent_terminal_core::term::ClipboardType;

use copypasta::nop_clipboard::NopClipboardContext;
#[cfg(all(feature = "wayland", not(any(target_os = "macos", windows))))]
use copypasta::wayland_clipboard;
#[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
use copypasta::x11_clipboard::{Primary as X11SelectionClipboard, X11ClipboardContext};
#[cfg(any(feature = "x11", target_os = "macos", windows))]
use copypasta::ClipboardContext;
use copypasta::ClipboardProvider;

pub struct Clipboard {
    clipboard: Box<dyn ClipboardProvider>,
    selection: Option<Box<dyn ClipboardProvider>>,
}

impl Clipboard {
    /// # Safety
    /// On Wayland, the caller must pass a valid RawDisplayHandle corresponding to the
    /// current process' Wayland display. The handle must remain valid for the duration of clipboard
    /// initialization. On other platforms, this function returns a default clipboard and does not
    /// dereference the handle.
    pub unsafe fn new(display: RawDisplayHandle) -> Self {
        match display {
            #[cfg(all(feature = "wayland", not(any(target_os = "macos", windows))))]
            RawDisplayHandle::Wayland(display) => {
                let (selection, clipboard) = unsafe {
                    wayland_clipboard::create_clipboards_from_external(display.display.as_ptr())
                };
                Self { clipboard: Box::new(clipboard), selection: Some(Box::new(selection)) }
            }
            _ => Self::default(),
        }
    }

    /// Used for tests, to handle missing clipboard provider when built without the `x11`
    /// feature, and as default clipboard value.
    pub fn new_nop() -> Self {
        Self { clipboard: Box::new(NopClipboardContext::new().unwrap()), selection: None }
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        #[cfg(any(target_os = "macos", windows))]
        {
            match ClipboardContext::new() {
                Ok(ctx) => Self { clipboard: Box::new(ctx), selection: None },
                Err(err) => {
                    warn!("Clipboard unavailable on this platform: {err}; falling back to Nop");
                    return Self::new_nop();
                }
            }
        }

        #[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
        {
            let clipboard = match ClipboardContext::new() {
                Ok(ctx) => Box::new(ctx) as Box<dyn ClipboardProvider>,
                Err(err) => {
                    warn!("X11 clipboard provider unavailable: {err}; using Nop clipboard");
                    Box::new(NopClipboardContext::new().expect("create nop clipboard"))
                }
            };
            let selection = match X11ClipboardContext::<X11SelectionClipboard>::new() {
                Ok(sel) => Some(Box::new(sel) as Box<dyn ClipboardProvider>),
                Err(err) => {
                    warn!(
                        "X11 selection provider unavailable: {err}; selection clipboard disabled"
                    );
                    None
                }
            };
            Self { clipboard, selection }
        }

        #[cfg(not(any(feature = "x11", target_os = "macos", windows)))]
        Self::new_nop()
    }
}

impl Clipboard {
    #[allow(clippy::needless_return)]
    fn log_backend_once(&self) {
        static ONCE: std::sync::Once = std::sync::Once::new();
        if std::env::var("OPENAGENT_CLIPBOARD_LOG").ok().as_deref() == Some("1") {
            ONCE.call_once(|| {
                #[cfg(all(feature = "wayland", not(any(target_os = "macos", windows))))]
                {
                    if self.selection.is_some() {
                        debug!("clipboard backend: Wayland (selection + clipboard)");
                        return;
                    }
                }
                #[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
                {
                    if self.selection.is_some() {
                        debug!("clipboard backend: X11 (selection + clipboard)");
                    } else {
                        debug!("clipboard backend: X11 (clipboard only)");
                    }
                    return;
                }
                #[cfg(target_os = "macos")]
                {
                    debug!("clipboard backend: macOS");
                    return;
                }
                #[cfg(windows)]
                {
                    debug!("clipboard backend: Windows");
                    return;
                }
                // Fallback
            });
        }
    }

    pub fn store(&mut self, ty: ClipboardType, text: impl Into<String>) {
        self.log_backend_once();
        let clipboard = match (ty, &mut self.selection) {
            (ClipboardType::Selection, Some(provider)) => provider,
            (ClipboardType::Selection, None) => return,
            _ => &mut self.clipboard,
        };

        clipboard.set_contents(text.into()).unwrap_or_else(|err| {
            warn!("Unable to store text in clipboard: {err}");
        });
    }

    pub fn load(&mut self, ty: ClipboardType) -> String {
        let clipboard = match (ty, &mut self.selection) {
            (ClipboardType::Selection, Some(provider)) => provider,
            _ => &mut self.clipboard,
        };

        match clipboard.get_contents() {
            Err(err) => {
                debug!("Unable to load text from clipboard: {err}");
                String::new()
            }
            Ok(text) => text,
        }
    }
}
