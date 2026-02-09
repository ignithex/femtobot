use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;
use tracing::warn;

use crate::memory::client::{ChatMessage, OpenRouterClient, ResponseFormat};
use crate::memory::extractor::ExtractedFact;
use crate::memory::vector_store::{MemoryItem, VectorMemoryStore};

#[derive(Clone, Debug)]
pub enum Operation {
    Add,
    Update,
    Delete,
    Noop,
}

#[derive(Clone, Debug)]
pub struct ConsolidationResult {
    pub operation: Operation,
    pub memory_id: Option<String>,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
    pub similarity: f32,
    pub reason: String,
}

#[derive(Deserialize)]
struct ConsolidationDecision {
    operation: String,
    #[serde(default)]
    memory_id: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Clone)]
pub struct MemoryConsolidator {
    store: VectorMemoryStore,
    model: String,
    candidate_threshold: f32,
    client: OpenRouterClient,
}

impl MemoryConsolidator {
    pub fn new(
        store: VectorMemoryStore,
        model: String,
        client: OpenRouterClient,
        candidate_threshold: f32,
    ) -> Self {
        Self {
            store,
            model,
            client,
            candidate_threshold,
        }
    }

    pub async fn consolidate(
        &self,
        facts: Vec<ExtractedFact>,
        namespace: &str,
    ) -> Vec<ConsolidationResult> {
        let mut results = Vec::new();
        for fact in facts {
            if fact.content.trim().len() < 5 {
                continue;
            }
            let fact_source = fact.source.clone();
            let (result, valid_ids) = self
                .consolidate_single(fact.content.trim(), namespace)
                .await
                .unwrap_or_else(|e| {
                    warn!("LLM decision failed: {}", e);
                    (
                        ConsolidationResult {
                            operation: Operation::Add,
                            memory_id: None,
                            old_content: None,
                            new_content: Some(fact.content.clone()),
                            similarity: 0.0,
                            reason: "LLM failed".to_string(),
                        },
                        vec![],
                    )
                });
            results.push(result.clone());

            let importance = if fact.importance.is_finite() {
                fact.importance.clamp(0.0, 1.0)
            } else {
                0.5
            };

            if let Err(err) = self
                .execute_operation(&result, namespace, importance, &valid_ids)
                .await
            {
                warn!("Failed to execute operation: {}", err);
            } else {
                tracing::debug!("memory operation applied from source={}", fact_source);
            }
        }
        results
    }

    async fn consolidate_single(
        &self,
        fact: &str,
        namespace: &str,
    ) -> Result<(ConsolidationResult, Vec<String>)> {
        let similar = self
            .store
            .search(fact, 3, self.candidate_threshold, Some(namespace), 0.3)
            .await?;
        let valid_ids: Vec<String> = similar.iter().map(|(item, _)| item.id.clone()).collect();
        if similar.is_empty() {
            return Ok((
                ConsolidationResult {
                    operation: Operation::Add,
                    memory_id: None,
                    old_content: None,
                    new_content: Some(fact.to_string()),
                    similarity: 0.0,
                    reason: "No similar memories found".to_string(),
                },
                valid_ids,
            ));
        }
        let decision = self.llm_decide_operation(fact, &similar).await?;
        Ok((decision, valid_ids))
    }

