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
            max_tokens: 16384,
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

    pub async fn send_chat_request(&self, request: &crate::agent::LlmRequest) -> Result<serde_json::Value> {
        let response = tokio::time::timeout(
            Duration::from_secs(55),
            self.client.post(&format!("{}/v1/chat/completions", self.base_url))
                .json(request)
                .send()
        ).await
        .context("LLM request timed out after 55 seconds")?
        .context("Failed to send request to LLM")?;

        let response_json: serde_json::Value = response.json().await
            .context("Failed to parse LLM response JSON")?;

        let message = response_json["choices"][0]["message"].clone();
        Ok(message)
    }

    pub fn parse_content(&self, content: &str) -> Result<WorldUpdate> {
        let cleaned_content = content.trim();

        if !self.is_complete_json(cleaned_content) {
            return Err(anyhow::anyhow!("LLM response JSON appears incomplete (mismatched braces/brackets or unclosed string). Content: {}...", &cleaned_content[..cleaned_content.len().min(200)]));
        }

        let json_start = cleaned_content.find('{');
        let json_end = cleaned_content.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &cleaned_content[start..=end];
            let update: WorldUpdate = serde_json::from_str(json_str)
                .context(format!("Failed to parse WorldUpdate from LLM content. JSON: {}", json_str))?;
            return Ok(update);
        }

        Err(anyhow::anyhow!("No JSON object found in LLM response. Content: {}", cleaned_content))
    }

    pub fn parse_location_json(&self, content: &str) -> Result<Location> {
        let cleaned_content = content.trim();

        if !self.is_complete_json(cleaned_content) {
            return Err(anyhow::anyhow!(
                "LLM response JSON appears incomplete (mismatched braces/brackets or unclosed string).\n\
                First 300 chars: {}\n\
                This usually means the LLM response was truncated. Try reducing max_tokens or the prompt length.",
                &cleaned_content[..cleaned_content.len().min(300)]
            ));
        }

        let json_start = cleaned_content.find('{');
        let json_end = cleaned_content.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &cleaned_content[start..=end];

            match serde_json::from_str::<Location>(json_str) {
                Ok(mut loc) => {
                    loc.visited = false;
                    Ok(loc)
                }
                Err(e) => {
                    let mut missing_fields = Vec::new();
                    let json_value: serde_json::Value = serde_json::from_str(json_str).unwrap_or(serde_json::Value::Null);

                    if json_value.get("name").is_none() { missing_fields.push("name"); }
                    if json_value.get("description").is_none() { missing_fields.push("description"); }
                    if json_value.get("exits").is_none() { missing_fields.push("exits"); }
                    if json_value.get("items").is_none() { missing_fields.push("items"); }
                    if json_value.get("actors").is_none() { missing_fields.push("actors"); }

                    let error_msg = if !missing_fields.is_empty() {
                        format!(
                            "Missing required fields: {}. JSON: {}",
                            missing_fields.join(", "),
                            json_str
                        )
                    } else {
                        format!("Failed to parse Location JSON. Error: {}. JSON: {}", e, json_str)
                    };

                    Err(anyhow::anyhow!(error_msg))
                }
            }
        } else {
            Err(anyhow::anyhow!(
                "No JSON object found in LLM response.\n\
                First 300 chars: {}",
                &cleaned_content[..cleaned_content.len().min(300)]
            ))
        }
    }

    pub fn is_complete_json(&self, content: &str) -> bool {
        let mut brace_count = 0;
        let mut bracket_count = 0;
        let mut in_string = false;
        let mut i = 0;
        let chars: Vec<char> = content.chars().collect();

        while i < chars.len() {
            let ch = chars[i];

            if ch == '"' {
                let mut backslash_count = 0;
                let mut j = i;
                while j > 0 && chars[j - 1] == '\\' {
                    backslash_count += 1;
                    j -= 1;
                }

                if backslash_count % 2 == 0 {
                    in_string = !in_string;
                }
            } else if !in_string {
                if ch == '{' {
                    brace_count += 1;
                } else if ch == '}' {
                    brace_count -= 1;
                    if brace_count < 0 {
                        return false;
                    }
                } else if ch == '[' {
                    bracket_count += 1;
                } else if ch == ']' {
                    bracket_count -= 1;
                    if bracket_count < 0 {
                        return false;
                    }
                }
            }

            i += 1;

            if i > 0 && i % 1000 == 0 {
                if brace_count < 0 || bracket_count < 0 {
                    return false;
                }
            }
        }

        brace_count == 0 && bracket_count == 0 && !in_string
    }
}
