use kovi::{
    MsgEvent,
    bot::message::Segment,
    log::{debug, error},
    serde_json::{self, json},
};
use rand::seq::IndexedRandom;

use crate::{
    duel::problem::Problem,
    sql::{
        self,
        duel::{challenge::CommitChallengeExt, user::CommitUserExt},
        utils::Commit,
    },
    utils::{IdOrText, today_utc, user_id_or_text},
};

use super::{
    challenge::{Challenge, ChallengeStatus},
    user::BindingUsers,
};

/// 处理错误并向用户发送错误消息
fn handle_error(event: &MsgEvent, e: anyhow::Error) {
    error!("Error: {}", e);
    event.reply(e.to_string());
}

/// 格式化题目链接
fn format_problem_link(contest_id: i64, index: &str) -> String {
    format!(
        "题目链接：https://codeforces.com/problemset/problem/{}/{}",
        contest_id, index
    )
}

//
// 排行榜相关处理器
//

/// 显示每日任务排行榜
pub async fn daily_ranklist(event: &MsgEvent) {
    match sql::duel::user::get_top_20_daily().await {
        Ok(ranklist) => {
            let mut result = "每日任务排行榜：(只显示前20)\n".to_string();
            for (i, user) in ranklist.iter().enumerate() {
                let default_str = "未绑定".to_string();
                result.push_str(&format!(
                    "{}. {} score: {}\n",
                    i + 1,
                    user.cf_id.as_ref().unwrap_or(&default_str),
                    user.daily_score
                ));
            }
            event.reply(result);
        }
        Err(e) => handle_error(event, e),
    }
}

/// 显示总排行榜
pub async fn rating_ranklist(event: &MsgEvent) {
    match sql::duel::user::get_top_20_ranklist().await {
        Ok(ranklist) => {
            let mut result = "排行榜：(只显示前20)\n".to_string();
            for (i, user) in ranklist.iter().enumerate() {
                let default_str = "未绑定".to_string();
                result.push_str(&format!(
                    "{}. {} rating: {}\n",
                    i + 1,
                    user.cf_id.as_ref().unwrap_or(&default_str),
                    user.rating,
                ));
            }
            event.reply(result);
        }
        Err(e) => handle_error(event, e),
    }
}

/// 排行榜（别名：rating_ranklist）
pub async fn ranklist(event: &MsgEvent) {
    rating_ranklist(event).await
}

/// 显示正在进行的决斗
pub async fn ongoing(event: &MsgEvent) {
    // 获取进行中的挑战
    let challenges = match sql::duel::challenge::get_ongoing_challenges().await {
        Ok(challenges) => challenges,
        Err(e) => {
            handle_error(event, anyhow::anyhow!(e));
            return;
        }
    };

    // 格式化结果
    let mut result = "正在进行的决斗：\n".to_string();

    for challenge in challenges.iter() {
        // 获取用户信息
        let user1 = match sql::duel::user::get_user(challenge.user1).await {
            Ok(user) => user,
            Err(e) => {
                handle_error(event, anyhow::anyhow!(e));
                return;
            }
        };

        let user2 = match sql::duel::user::get_user(challenge.user2).await {
            Ok(user) => user,
            Err(e) => {
                handle_error(event, anyhow::anyhow!(e));
                return;
            }
        };

        let user1_id = user1.cf_id.unwrap_or_else(|| "未绑定".to_string());
        let user2_id = user2.cf_id.unwrap_or_else(|| "未绑定".to_string());

        // 获取题目信息
        let problem = match challenge.problem.as_ref() {
            Some(problem) => problem,
            None => {
                event.reply("错误：挑战中没有题目信息");
                return;
            }
        };

        // 计算持续时间
        let duration = today_utc().signed_duration_since(challenge.time);
        let duration = format!(
            "{}d {}h {}m {}s",
            duration.num_days(),
            duration.num_hours() % 24,
            duration.num_minutes() % 60,
            duration.num_seconds() % 60
        );

        // 添加到结果
        result.push_str(&format!(
            "{} vs {} problem: {}{}, last for {}\n",
            user1_id, user2_id, problem.contest_id, problem.index, duration
        ));
    }

    // 发送消息
    let seg = Segment::new(
        "node",
        json!({
            "user_id": event.self_id,
            "nickname": "呵呵哒",
            "content": [{
                "type": "text",
                "data": {
                    "text": result
                }
            }]
        }),
    );

    let msg = kovi::Message::from(vec![seg]);
    event.reply(msg);
}

/// 放弃决斗
pub async fn give_up(event: &MsgEvent) {
    let user_id = event.user_id;

    // 获取进行中的挑战
    let mut challenge = match sql::duel::challenge::get_chall_ongoing_by_user(user_id).await {
        Ok(challenge) => challenge,
        Err(_) => {
            event.reply("你似乎没有正在进行的决斗");
            return;
        }
    };

    // 执行放弃
    match challenge.give_up(event.user_id).await {
        Ok(_) => handle_challenge_result(event, &challenge).await,
        Err(e) => handle_error(event, e),
    }
}

