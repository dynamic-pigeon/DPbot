use std::collections::VecDeque;

use anyhow::Result;
use kovi::serde_json;
use serde::{Deserialize, Serialize};

use crate::config::Config;

pub struct Chat {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

#[derive(Deserialize, Serialize)]
pub struct ChatBody {
    model: String,
    messages: VecDeque<Message>,
    stream: bool,
}

#[derive(Deserialize, Serialize)]
struct Message {
    role: String,
    content: String,
}

impl ChatBody {
    pub fn new(model: String) -> Self {
        Self {
            model,
            messages: VecDeque::new(),
            stream: false,
        }
    }

    fn add_message(&mut self, msg: Message) {
        self.messages.push_back(msg);
        if self.messages.len() > 16 {
            self.remove_message(2);
        }
    }

    fn pop_message(&mut self) {
        self.messages.pop_back();
    }

    fn remove_message(&mut self, cnt: usize) {
        for _ in 0..cnt {
            if self.messages.is_empty() {
                break;
            }
            self.messages.pop_front();
        }
    }
}

impl Chat {
    pub fn new(api_key: String, base_url: String) -> Self {
        let client = reqwest::Client::new();
        Self {
            client,
            api_key,
            base_url,
        }
    }

    pub fn from_config(config: Config) -> Self {
        Self::new(config.api_key, config.base_url)
    }

    pub async fn chat(&self, content: String, msgs: &mut ChatBody) -> Result<String> {
        let message = Message {
            role: "user".to_string(),
            content,
        };

        msgs.add_message(message);

        let response = match self
            .client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(msgs)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                msgs.pop_message();
                return Err(e.into());
            }
        };

        let res: serde_json::Value = response.json().await.unwrap();
        let reply = match res["choices"][0]["message"]["content"].as_str() {
            Some(reply) => reply,
            None => {
                msgs.pop_message();
                return Err(anyhow::anyhow!("No reply found"));
            }
        };

        msgs.add_message(Message {
            role: "system".to_string(),
            content: reply.to_string(),
        });

        Ok(reply.to_string())
    }
}