    async fn llm_decide_operation(
        &self,
        fact: &str,
        candidates: &[(MemoryItem, f32)],
    ) -> Result<ConsolidationResult> {
        let candidates_text = candidates
            .iter()
            .enumerate()
            .map(|(i, (item, score))| {
                format!(
                    "{}. [id: {}] \"{}\" (similarity: {:.2})",
                    i + 1,
                    item.id,
                    sanitize_content(&item.content),
                    score
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Memory management decision.\n\nExisting memories:\n{}\n\nNew fact: \"{}\"\n\nOperations:\n- ADD: Completely new information\n- UPDATE <id>: Update/replace existing (provide merged content)\n- DELETE <id>: Contradicts existing (provide new content)\n- NOOP: Already captured\n\nJSON format: {{\"operation\": \"UPDATE\", \"memory_id\": \"abc123\", \"content\": \"merged\", \"reason\": \"...\"}}\nFor ADD/NOOP, omit memory_id. For UPDATE, MUST provide merged content.\n\nResponse:",
            candidates_text,
            sanitize_content(fact)
        );

        let response = self
            .client
            .chat_completion(
                &self.model,
                vec![ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                }],
                500,
                0.0,
                Some(ResponseFormat {
                    kind: "json_object".to_string(),
                }),
            )
            .await?;

        let decision: ConsolidationDecision = serde_json::from_str(&response)
            .map_err(|e| anyhow!("invalid consolidation response: {e}"))?;

        let operation = match decision.operation.to_uppercase().as_str() {
            "UPDATE" => Operation::Update,
            "DELETE" => Operation::Delete,
            "NOOP" => Operation::Noop,
            _ => Operation::Add,
        };

        let mut result = ConsolidationResult {
            operation: operation.clone(),
            memory_id: decision.memory_id.clone(),
            old_content: None,
            new_content: decision.content.clone().or_else(|| Some(fact.to_string())),
            similarity: candidates.first().map(|c| c.1).unwrap_or(0.0),
            reason: decision
                .reason
                .unwrap_or_else(|| "LLM decision".to_string()),
        };

        if matches!(operation, Operation::Update | Operation::Delete) {
            if let Some(memory_id) = &decision.memory_id {
                if let Some((item, score)) =
                    candidates.iter().find(|(item, _)| &item.id == memory_id)
                {
                    result.old_content = Some(item.content.clone());
                    result.similarity = *score;
                } else {
                    result.operation = Operation::Add;
                    result.memory_id = None;
                    result.reason = "Invalid memory_id".to_string();
                }
            } else {
                result.operation = Operation::Add;
                result.reason = "Missing memory_id".to_string();
            }
        }

        Ok(result)
    }

    async fn execute_operation(
        &self,
        result: &ConsolidationResult,
        namespace: &str,
        importance: f32,
        valid_ids: &[String],
    ) -> Result<()> {
        let mut base_metadata = HashMap::new();
        base_metadata.insert("importance".to_string(), Value::from(importance));

        match result.operation {
            Operation::Add => {
                if let Some(content) = &result.new_content {
                    let _ = self
                        .store
                        .add(
                            &sanitize_storage_content(content),
                            base_metadata.clone(),
                            Some(namespace),
                        )
                        .await?;
                }
            }
            Operation::Update => {
                if let (Some(id), Some(content)) = (&result.memory_id, &result.new_content) {
                    if !valid_ids.contains(id) {
                        return Ok(());
                    }
                    let updated = self
                        .store
                        .update(
                            id,
                            &sanitize_storage_content(content),
                            base_metadata.clone(),
                            Some(namespace),
                        )
                        .await?;
                    if updated.is_none() {
                        let _ = self
                            .store
                            .add(
                                &sanitize_storage_content(content),
                                base_metadata.clone(),
                                Some(namespace),
                            )
                            .await?;
                    }
                }
            }
            Operation::Delete => {
                if let Some(id) = &result.memory_id {
                    if !valid_ids.contains(id) {
                        return Ok(());
                    }
                    let _ = self.store.delete(id, Some(namespace)).await?;
                }
                if let Some(content) = &result.new_content {
                    if let Some(old) = &result.old_content {
                        if content == old {
                            return Ok(());
                        }
                    }
                    let _ = self
                        .store
                        .add(
                            &sanitize_storage_content(content),
                            base_metadata.clone(),
                            Some(namespace),
                        )
                        .await?;
                }
            }
            Operation::Noop => {}
        }
        Ok(())
    }
}

fn sanitize_content(text: &str) -> String {
    let mut sanitized = text.replace('"', "\\\"").replace('\n', " ");
    if sanitized.len() > 500 {
        sanitized.truncate(500);
    }
    sanitized
}

fn sanitize_storage_content(text: &str) -> String {
    text.replace('<', "&lt;").replace('>', "&gt;")
}
