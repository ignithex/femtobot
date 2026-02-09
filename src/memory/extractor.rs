use std::collections::HashSet;
use std::sync::LazyLock;

use anyhow::{anyhow, Result};
use regex::Regex;
use serde::Deserialize;

use crate::memory::client::{ChatMessage, OpenRouterClient};

pub const FACT_KEYWORDS: &[&str] = &[
    "my name is",
    "i am",
    "i'm",
    "i work",
    "i live",
    "i prefer",
    "remember that",
    "note that",
    "important:",
    "email:",
    "phone:",
    "address:",
    "birthday:",
    "project uses",
    "using",
    "configured to",
];

const EXTRACTION_PROMPT: &str = r#"Analyze the conversation and extract key facts.

<conversation>
{conversation}
</conversation>

Extract:
- Personal info (name, job, location, preferences)
- Decisions, requirements, relationships
- Technical preferences (tools, languages)

Rules:
- Facts only, no opinions or temporary context
- Self-contained statements
- Skip greetings and small talk

Return JSON array: [{"fact": "...", "importance": "high|medium|low"}]
Example: [{"fact": "User's name is John", "importance": "high"}]

Facts:"#;

#[derive(Clone, Debug)]
pub struct ExtractedFact {
    pub content: String,
    pub importance: f32,
    pub source: String,
}

#[derive(Clone)]
pub struct MemoryExtractor {
    model: String,
    max_facts: usize,
    client: OpenRouterClient,
    trivial_patterns: Vec<Regex>,
}

impl MemoryExtractor {
    pub fn new(model: String, max_facts: usize, client: OpenRouterClient) -> Self {
        let patterns = [
            r"^(ok|okay|yes|no|thanks|sure|got it|cool|nice|great|hmm|ah|oh|lol|yep|yeah)[\.\!\?]?\s*$",
            r"^[\s\W]*$",
        ];
        let trivial_patterns = patterns.iter().filter_map(|p| Regex::new(p).ok()).collect();
        Self {
            model,
            max_facts,
            client,
            trivial_patterns,
        }
    }

    pub async fn extract(&self, messages: &[ChatMessage]) -> Vec<ExtractedFact> {
        if messages.is_empty() {
            return Vec::new();
        }

        let user_messages: Vec<&ChatMessage> =
            messages.iter().filter(|m| m.role == "user").collect();
        if user_messages.len() < 3 {
            return Vec::new();
        }

        if let Some(last_msg) = user_messages.last() {
            let trimmed = last_msg.content.trim();
            if trimmed.is_empty() || self.trivial_patterns.iter().any(|p| p.is_match(trimmed)) {
                return Vec::new();
            }
        }

        let conversation = format_conversation(messages);
        if conversation.len() < 50 {
            return Vec::new();
        }

        match self.llm_extract(&conversation).await {
            Ok(mut facts) => {
                facts.truncate(self.max_facts);
                facts
            }
            Err(_) => heuristic_extract(messages)
                .into_iter()
                .take(self.max_facts)
                .collect(),
        }
    }

    async fn llm_extract(&self, conversation: &str) -> Result<Vec<ExtractedFact>> {
        let prompt =
            EXTRACTION_PROMPT.replace("{conversation}", &sanitize_for_prompt(conversation));
        let response = self
            .client
            .chat_completion(
                &self.model,
                vec![ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                }],
                300,
                0.1,
                None,
            )
            .await?;
        let content = strip_code_fences(&response);
        let raw: Vec<ExtractedFactSchema> = serde_json::from_str(&content)
            .map_err(|e| anyhow!("invalid extraction response: {e}"))?;

        let mut extracted = Vec::new();
        for item in raw.into_iter().take(self.max_facts) {
            let importance = match item.importance.as_str() {
                "high" => 0.9,
                "low" => 0.3,
                _ => 0.7,
            };
            let content = item.fact.replace('<', "&lt;").replace('>', "&gt;");
            extracted.push(ExtractedFact {
                content,
                importance,
                source: "llm".to_string(),
            });
        }
        Ok(extracted)
    }
}

#[derive(Debug, Deserialize)]
struct ExtractedFactSchema {
    fact: String,
    #[serde(default = "default_importance")]
    importance: String,
}

