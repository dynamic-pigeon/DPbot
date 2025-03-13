use serde::Deserialize;

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct Config {
    pub whitelist: Vec<i64>,
}

impl Config {
    pub fn empty() -> Self {
        Self { whitelist: vec![] }
    }
}
