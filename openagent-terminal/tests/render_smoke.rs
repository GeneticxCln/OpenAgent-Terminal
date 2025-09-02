use std::process::Command;

fn example_bin() -> String {
    // Cargo sets this env var for example binaries during tests.
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_render_smoke") {
        return p;
    }
    // Fallback to common path if env var is not present (shouldn't happen under `cargo test`).
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // leave openagent-terminal/
    p.push("target");
    p.push("debug");
    p.push("examples");
    p.push(if cfg!(windows) { "render_smoke.exe" } else { "render_smoke" });
    p.to_string_lossy().into_owned()
}

#[test]
fn test_render_smoke_gl_backend_runs() {
    let output = Command::new(example_bin())
        .arg("--backend=gl")
        .output()
        .expect("failed to run render_smoke example");
    assert!(output.status.success(), "render_smoke(gl) exited with failure: {:?}", output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("backend:gl"), "expected backend:gl in output, got: {stdout}");
}

#[test]
fn test_render_smoke_wgpu_backend_runs_or_falls_back() {
    let output = Command::new(example_bin())
        .arg("--backend=wgpu")
        .output()
        .expect("failed to run render_smoke example");
    assert!(output.status.success(), "render_smoke(wgpu) exited with failure: {:?}", output);

    let stdout = String::from_utf8_lossy(&output.stdout);

    #[cfg(feature = "wgpu")]
    {
        // If wgpu is built, adapter may or may not be available; accept either.
        assert!(
            stdout.contains("backend:wgpu") || stdout.contains("backend:fallback-gl"),
            "expected backend:wgpu or backend:fallback-gl in output, got: {stdout}"
        );
    }

    #[cfg(not(feature = "wgpu"))]
    {
        assert!(
            stdout.contains("backend:fallback-gl"),
            "expected backend:fallback-gl when wgpu feature is disabled, got: {stdout}"
        );
    }
}

