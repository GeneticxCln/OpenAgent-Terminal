// Workflow Validator Module

use super::*;
use anyhow::{anyhow, Result};

pub struct WorkflowValidator;

impl Default for WorkflowValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate(&self, workflow: &WorkflowDefinition) -> Result<()> {
        // Validate workflow name
        if workflow.name.is_empty() {
            return Err(anyhow!("Workflow name cannot be empty"));
        }

        // Validate version format
        if workflow.version.is_empty() {
            return Err(anyhow!("Workflow version cannot be empty"));
        }

        // Validate steps
        if workflow.steps.is_empty() {
            return Err(anyhow!("Workflow must have at least one step"));
        }

        // Check for duplicate step IDs
        let mut step_ids = std::collections::HashSet::new();
        for step in &workflow.steps {
            if !step_ids.insert(&step.id) {
                return Err(anyhow!("Duplicate step ID: {}", step.id));
            }

            if step.commands.is_empty() {
                return Err(anyhow!("Step {} must have at least one command", step.id));
            }
        }

        // Validate parameters
        let mut param_names = std::collections::HashSet::new();
        for param in &workflow.parameters {
            if !param_names.insert(&param.name) {
                return Err(anyhow!("Duplicate parameter name: {}", param.name));
            }
        }

        Ok(())
    }
}
