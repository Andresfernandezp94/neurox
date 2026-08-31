//! Working memory — conversation history with token-based compaction.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<crate::llm::ToolCallRequest>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

pub struct WorkingMemory {
    pub messages: Vec<ChatMessage>,
    pub context_summary: String,
    max_messages: usize,
    compaction_threshold_tokens: usize,
    keep_recent: usize,
}

impl WorkingMemory {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            context_summary: String::new(),
            max_messages: 200,
            compaction_threshold_tokens: 750_000,
            keep_recent: 20,
        }
    }

    /// Add a message to working memory. Trims oldest if over max_messages.
    pub fn add_message(&mut self, role: &str, content: Option<String>) {
        self.messages.push(ChatMessage {
            role: role.to_string(),
            content,
            tool_calls: None,
            tool_call_id: None,
        });
        if self.messages.len() > self.max_messages {
            self.messages.remove(0);
        }
    }

    /// Add a full ChatMessage (for tool calls, tool results, etc.)
    pub fn add_raw_message(&mut self, msg: ChatMessage) {
        self.messages.push(msg);
        if self.messages.len() > self.max_messages {
            self.messages.remove(0);
        }
    }

    /// Check if compaction is needed based on estimated token count.
    pub fn needs_compaction(&self) -> bool {
        self.estimated_tokens() >= self.compaction_threshold_tokens
    }

    /// Get text for compaction (all messages except last keep_recent).
    pub fn get_compaction_text(&self) -> String {
        if self.messages.len() <= self.keep_recent {
            return String::new();
        }
        let end = self.messages.len() - self.keep_recent;
        let mut text = String::new();
        for msg in &self.messages[..end] {
            let content = msg.content.as_deref().unwrap_or("");
            text.push_str(&format!("{}: {}\n", msg.role, content));
        }
        text
    }

    /// Apply compaction: save summary, keep only recent messages.
    pub fn apply_compaction(&mut self, summary: String) {
        self.context_summary = summary;
        if self.messages.len() > self.keep_recent {
            let start = self.messages.len() - self.keep_recent;
            self.messages = self.messages[start..].to_vec();
        }
        eprintln!(
            "[default] compaction applied, kept {} messages, summary {} chars",
            self.messages.len(),
            self.context_summary.len()
        );
    }

    /// Get all messages for LLM call (system prompt is NOT included here).
    pub fn get_messages_for_llm(&self) -> Vec<ChatMessage> {
        self.messages.clone()
    }

    /// Estimate token count (rough: total chars / 4).
    pub fn estimated_tokens(&self) -> usize {
        self.messages
            .iter()
            .map(|m| m.content.as_ref().map_or(0, |c| c.len()) / 4)
            .sum()
    }

    /// Count user messages (turn count).
    pub fn turn_count(&self) -> usize {
        self.messages.iter().filter(|m| m.role == "user").count()
    }

    /// Clear all memory (reset session).
    pub fn clear(&mut self) {
        self.messages.clear();
        self.context_summary.clear();
    }
}
