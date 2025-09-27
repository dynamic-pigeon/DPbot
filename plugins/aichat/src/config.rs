use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub system_prompt: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: "".to_string(),
            base_url: "".to_string(),
            model: "".to_string(),
            system_prompt: None,
        }
    }
}
