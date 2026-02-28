use crate::config::LlmConfig;
use crate::event::{Message, Role};
use crate::tools::ToolDef;
use anyhow::Result;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWrite;

pub struct LlmClient {
    client: reqwest::Client,
    config: LlmConfig,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolSchema>>,
}

#[derive(Serialize)]
struct ToolSchema {
    r#type: String,
    function: ToolFunction,
}

#[derive(Serialize)]
struct ToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }

    fn build_request(&self, messages: &[Message], tools: Option<&[ToolDef]>, stream: bool) -> ChatRequest {
        let chat_messages = messages.iter().map(|m| ChatMessage {
            role: match m.role {
                Role::System => "system".into(),
                Role::User => "user".into(),
                Role::Assistant => "assistant".into(),
            },
            content: m.content.clone(),
        }).collect();

        let tool_schemas = tools.map(|defs| {
            defs.iter().map(|t| ToolSchema {
                r#type: "function".into(),
                function: ToolFunction {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            }).collect()
        });

        ChatRequest {
            model: self.config.model.clone(),
            messages: chat_messages,
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            stream,
            tools: tool_schemas,
        }
    }

    pub async fn chat(&self, messages: &[Message]) -> Result<String> {
        self.chat_with_tools(messages, None).await
    }

    pub async fn chat_with_tools(&self, messages: &[Message], tools: Option<&[ToolDef]>) -> Result<String> {
        let request = self.build_request(messages, tools, false);
        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let resp = self.client.post(&url).json(&request).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM request failed ({}): {}", status, body);
        }

        let chat_resp: ChatResponse = resp.json().await?;
        chat_resp.choices.first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("Empty response from LLM"))
    }

    pub async fn chat_stream<W: AsyncWrite + Unpin>(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDef]>,
        writer: &mut W,
    ) -> Result<String> {
        let request = self.build_request(messages, tools, true);
        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let resp = self.client.post(&url).json(&request).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM request failed ({}): {}", status, body);
        }

        let mut full_response = String::new();
        let mut stream = resp.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                let line = line.trim();
                if !line.starts_with("data: ") { continue; }
                let data = &line[6..];
                if data == "[DONE]" { break; }
                if let Ok(parsed) = serde_json::from_str::<StreamChunk>(data) {
                    for choice in &parsed.choices {
                        if let Some(content) = &choice.delta.content {
                            full_response.push_str(content);
                            use tokio::io::AsyncWriteExt;
                            writer.write_all(content.as_bytes()).await?;
                            writer.flush().await?;
                        }
                    }
                }
            }
        }

        Ok(full_response)
    }
}
