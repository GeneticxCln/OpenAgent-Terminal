//! GTK4 + WebKit (webkit6) webview window for Linux.

use crate::IdeResult;

#[cfg(feature = "web-editors")]
pub fn open_webview_blocking(url: &str, title: &str, width: u32, height: u32) -> IdeResult<()> {
    use gtk4::prelude::*;
    use gtk4::{Application, ApplicationWindow};
    use webkit6::prelude::WebViewExt;
    use webkit6::WebView;

    // Use a stable application ID for DBus integration and single-instance semantics if desired.
    let app_id = "com.openagent.terminal.ide";
    let app = Application::builder().application_id(app_id).build();

    let url = url.to_owned();
    let title = title.to_owned();

    app.connect_activate(move |app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title(title.clone())
            .default_width(width as i32)
            .default_height(height as i32)
            .build();

        let webview = WebView::new();
        webview.load_uri(&url);
        window.set_child(Some(&webview));

        window.present();
    });

    // Run the GTK application main loop (blocking until the window is closed).
    // Exit code is ignored by the caller; errors are surfaced during construction.
    let _exit_code = app.run();

    Ok(())
}
