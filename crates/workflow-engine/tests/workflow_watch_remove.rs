use std::path::PathBuf;

#[tokio::test]
async fn test_workflow_engine_load_and_remove_by_path() {
    use workflow_engine::WorkflowEngine;

    // Create a temporary workflow file
    let dir = tempfile::tempdir().expect("tmpdir");
    let wf_path: PathBuf = dir.path().join("sample.yaml");
    let yaml = r#"name: Sample
version: "1.0.0"
description: Test wf
metadata: { tags: [], icon: null, estimated_duration: null }
requirements: []
parameters: []
environment: {}
steps:
  - id: s1
    name: Run
    commands: ["echo ok"]
hooks: {}
outputs: []
"#;
    tokio::fs::write(&wf_path, yaml).await.expect("write yaml");

    let engine = WorkflowEngine::new().expect("engine");

    // Load
    let id = engine.load_workflow(&wf_path).await.expect("load");
    assert!(id.starts_with("Sample-"));

    // Remove by path
    let removed = engine.remove_workflow_by_path(&wf_path).await;
    assert!(removed, "expected removal by path to succeed");

    // Removing again should return false
    let removed2 = engine.remove_workflow_by_path(&wf_path).await;
    assert!(!removed2);
}
