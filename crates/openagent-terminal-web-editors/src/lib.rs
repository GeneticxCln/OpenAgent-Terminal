//! Native WebView editor (Monaco) integration for OpenAgent Terminal (Linux: WebKitGTK via wry)

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tao::event_loop::EventLoop;
use tao::window::WindowBuilder;
use wry::http::Request;
use wry::WebViewBuilder;

pub struct WebEditorConfig {
    pub file_path: PathBuf,
    pub title: Option<String>,
    pub prefer_monaco: bool,
}

#[allow(unreachable_code)]
pub fn open_editor_blocking(cfg: WebEditorConfig) -> Result<()> {
    // Read initial content
    let initial = fs::read_to_string(&cfg.file_path).unwrap_or_default();
    let lang = guess_language_from_path(&cfg.file_path);

    // HTML content loading Monaco from CDN (native window, no browser)
    let html = build_monaco_html(&initial, &lang);

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(
            cfg.title
                .unwrap_or_else(|| format!("Editor - {}", cfg.file_path.display())),
        )
        .with_inner_size(tao::dpi::PhysicalSize::new(1000, 700))
        .build(&event_loop)?;

    let file_path = cfg.file_path.clone();
    let webview = WebViewBuilder::new(&window)
        .with_initialization_script("window.__OPENAGENT__ = { save: () => {}, };")
        .with_html(html)
        .with_ipc_handler(move |req: Request<String>| {
            // Expect JSON messages like {"type":"save","content":"..."}
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(req.body()) {
                if let Some(t) = v.get("type").and_then(|x| x.as_str()) {
                    if t == "save" {
                        if let Some(content) = v.get("content").and_then(|x| x.as_str()) {
                            let _ = fs::write(&file_path, content);
                        }
                    }
                }
            }
        })
        .build()?;

    // Keep webview alive and block until window closes
    let _keep_alive = webview;
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::ControlFlow;
    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
    Ok(())
}

fn guess_language_from_path(path: &Path) -> String {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "rs" => "rust",
        "ts" => "typescript",
        "tsx" => "typescript",
        "js" => "javascript",
        "jsx" => "javascript",
        "json" => "json",
        "md" => "markdown",
        "py" => "python",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" => "cpp",
        other => {
            if other.is_empty() {
                "plaintext"
            } else {
                other
            }
        }
    }
    .into()
}

fn escape_js(s: &str) -> String {
    let mut escaped = String::new();
    for c in s.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            _ => escaped.push(c),
        }
    }
    escaped
}

fn build_monaco_html(initial_content: &str, language: &str) -> String {
    let content = escape_js(initial_content);
    let lang = language;
    // Minimal Monaco loader via unpkg
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<style>
  html, body, #container {{ height: 100%; margin: 0; padding: 0; }}
</style>
</head>
<body>
<div id="container"></div>
<script>
  const SAVE_KEY = (e) => (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's';
  function post(msg) {{ window.ipc.postMessage(JSON.stringify(msg)); }}
</script>
<script src="https://unpkg.com/monaco-editor@0.44.0/min/vs/loader.js"></script>
<script>
  require.config({{ paths: {{ 'vs': 'https://unpkg.com/monaco-editor@0.44.0/min/vs' }} }});
  window.MonacoEnvironment = {{ getWorkerUrl: () => `data:text/javascript;charset=utf-8,` + encodeURIComponent(`
    self.MonacoEnvironment = {{ baseUrl: 'https://unpkg.com/monaco-editor@0.44.0/min/' }};
    importScripts('https://unpkg.com/monaco-editor@0.44.0/min/vs/base/worker/workerMain.js');
  `)}};
  require(['vs/editor/editor.main'], function() {{
    const editor = monaco.editor.create(document.getElementById('container'), {{
      value: "{content}",
      language: '{lang}',
      theme: 'vs-dark',
      automaticLayout: true,
      minimap: {{ enabled: false }}
    }});
    window.addEventListener('keydown', function(e) {{
      if (SAVE_KEY(e)) {{
        e.preventDefault();
        const text = editor.getValue();
        post({{ type: 'save', content: text }});
      }}
    }});
  }});
</script>
</body>
</html>
"#
    )
}
