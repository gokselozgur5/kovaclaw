use crate::event::{Message, Role};
use crate::llm::LlmClient;
use crate::session::Session;
use crate::tools::{ToolCall, ToolRegistry};
use anyhow::Result;
use tokio::io::AsyncWrite;

pub struct Agent {
    llm: LlmClient,
    system_prompt: String,
    history: Vec<Message>,
    pub tools: ToolRegistry,
    session: Option<Session>,
}

impl Agent {
    pub fn new(llm: LlmClient, system_prompt: String) -> Self {
        let mut tools = ToolRegistry::new();
        tools.register_defaults();
        Self {
            llm,
            system_prompt,
            history: Vec::new(),
            tools,
            session: None,
        }
    }

    pub fn with_session(mut self, session: Session) -> Result<Self> {
        self.history = session.load()?;
        self.session = Some(session);
        Ok(self)
    }

    fn build_messages(&self) -> Vec<Message> {
        let tool_defs = self.tools.definitions();
        let tools_desc = if tool_defs.is_empty() {
            String::new()
        } else {
            let tools_json = serde_json::to_string_pretty(&tool_defs).unwrap_or_default();
            format!(
                "\n\nYou have access to these tools. To call a tool, respond with EXACTLY this format:\n\
                <tool_call>\n{{\"name\": \"tool_name\", \"arguments\": {{...}}}}\n</tool_call>\n\n\
                Available tools:\n{tools_json}\n\n\
                After receiving tool results, continue your response."
            )
        };

        let mut messages = vec![Message {
            role: Role::System,
            content: format!("{}{}", self.system_prompt, tools_desc),
        }];
        messages.extend(self.history.clone());
        messages
    }

    fn append(&mut self, msg: Message) {
        if let Some(ref session) = self.session {
            let _ = session.append(&msg);
        }
        self.history.push(msg);
    }

    pub async fn send(&mut self, user_input: &str) -> Result<String> {
        self.append(Message { role: Role::User, content: user_input.to_string() });
        let messages = self.build_messages();
        let response = self.llm.chat(&messages).await?;
        self.append(Message { role: Role::Assistant, content: response.clone() });
        Ok(response)
    }

    pub async fn send_stream<W: AsyncWrite + Unpin>(
        &mut self,
        user_input: &str,
        writer: &mut W,
    ) -> Result<String> {
        self.append(Message { role: Role::User, content: user_input.to_string() });
        let messages = self.build_messages();
        let response = self.llm.chat_stream(&messages, None, writer).await?;
        self.append(Message { role: Role::Assistant, content: response.clone() });
        Ok(response)
    }

    pub fn parse_tool_calls(response: &str) -> Vec<ToolCall> {
        let mut calls = Vec::new();
        let mut remaining = response;
        while let Some(start) = remaining.find("<tool_call>") {
            if let Some(end) = remaining[start..].find("</tool_call>") {
                let json_str = &remaining[start + 11..start + end].trim();
                if let Ok(call) = serde_json::from_str::<ToolCall>(json_str) {
                    calls.push(call);
                }
                remaining = &remaining[start + end + 12..];
            } else {
                break;
            }
        }
        calls
    }

    pub fn feed_tool_result(&mut self, name: &str, output: &str) {
        self.append(Message {
            role: Role::User,
            content: format!("<tool_result>\n{{\"name\": \"{name}\", \"output\": {}}}\n</tool_result>",
                serde_json::to_string(output).unwrap_or_else(|_| format!("\"{output}\""))),
        });
    }
}
