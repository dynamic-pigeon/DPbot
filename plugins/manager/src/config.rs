use std::sync::LazyLock;

use kovi::serde_json::{Value, json};
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

pub static HELP: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "bind": "/bind begin Codeforces用户名 绑定你的 CF 账号",
        "duel": [
            "/duel 用法：",
            "/duel challenge @p rating：挑战用户p，题目将随机选取一道分数为 rating 的题目",
            "/duel ongoing: 查询正在进行的单挑",
            "/duel query @p：查询用户 p 的 ELO rating",
            "/duel ranklist: 查询排行榜",
            "/duel history @p 查询用户 p 的单挑历史",
            "/duel statics: 查询历史统计",
            "/duel problem rating：随机一道分数为 rating 的题目"
        ],
        "contest": "/contest，获取最近的比赛信息",
        "cf": [
        ],
        "chat": "/chat [content], 和 ai 聊天",
        "at": [
            "/at rating at_id: 查询用户的 AtCoder rating"
        ]
    })
});

pub static CF_HELP: LazyLock<Value> = LazyLock::new(|| {
    json!([
        "/cf rating [@p, cf_id]: 查询用户的 CF rating",
        "/cf analyze [@p, cf_id]: 查询用户的 CF 做题情况",
        "/cf recommend: 智能推荐题目\n用法：/cf recommend [难度] [选项]\n难度可选 easy, medium(moderate), hard(difficult) 或具体 rating（800-3500 的 100 的倍数）\n选项有 --exclude-solved/-e（排除已解决题目），--count/-c N（推荐 N 个题目，默认为1，最多10个）\n\n默认参数是/cf recommend moderate -c 1 -r <你的rating向下取整>\n例如：\n/cf recommend easy 1200 -c 3 -r 1200 -e\n推荐3个简单且未解决的rating为1200的题目",
    ])
});

pub static DUEL_HELP: LazyLock<Value> = LazyLock::new(|| {
    json!([
        {
            "type": "text",
            "data": {
                "text": "在使用 /duel 进行单挑之前，请先使用 /bind 绑定自己的 CF 账号"
            }
        },
        {
            "type": "text",
            "data": {
                "text": "duel challenge @p rating [tags]：挑战用户p，题目将随机选取一道分数为 rating，标签为 [tags] 的题目\n\n标签的用法为：输入 CF 题目中包含的标签，多个标签用空格隔开，本身包含空格的标签请将空格用下划线 \"_\" 替换。可以在标签前加上 \"!\" ，表示要求不包含该标签。\n\n例如：\n/duel challenge @EternalAlexander 2400 geometry !data_structures\n输入以上指令，将挑战用户 EternalAlexander，题目将随机选取一道 rating 为 2400，标签包含 geometry，且不包含 data structures 的题目。\n\n另外支持两个 CF 中不包含的标签。new 将筛选比赛 id >= 1000 的题目，not-seen 将筛选自己没有提交过的题目。"
            }
        },
        {
            "type": "text",
            "data": {
                "text": "/duel problem rating [tags]：随机一道分数为 rating ，标签为 [tags] 的题目。tag 的用法和 /duel challenge 中一致"
            }
        },
        {
            "type": "text",
            "data": {
                "text": "/duel daily problem 访问今天的每日挑战题目\n通过每日挑战题目可以得到相应的积分\n/duel daily ranklist 可以查询总积分排行"
            }
        },
        {
            "type": "text",
            "data": {
                "text": "/duel ongoing: 查询正在进行的单挑\n/duel query @p：查询用户 p 的 ELO rating\n/duel ranklist: 查询排行榜\n/duel history @p 查询用户 p 的单挑历史\n/duel statics: 查询历史统计"
            }
        }
    ])
});