/// 完成每日任务
pub async fn daily_finish(event: &MsgEvent) {
    let user_id = event.user_id;

    // 获取用户信息并检查绑定状态
    let mut user = match sql::duel::user::get_user(user_id).await {
        Ok(user) if user.cf_id.is_some() => user,
        _ => {
            event.reply("你好像没有绑定 CF 账号哦");
            return;
        }
    };

    // 获取每日题目
    let daily_problem = match super::problem::get_daily_problem().await {
        Ok(problem) => problem,
        Err(e) => {
            handle_error(event, e);
            return;
        }
    };

    // 检查是否已完成
    let now = today_utc().format("%Y-%m-%d").to_string();
    if user.last_daily == now {
        event.reply("你今天已经完成了每日任务");
        return;
    }

    // 获取最新提交
    let cf_id = user.cf_id.as_ref().unwrap();
    let submission = match super::problem::get_last_submission(cf_id).await {
        Ok(submission) => submission,
        Err(e) => {
            event.reply("获取提交记录失败");
            debug!("获取提交记录失败: {}", e);
            return;
        }
    };

    debug!("Submission: {:#?}", submission);

    // 解析提交和题目信息
    let (submission, problem) = match (move || {
        if let serde_json::Value::Object(mut map) = submission {
            let problem: Problem = serde_json::from_value(
                map.remove("problem")
                    .ok_or_else(|| anyhow::anyhow!("没有找到提交的题目"))?,
            )?;
            anyhow::Ok((map, problem))
        } else {
            Err(anyhow::anyhow!("获取提交记录失败"))?
        }
    })() {
        Ok(res) => res,
        Err(e) => {
            handle_error(event, e);
            return;
        }
    };

    // 检查是否完成了正确的题目
    if !problem.same_problem(&daily_problem)
        || submission.get("verdict").and_then(|v| v.as_str()) != Some("OK")
    {
        event.reply("未发现通过记录");
        return;
    }

    // 更新用户分数
    user.daily_score += daily_problem.rating.unwrap();
    user.last_daily = now;

    // 提交更改
    match async {
        Commit::start()
            .await?
            .update_user_daily(&user)
            .await?
            .commit()
            .await?;
        anyhow::Ok(())
    }
    .await
    {
        Ok(_) => {
            event.reply(format!(
                "你今天完成了每日任务，获得了 {} 分\n你现在的总分为 {}",
                daily_problem.rating.unwrap(),
                user.daily_score
            ));
        }
        Err(e) => handle_error(event, anyhow::anyhow!(e)),
    }
}

//
// 题目相关处理器
//

/// 获取每日题目
pub async fn daily_problem(event: &MsgEvent) {
    match super::problem::get_daily_problem().await {
        Ok(problem) => {
            let link = format_problem_link(problem.contest_id, &problem.index);
            event.reply(link);
        }
        Err(e) => handle_error(event, e),
    }
}

/// 随机获取符合条件的题目
pub async fn problem(event: &MsgEvent, args: &[String]) {
    // 解析参数
    let rating = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let tags = if args.len() >= 3 { &args[3..] } else { &[] };

    // 获取并选择随机题目
    let result = super::problem::get_problems_by(tags, rating, event.user_id)
        .await
        .and_then(|problems| {
            problems
                .choose(&mut rand::rng())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("没有找到题目"))
        });

    match result {
        Ok(problem) => {
            let link = format_problem_link(problem.contest_id, &problem.index);
            event.reply(link);
        }
        Err(e) => handle_error(event, e),
    }
}

//
// 决斗相关处理器
//

/// 处理挑战结果
async fn handle_challenge_result(event: &MsgEvent, challenge: &Challenge) {
    // 获取用户信息
    let mut user1 = match sql::duel::user::get_user(challenge.user1).await {
        Ok(user) => user,
        Err(e) => {
            handle_error(event, e);
            return;
        }
    };

    let mut user2 = match sql::duel::user::get_user(challenge.user2).await {
        Ok(user) => user,
        Err(e) => {
            handle_error(event, e);
            return;
        }
    };

    // 确定胜者和败者
    let (winner, loser) = match challenge.status {
        ChallengeStatus::Finished(0) => (challenge.user1, challenge.user2),
        ChallengeStatus::Finished(1) => {
            std::mem::swap(&mut user1, &mut user2);
            (challenge.user2, challenge.user1)
        }
        _ => {
            event.reply("未知错误：决斗未结束");
            return;
        }
    };

    // 获取胜者和败者的信息
    let winner_user = match sql::duel::user::get_user(winner).await {
        Ok(user) => user,
        Err(e) => {
            handle_error(event, e);
            return;
        }
    };

    let winner_id = winner_user.cf_id.clone().unwrap_or_default();

    let loser_user = match sql::duel::user::get_user(loser).await {
        Ok(user) => user,
        Err(e) => {
            handle_error(event, e);
            return;
        }
    };

    // 生成结果消息
    let result = format!(
        "比赛结束，{winner_id} 取得了胜利。\nrating 变化: \n{}: {} + {} = {}\n{}: {} - {} = {}",
        user1.cf_id.as_ref().unwrap_or(&"未绑定".to_string()),
        user1.rating,
        winner_user.rating - user1.rating,
        winner_user.rating,
        user2.cf_id.as_ref().unwrap_or(&"未绑定".to_string()),
        user2.rating,
        user2.rating - loser_user.rating,
        loser_user.rating
    );

    event.reply(result);
}

