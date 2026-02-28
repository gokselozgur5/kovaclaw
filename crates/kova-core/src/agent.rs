use crate::event::{Message, Role};
use crate::llm::LlmClient;
use crate::session::Session;
use crate::tools::{ToolCall, ToolRegistry};
use anyhow::Result;
use tokio::io::AsyncWrite;

const MAX_TOOL_ROUNDS: usize = 10;

pub struct Agent {
    llm: LlmClient,
    system_prompt: String,
    history: Vec<Message>,
    pub tools: ToolRegistry,
    session: Option<Session>,
}

pub struct LoopResult {
    pub final_text: String,
    pub tool_log: Vec<ToolExecution>,
}

pub struct ToolExecution {
    pub name: String,
    pub args: serde_json::Value,
    pub output: String,
    pub success: bool,
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
                After receiving tool results, continue your response. You can chain multiple tool calls across rounds."
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

    /// Agent loop: send message, execute tool calls automatically, repeat until no more tool calls.
    /// auto_approve_fn decides per-tool whether to auto-approve.
    pub async fn run_loop<F>(
        &mut self,
        user_input: &str,
        auto_approve: F,
    ) -> Result<LoopResult>
    where
        F: Fn(&str) -> bool,
    {
        self.append(Message { role: Role::User, content: user_input.to_string() });

        let mut tool_log = Vec::new();

        for _ in 0..MAX_TOOL_ROUNDS {
            let messages = self.build_messages();
            let response = self.llm.chat(&messages).await?;
            self.append(Message { role: Role::Assistant, content: response.clone() });

            let calls = Self::parse_tool_calls(&response);
            if calls.is_empty() {
                return Ok(LoopResult {
                    final_text: clean_response(&response),
                    tool_log,
                });
            }

            for call in &calls {
                if !auto_approve(&call.name) {
                    self.feed_tool_result(&call.name, "Tool call denied (not in whitelist).");
                    tool_log.push(ToolExecution {
                        name: call.name.clone(),
                        args: call.arguments.clone(),
                        output: "denied".into(),
                        success: false,
                    });
                    continue;
                }

                match self.tools.execute(call) {
                    Ok(result) => {
                        self.feed_tool_result(&call.name, &result.output);
                        tool_log.push(ToolExecution {
                            name: call.name.clone(),
                            args: call.arguments.clone(),
                            output: result.output,
                            success: result.success,
                        });
                    }
                    Err(e) => {
                        let err = format!("Error: {e}");
                        self.feed_tool_result(&call.name, &err);
                        tool_log.push(ToolExecution {
                            name: call.name.clone(),
                            args: call.arguments.clone(),
                            output: err,
                            success: false,
                        });
                    }
                }
            }
            // Loop continues: LLM gets tool results and responds again
        }

        // Hit max rounds
        Ok(LoopResult {
            final_text: "[max tool rounds reached]".into(),
            tool_log,
        })
    }

    pub fn parse_tool_calls(response: &str) -> Vec<ToolCall> {
        let mut calls = Vec::new();

        // Try <tool_call> tags first
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
        if !calls.is_empty() {
            return calls;
        }

        // Fallback: try to find raw JSON with "name" and "arguments" keys
        for line in response.lines() {
            let line = line.trim();
            if line.starts_with('{') && line.contains("\"name\"") && line.contains("\"arguments\"") {
                if let Ok(call) = serde_json::from_str::<ToolCall>(line) {
                    calls.push(call);
                }
            }
        }
        if !calls.is_empty() {
            return calls;
        }

        // Fallback: try entire response as JSON (model sometimes returns just the JSON)
        let trimmed = response.trim().trim_start_matches("assistant").trim();
        if let Ok(call) = serde_json::from_str::<ToolCall>(trimmed) {
            calls.push(call);
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

pub fn clean_response(text: &str) -> String {
    let text = strip_tool_calls(text);
    // Strip leading "assistant" prefix that some models prepend
    let text = text.trim();
    let text = text.strip_prefix("assistant").unwrap_or(text).trim();
    text.to_string()
}

fn strip_tool_calls(text: &str) -> String {
    let mut result = String::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("<tool_call>") {
        result.push_str(&remaining[..start]);
        if let Some(end) = remaining[start..].find("</tool_call>") {
            remaining = &remaining[start + end + 12..];
        } else {
            break;
        }
    }
    result.push_str(remaining);
    result.trim().to_string()
}
