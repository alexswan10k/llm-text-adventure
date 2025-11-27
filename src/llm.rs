use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use crate::model::WorldUpdate;

use std::time::Duration;

#[derive(Clone)]
pub struct LlmClient {
    pub base_url: String,
    pub model_name: String,
    pub client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
struct LlmRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: i32,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

impl LlmClient {
    pub fn new(base_url: String, model_name: String) -> Self {
        Self {
            base_url,
            model_name,
            client: reqwest::ClientBuilder::new()
                .timeout(Duration::from_secs(600))
                .connect_timeout(Duration::from_secs(60))
                .build()
                .expect("Failed to build reqwest client"),
        }
    }

    pub async fn generate_update(&self, system_prompt: &str, user_input: &str) -> Result<WorldUpdate> {
        let request = LlmRequest {
            model: self.model_name.clone(),
            messages: vec![
                Message { role: "system".to_string(), content: system_prompt.to_string() },
                Message { role: "user".to_string(), content: user_input.to_string() },
            ],
            temperature: 0.7,
            max_tokens: -1,
            stream: false,
        };

        let response = self.client.post(&format!("{}/v1/chat/completions", self.base_url))
            .json(&request)
            .send()
            .await
            .context("Failed to send request to LLM")?;

        let response_json: serde_json::Value = response.json().await
            .context("Failed to parse LLM response JSON")?;

        let content = response_json["choices"][0]["message"]["content"].as_str()
            .context("No content in LLM response")?;

        // Parse the content to extract JSON and Narrative
        self.parse_content(content)
    }

    fn parse_content(&self, content: &str) -> Result<WorldUpdate> {
        // Try to find a JSON block
        let json_start = content.find('{');
        let json_end = content.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &content[start..=end];
            let update: WorldUpdate = serde_json::from_str(json_str)
                .context(format!("Failed to parse WorldUpdate from LLM content: {}", json_str))?;
            return Ok(update);
        }

        // Fallback if no JSON found (shouldn't happen with good prompt, but handle it)
        Err(anyhow::anyhow!("No JSON object found in LLM response"))
    }
}