/// 评判决斗结果
pub async fn judge(event: &MsgEvent) {
    let user_id = event.user_id;

    // 获取进行中的挑战
    let mut challenge = match sql::duel::challenge::get_chall_ongoing_by_user(user_id).await {
        Ok(challenge) => challenge,
        Err(_) => {
            event.reply("你似乎没有正在进行的决斗");
            return;
        }
    };

    // 执行判定
    match challenge.judge().await {
        Ok(_) => handle_challenge_result(event, &challenge).await,
        Err(e) => handle_error(event, e),
    }
}

/// 更换题目
pub async fn change(event: &MsgEvent) {
    let user_id = event.user_id;

    match super::challenge::get_ongoing_challenge_by_user(user_id).await {
        Ok(mut challenge) => match challenge.status {
            ChallengeStatus::Pending => {
                event.reply("你还没有开始决斗");
            }
            ChallengeStatus::ChangeProblem(user) if user == user_id => {
                event.reply("你已经发起了换题请求");
            }
            _ => {
                // 执行换题操作
                let problem = match challenge.change().await {
                    Ok(problem) => problem,
                    Err(e) => {
                        handle_error(event, e);
                        return;
                    }
                };

                challenge.status = ChallengeStatus::Ongoing;

                // 生成并发送题目链接
                let link = format_problem_link(problem.contest_id, &problem.index);
                event.reply(link);

                // 提交状态变更
                match Commit::start().await {
                    Ok(mut commit) => {
                        if let Err(e) = commit.change_problem(&challenge).await {
                            handle_error(event, e);
                        }
                    }
                    Err(e) => handle_error(event, anyhow::anyhow!(e)),
                }
            }
        },
        _ => {
            // 获取进行中的挑战
            let mut challenge = match sql::duel::challenge::get_chall_ongoing_by_user(user_id).await
            {
                Ok(challenge) => challenge,
                Err(_) => {
                    event.reply("你没有正在进行的决斗");
                    return;
                }
            };

            // 获取用户信息
            let user = match sql::duel::user::get_user(user_id).await {
                Ok(user) => user,
                Err(e) => {
                    handle_error(event, e);
                    return;
                }
            };

            // 更新挑战状态
            if let Err(e) = challenge
                .change_status(ChallengeStatus::ChangeProblem(user_id))
                .await
            {
                handle_error(event, e);
                return;
            }

            // 发送确认消息
            let default_str = "未绑定".to_string();
            let cf_id = user.cf_id.as_ref().unwrap_or(&default_str);
            event.reply(format!(
                "{} 发起了换题请求，请输入 /duel change 确认",
                cf_id
            ));
        }
    }
}

// 这些函数已经重新实现在下方

//
// 挑战相关处理器
//

/// 发起挑战
pub async fn challenge(event: &MsgEvent, args: &[String]) {
    // 解析被挑战者ID
    let user1 = event.user_id;
    let user2 = match args.get(2).and_then(|s| match user_id_or_text(s) {
        Ok(IdOrText::At(user_id)) => Some(user_id),
        _ => None,
    }) {
        Some(user2) => user2,
        None => {
            event.reply("参数非法：需要@被挑战者");
            return;
        }
    };

    // 解析题目难度
    let rating = match args.get(3).and_then(|s| s.parse::<i64>().ok()) {
        Some(rating) => rating,
        None => {
            event.reply("参数非法：需要提供题目难度");
            return;
        }
    };

    // 解析题目标签
    let tags = if args.len() >= 4 {
        args[4..].to_vec()
    } else {
        Vec::new()
    };

    // 创建挑战
    match Challenge::from_args(user1, user2, rating, tags).await {
        Ok((_chall, u1, u2)) => {
            event.reply(format!(
                "{} 向 {} 发起了挑战，请输入 /duel accept 接受挑战，或 /duel decline 拒绝挑战",
                u1, u2
            ));
        }
        Err(e) => handle_error(event, e),
    }
}

