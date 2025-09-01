// Workflow Parser Module

use super::*;
use anyhow::Result;

pub struct WorkflowParser;

impl WorkflowParser {
    pub fn new() -> Self {
        Self
    }
    
    pub fn parse_yaml(&self, content: &str) -> Result<WorkflowDefinition> {
        Ok(serde_yaml::from_str(content)?)
    }
    
    pub fn parse_toml(&self, content: &str) -> Result<WorkflowDefinition> {
        Ok(toml::from_str(content)?)
    }
}