fn default_importance() -> String {
    "medium".to_string()
}

pub fn extract_facts_from_messages(messages: &[ChatMessage], max_facts: usize) -> Vec<String> {
    let mut facts = Vec::new();
    let mut seen = HashSet::new();

    for msg in messages {
        if msg.role == "system" {
            continue;
        }
        for line in msg.content.lines() {
            let line = line.trim();
            if line.len() < 10 {
                continue;
            }
            if FACT_KEYWORDS
                .iter()
                .any(|kw| line.to_lowercase().contains(kw))
            {
                let fact = line.chars().take(200).collect::<String>();
                if seen.insert(fact.clone()) {
                    facts.push(fact);
                    if facts.len() >= max_facts {
                        return facts;
                    }
                }
            }
        }
    }
    facts
}

fn heuristic_extract(messages: &[ChatMessage]) -> Vec<ExtractedFact> {
    let patterns = [
        ("my name is", 0.9),
        ("i am a", 0.7),
        ("i work", 0.8),
        ("i live", 0.8),
        ("i prefer", 0.7),
        ("i like", 0.6),
        ("i use", 0.6),
        ("call me", 0.8),
    ];

    let mut facts = Vec::new();
    let mut seen = HashSet::new();

    for msg in messages {
        if msg.role != "user" {
            continue;
        }
        let content = msg.content.to_lowercase();
        for (indicator, importance) in patterns.iter() {
            if let Some(start) = content.find(indicator) {
                let end = [".", "!", "?", "\n"]
                    .iter()
                    .filter_map(|sep| content[start..].find(sep).map(|pos| pos + start))
                    .next()
                    .unwrap_or(content.len());
                let fact_text = content[start..end].trim();
                if fact_text.len() > 5 {
                    let mut fact = to_third_person(fact_text);
                    if let Some(first) = fact.get_mut(0..1) {
                        first.make_ascii_uppercase();
                    }
                    if seen.insert(fact.clone()) {
                        facts.push(ExtractedFact {
                            content: fact,
                            importance: *importance,
                            source: "heuristic".to_string(),
                        });
                    }
                }
            }
        }
    }

    facts
}

static THIRD_PERSON_RULES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"\bmy\b").unwrap(), "User's"),
        (Regex::new(r"\bi am\b").unwrap(), "User is"),
        (Regex::new(r"\bi'm\b").unwrap(), "User is"),
        (Regex::new(r"\bi have\b").unwrap(), "User has"),
        (Regex::new(r"\bi've\b").unwrap(), "User has"),
        (Regex::new(r"\bi will\b").unwrap(), "User will"),
        (Regex::new(r"\bi'll\b").unwrap(), "User will"),
        (Regex::new(r"\bi\b").unwrap(), "User"),
    ]
});

fn to_third_person(text: &str) -> String {
    let mut result = text.to_string();
    for (re, replacement) in THIRD_PERSON_RULES.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result = result.replace("User User", "User");
    result
}

fn sanitize_for_prompt(text: &str) -> String {
    let mut sanitized = text.replace("```", "'''");
    sanitized = sanitized.replace("</", "&lt;/");
    sanitized = sanitized.replace('<', "&lt;").replace('>', "&gt;");
    if sanitized.len() > 2000 {
        sanitized.truncate(2000);
        sanitized.push_str("...");
    }
    sanitized
}

fn format_conversation(messages: &[ChatMessage]) -> String {
    let mut parts = Vec::new();
    for msg in messages.iter().rev().take(20).rev() {
        if msg.role == "user" || msg.role == "assistant" {
            let mut content = sanitize_for_prompt(&msg.content);
            if content.len() > 500 {
                content.truncate(500);
            }
            parts.push(format!("{}: {}", msg.role.to_uppercase(), content));
        }
    }
    parts.join("\n")
}

fn strip_code_fences(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.starts_with("```") {
        let mut lines: Vec<&str> = trimmed.lines().collect();
        // Remove opening fence (e.g. ```json)
        lines.remove(0);
        // Remove closing fence if present
        if let Some(last) = lines.last() {
            if last.trim().starts_with("```") {
                lines.pop();
            }
        }
        return lines.join("\n").trim().to_string();
    }
    trimmed.to_string()
}