/// 接受挑战
pub async fn accept(event: &MsgEvent) {
    let user2 = event.user_id;

    // 获取发起挑战的用户
    let user1 = match crate::duel::challenge::get_challenge_by_user2(user2).await {
        Ok(challenge) if challenge.status != ChallengeStatus::Pending => {
            event.reply("比赛已经开始了");
            return;
        }
        Ok(challenge) => challenge.user1,
        Err(_) => {
            event.reply("你没有收到挑战");
            return;
        }
    };

    // 获取并开始挑战
    match crate::duel::challenge::get_challenge(user1, user2).await {
        Ok(mut challenge) => match challenge.start().await {
            Ok(problem) => {
                let link = format_problem_link(problem.contest_id, &problem.index);
                event.reply(link);
            }
            Err(e) => handle_error(event, e),
        },
        Err(e) => handle_error(event, e),
    }
}

/// 拒绝挑战
pub async fn decline(event: &MsgEvent) {
    let user2 = event.user_id;

    // 获取挑战信息
    match crate::duel::challenge::get_challenge_by_user2(user2).await {
        Ok(challenge) if challenge.is_started() => {
            event.reply("比赛已经开始了");
        }
        Ok(challenge) => match crate::duel::challenge::remove_challenge(&challenge).await {
            Ok(_) => event.reply("你拒绝了挑战"),
            Err(e) => handle_error(event, e),
        },
        Err(_) => {
            event.reply("你没有收到挑战");
        }
    }
}

/// 取消挑战
pub async fn cancel(event: &MsgEvent) {
    let user1 = event.user_id;

    // 获取挑战信息
    match crate::duel::challenge::get_challenge_by_user1(user1).await {
        Ok(challenge) if challenge.is_started() => {
            event.reply("比赛已经开始了");
        }
        Ok(challenge) => match crate::duel::challenge::remove_challenge(&challenge).await {
            Ok(_) => event.reply("你取消了挑战"),
            Err(e) => handle_error(event, e),
        },
        Err(e) => {
            // 这里使用error!而不是handle_error是因为不需要向用户展示详细错误
            error!("{}", e);
            event.reply("你没有发起挑战");
        }
    }
}

/// 获取随机题目
#[allow(dead_code)]
pub async fn random_problem(event: &MsgEvent, args: &[String]) {
    // 解析参数
    let rating = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let tags = if args.len() >= 3 { &args[3..] } else { &[] };

    // 获取并选择随机题目
    let result = super::problem::get_problems_by(tags, rating, event.user_id)
        .await
        .and_then(|problems| {
            problems
                .choose(&mut rand::rng())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("没有找到题目"))
        });

    match result {
        Ok(problem) => {
            let link = format_problem_link(problem.contest_id, &problem.index);
            event.reply(link);
        }
        Err(e) => handle_error(event, e),
    }
}

//
// 用户绑定相关处理器
//

/// 开始绑定 CF 账号
pub async fn bind(event: &MsgEvent, args: &[String], binding_users: &BindingUsers) {
    let Some(cf_id) = args.get(2) else {
        event.reply("请告知 cf 账号");
        return;
    };

    if binding_users.contains(event.user_id).await {
        event.reply("你正在绑定一个账号，请先输入 /bind finish 结束绑定");
        return;
    }

    // 获取或创建用户
    let mut user = match get_or_create_user(event).await {
        Some(user) => user,
        None => return,
    };

    user.start_bind(cf_id.clone());
    binding_users.insert(user).await;
    event.reply(format!(
        "你正在绑定 CF 账号：{}，请在 120 秒内向 https://codeforces.com/contest/1/problem/A 提交一个 CE，之后输入 /bind finish 完成绑定。", 
        cf_id
    ));
}

/// 完成绑定 CF 账号
pub async fn finish_bind(event: &MsgEvent, binding_users: &BindingUsers) {
    let Some(mut user) = binding_users.take(event.user_id).await else {
        event.reply("你似乎没有在绑定哦");
        return;
    };

    match user.finish_bind().await {
        Ok(_) => event.reply("绑定成功"),
        Err(e) => handle_error(event, e),
    }
}

/// 获取或创建用户
async fn get_or_create_user(event: &MsgEvent) -> Option<super::user::User> {
    match crate::sql::duel::user::get_user(event.user_id).await {
        Ok(user) => Some(user),
        Err(_) => {
            let result = async {
                Commit::start()
                    .await?
                    .add_user(event.user_id)
                    .await?
                    .commit()
                    .await?;
                anyhow::Ok(())
            }
            .await;

            if result.is_err() {
                event.reply("未知错误");
                return None;
            }

            match sql::duel::user::get_user(event.user_id).await {
                Ok(user) => Some(user),
                Err(e) => {
                    handle_error(event, e);
                    None
                }
            }
        }
    }
}
