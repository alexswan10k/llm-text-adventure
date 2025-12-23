use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use crate::model::{WorldUpdate, Location};

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
                .timeout(Duration::from_secs(60))
                .connect_timeout(Duration::from_secs(15))
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
            max_tokens: 8192,
            stream: false,
        };

        let response = tokio::time::timeout(
            Duration::from_secs(55),
            self.client.post(&format!("{}/v1/chat/completions", self.base_url))
                .json(&request)
                .send()
        ).await
        .context("LLM request timed out after 55 seconds")?
        .context("Failed to send request to LLM")?;

        let response_json: serde_json::Value = response.json().await
            .context("Failed to parse LLM response JSON")?;

        let content = response_json["choices"][0]["message"]["content"].as_str()
            .context("No content in LLM response")?;

        self.parse_content(content)
    }

    pub async fn generate_location(&self, system_prompt: &str, user_input: &str) -> Result<Location> {
        let request = LlmRequest {
            model: self.model_name.clone(),
            messages: vec![
                Message { role: "system".to_string(), content: system_prompt.to_string() },
                Message { role: "user".to_string(), content: user_input.to_string() },
            ],
            temperature: 0.8,
            max_tokens: 4096,
            stream: false,
        };

        let response = tokio::time::timeout(
            Duration::from_secs(55),
            self.client.post(&format!("{}/v1/chat/completions", self.base_url))
                .json(&request)
                .send()
        ).await
        .context("LLM request timed out after 55 seconds")?
        .context("Failed to send request to LLM")?;

        let response_json: serde_json::Value = response.json().await
            .context("Failed to parse LLM response JSON")?;

        let content = response_json["choices"][0]["message"]["content"].as_str()
            .context("No content in LLM response")?;

        self.parse_location_json(content)
    }

    fn parse_content(&self, content: &str) -> Result<WorldUpdate> {
        let json_start = content.find('{');
        let json_end = content.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &content[start..=end];
            let update: WorldUpdate = serde_json::from_str(json_str)
                .context(format!("Failed to parse WorldUpdate from LLM content: {}", json_str))?;
            return Ok(update);
        }

        Err(anyhow::anyhow!("No JSON object found in LLM response"))
    }

    fn parse_location_json(&self, content: &str) -> Result<Location> {
        let json_start = content.find('{');
        let json_end = content.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &content[start..=end];
            let mut loc: Location = serde_json::from_str(json_str)
                .context(format!("Failed to parse Location from LLM response: {}", json_str))?;
            loc.visited = false;
            return Ok(loc);
        }

        Err(anyhow::anyhow!("No JSON object found in LLM response"))
    }
}
