use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventPayload {
    MessageIn { source: Source, text: String },
    LlmRequest { messages: Vec<Message> },
    LlmResponse { content: String },
    ToolRequest { name: String, args: serde_json::Value },
    ToolResult { name: String, output: String, success: bool },
    MessageOut { target: Source, text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Source {
    Cli,
    WhatsApp { jid: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Event {
    pub fn new(payload: EventPayload) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            payload,
        }
    }
}
