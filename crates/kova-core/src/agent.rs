use crate::event::{Message, Role};
use crate::llm::LlmClient;
use anyhow::Result;

pub struct Agent {
    llm: LlmClient,
    system_prompt: String,
    history: Vec<Message>,
}

impl Agent {
    pub fn new(llm: LlmClient, system_prompt: String) -> Self {
        Self {
            llm,
            system_prompt,
            history: Vec::new(),
        }
    }

    pub async fn send(&mut self, user_input: &str) -> Result<String> {
        self.history.push(Message {
            role: Role::User,
            content: user_input.to_string(),
        });

        let mut messages = vec![Message {
            role: Role::System,
            content: self.system_prompt.clone(),
        }];
        messages.extend(self.history.clone());

        let response = self.llm.chat(&messages).await?;

        self.history.push(Message {
            role: Role::Assistant,
            content: response.clone(),
        });

        Ok(response)
    }
}
