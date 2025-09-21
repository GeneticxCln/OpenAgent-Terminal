#![allow(clippy::pedantic)]

// High-yield coverage: command_pipeline utility behavior
// These tests avoid spawning real shells; they focus on basic struct lifecycle.

use openagent_terminal::command_pipeline::CommandPipeline;

#[test]
fn pipeline_starts_empty() {
    let pipeline = CommandPipeline::new();
    assert_eq!(pipeline.active_command_count(), 0);
}
