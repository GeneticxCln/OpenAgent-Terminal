use crate::config::{Snippet, SnippetCollection};
use crate::SnippetSuggestion;
use anyhow::Result;
use fuzzy_matcher::FuzzyMatcher;
use std::collections::HashMap;

pub struct SnippetManager {
    snippets: HashMap<String, Snippet>,
    collections: HashMap<String, SnippetCollection>,
    fuzzy_matcher: fuzzy_matcher::skim::SkimMatcherV2,
}

impl std::fmt::Debug for SnippetManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnippetManager")
            .field("snippets", &self.snippets)
            .field("collections", &self.collections)
            .field("fuzzy_matcher", &"<SkimMatcherV2>")
            .finish()
    }
}

impl SnippetManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            snippets: HashMap::new(),
            collections: HashMap::new(),
            fuzzy_matcher: fuzzy_matcher::skim::SkimMatcherV2::default(),
        })
    }

    pub fn add_snippet(&mut self, snippet: Snippet) -> Result<()> {
        self.snippets.insert(snippet.id.clone(), snippet);
        Ok(())
    }

    pub fn find_by_trigger(&self, trigger: &str) -> Result<Vec<&Snippet>> {
        let matches = self.snippets.values()
            .filter(|snippet| {
                snippet.triggers.iter().any(|t| t.matches(trigger))
            })
            .collect();
        
        Ok(matches)
    }

    pub fn get_suggestions(&self, input: &str, limit: usize) -> Result<Vec<SnippetSuggestion>> {
        let mut suggestions = Vec::new();
        
        for snippet in self.snippets.values() {
            for trigger in &snippet.triggers {
                if let Some(score) = self.fuzzy_matcher.fuzzy_match(&trigger.pattern, input) {
                    suggestions.push(SnippetSuggestion {
                        snippet: snippet.clone(),
                        trigger_match: trigger.pattern.clone(),
                        score: score as f64,
                        context_relevance: 1.0, // Placeholder
                    });
                }
            }
        }

        suggestions.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        suggestions.truncate(limit);
        
        Ok(suggestions)
    }

    pub fn load_from_directory(&mut self, _dir: &std::path::Path) -> Result<()> {
        // Placeholder implementation
        Ok(())
    }

    pub fn save_to_file(&self, _path: &std::path::Path) -> Result<()> {
        // Placeholder implementation
        Ok(())
    }
}
