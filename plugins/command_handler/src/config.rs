use std::sync::LazyLock;

use kovi::serde_json::{self, Value};

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
            "analyze": "cf_analyze"
        },
        "at": {
            "rating": "at_rating"
        }
    })
});
