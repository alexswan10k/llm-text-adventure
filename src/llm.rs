use serde::{Deserialize, Serialize};
use serde_json::Value;
use anyhow::{Result, Context};
use crate::model::WorldUpdate;

#[derive(Clone)]
pub struct LlmClient {
    pub base_url: String,
    pub model_name: String,
    pub client: reqwest::Client,
}

#[derive(Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

impl LlmClient {
    pub fn new(base_url: String, model_name: String) -> Self {
        Self {
            base_url,
            model_name,
            client: reqwest::Client::new(),
        }
    }

    pub async fn generate_update(&self, system_prompt: &str, user_prompt: &str) -> Result<WorldUpdate> {
        let request = OpenAIChatRequest {
            model: self.model_name.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_prompt.to_string(),
                },
            ],
            temperature: 0.7,
        };

        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to LLM")?;

        let response_json: OpenAIChatResponse = response.json().await.context("Failed to parse LLM response JSON")?;
        
        let content = response_json.choices.first()
            .context("No choices in LLM response")?
            .message.content.clone();

        // Parse the content. It might be wrapped in markdown code blocks ```json ... ```
        let json_str = extract_json(&content).unwrap_or(&content);
        
        let update: WorldUpdate = serde_json::from_str(json_str)
            .context(format!("Failed to parse WorldUpdate from LLM content: {}", content))?;

        Ok(update)
    }
}

fn extract_json(content: &str) -> Option<&str> {
    let start = content.find("```json")?;
    let end = content.rfind("```")?;
    if start < end {
        Some(&content[start + 7..end].trim())
    } else {
        None
    }
}
