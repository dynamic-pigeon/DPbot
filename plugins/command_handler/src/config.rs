use std::sync::LazyLock;

use kovi::serde_json::{self, Value};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct Config {
    pub py_analyzer_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            py_analyzer_path: "python3".to_string(),
        }
    }
}

pub static COMMAND: LazyLock<Value> = LazyLock::new(|| {
    serde_json::json!({
        "duel": {
            "challenge": "challenge",
            "daily": {
                "problem": "daily_problem",
                "ranklist": "daily_ranklist",
                "finish": "daily_finish"
            },
            "accept": "accept",
            "decline": "decline",
            "cancel": "cancel",
            "change": "change",
            "giveup": "give_up",
            "judge": "judge",
            "ranklist": "ranklist",
            "ongoing": "ongoing",
            "history": "history",
            "statics": "statics",
            "problem": "problem"
        },
        "bind": {
            "begin": "bind",
            "finish": "finish_bind"
        },
        "cf": {
            "rating": "cf_rating",
            "analyze": "cf_analyze",
            "recommend": "cf_recommend"
        },
        "at": {
            "rating": "at_rating"
        }
    })
});
