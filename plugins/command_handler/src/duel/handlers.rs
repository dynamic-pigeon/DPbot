use kovi::{MsgEvent, bot::message::Segment, log::debug, serde_json::json};
use rand::seq::SliceRandom;

use crate::{sql, today_utc};

use super::challenge::Challenge;

pub async fn daily_ranklist(event: &MsgEvent) {
    let ranklist = match sql::duel::user::get_top_20_daily().await {
        Ok(ranklist) => ranklist,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    let mut result = "每日任务排行榜：(只显示前20)\n".to_string();
    for (i, user) in ranklist.iter().enumerate() {
        result.push_str(&format!(
            "{}. {} score: {}\n",
            i + 1,
            user.cf_id.as_ref().unwrap(),
            user.daily_score
        ));
    }

    event.reply(result);
}

pub async fn ranklist(event: &MsgEvent) {
    let ranklist = match sql::duel::user::get_top_20_ranklist().await {
        Ok(ranklist) => ranklist,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    let mut result = "排行榜：(只显示前20)\n".to_string();
    for (i, user) in ranklist.iter().enumerate() {
        result.push_str(&format!(
            "{}. {} rating: {}\n",
            i + 1,
            user.cf_id.as_ref().unwrap(),
            user.rating,
        ));
    }

    event.reply(result);
}

pub async fn ongoing(event: &MsgEvent) {
    let challenge = match sql::duel::challenge::get_ongoing_challenges().await {
        Ok(challenge) => challenge,
        Err(_) => {
            event.reply("未知错误");
            return;
        }
    };

    let mut result = "正在进行的决斗：\n".to_string();

    for challenge in challenge.iter() {
        let user1 = sql::duel::user::get_user(challenge.user1).await.unwrap();
        let user2 = sql::duel::user::get_user(challenge.user2).await.unwrap();

        let user1 = user1.cf_id.unwrap();
        let user2 = user2.cf_id.unwrap();

        let problem = challenge.problem.as_ref().unwrap();
        let duration = today_utc().signed_duration_since(challenge.time);

        let duration = format!(
            "{}d {}h {}m {}s",
            duration.num_days(),
            duration.num_hours() % 24,
            duration.num_minutes() % 60,
            duration.num_seconds() % 60
        );

        result.push_str(&format!(
            "{} vs {} problem: {}{}, last for {}\n",
            user1, user2, problem.contest_id, problem.index, duration
        ));
    }

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

pub async fn give_up(event: &MsgEvent) {
    let user_id = event.user_id;

    let mut challenge = match sql::duel::challenge::get_chall_ongoing_by_user(user_id).await {
        Ok(challenge) => challenge,
        Err(_) => {
            event.reply("你似乎没有正在进行的决斗");
            return;
        }
    };

    let mut user1 = sql::duel::user::get_user(challenge.user1).await.unwrap();
    let mut user2 = sql::duel::user::get_user(challenge.user2).await.unwrap();

    match challenge.give_up(event.user_id).await {
        Ok(_) => {
            let (winner, loser) = match challenge.result {
                Some(0) => (challenge.user1, challenge.user2),
                Some(1) => {
                    std::mem::swap(&mut user1, &mut user2);
                    (challenge.user2, challenge.user1)
                }
                _ => {
                    event.reply("未知错误");
                    return;
                }
            };

            let winner = sql::duel::user::get_user(winner).await.unwrap();
            let winner_id = winner.cf_id.unwrap();

            let loser = sql::duel::user::get_user(loser).await.unwrap();

            let result = format!(
                "比赛结束，{winner_id} 取得了胜利。\nrating 变化: \n{}: {} + {} = {}\n{}: {} - {} = {}",
                user1.cf_id.unwrap(),
                user1.rating,
                winner.rating - user1.rating,
                winner.rating,
                user2.cf_id.unwrap(),
                user2.rating,
                loser.rating - user2.rating,
                loser.rating
            );
            event.reply(result);
            let _ = super::challenge::remove_challenge(challenge.user1, challenge.user2).await;
        }
        Err(e) => {
            event.reply(e.to_string());
        }
    }
}

pub async fn daily_finish(event: &MsgEvent) {
    let user_id = event.user_id;

    let mut user = match sql::duel::user::get_user(user_id).await {
        Ok(user) if user.cf_id.is_some() => user,
        _ => {
            event.reply("你好像没有绑定 CF 账号哦");
            return;
        }
    };

    let daily_problem = match super::problem::get_daily_problem().await {
        Ok(problem) => problem,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    let now = today_utc().format("%Y-%m-%d").to_string();

    if user.last_daily == now {
        event.reply("你今天已经完成了每日任务");
        return;
    }

    let submission = match super::problem::get_last_submission(user.cf_id.as_ref().unwrap()).await {
        Some(submission) => submission,
        _ => {
            event.reply("获取提交记录失败");
            return;
        }
    };

    let problem = match submission.get("problem").and_then(|v| v.as_object()) {
        Some(problem) => problem,
        None => {
            event.reply("未知错误");
            return;
        }
    };

    debug!("Submission: {:#?}", submission);

    let contest_id = problem.get("contestId").and_then(|v| v.as_i64());
    let index = problem.get("index").and_then(|v| v.as_str());

    if contest_id != Some(daily_problem.contest_id)
        || index != Some(&daily_problem.index)
        || submission.get("verdict").and_then(|v| v.as_str()) != Some("OK")
    {
        event.reply("未发现通过记录");
        return;
    }

    user.daily_score += daily_problem.rating;
    user.last_daily = now;

    match sql::duel::user::update_user(&user).await {
        Ok(_) => {
            event.reply(format!(
                "你今天完成了每日任务，获得了 {} 分\n你现在的总分为 {}",
                daily_problem.rating, user.daily_score
            ));
        }
        Err(_) => {
            event.reply("未知错误");
        }
    }
}

pub async fn daily_problem(event: &MsgEvent) {
    let problem = super::problem::get_daily_problem().await.unwrap();

    let contest_id = problem.contest_id;
    let index = &problem.index;
    let problem = format!(
        "题目链接：https://codeforces.com/problemset/problem/{}/{}",
        contest_id, index
    );
    event.reply(problem);
}

pub async fn judge(event: &MsgEvent) {
    let user_id = event.user_id;

    let mut challenge = match sql::duel::challenge::get_chall_ongoing_by_user(user_id).await {
        Ok(challenge) => challenge,
        Err(_) => {
            event.reply("你似乎没有正在进行的决斗");
            return;
        }
    };

    let mut user1 = sql::duel::user::get_user(challenge.user1).await.unwrap();
    let mut user2 = sql::duel::user::get_user(challenge.user2).await.unwrap();

    match challenge.judge().await {
        Ok(_) => {
            let (winner, loser) = match challenge.result {
                Some(0) => (challenge.user1, challenge.user2),
                Some(1) => {
                    std::mem::swap(&mut user1, &mut user2);
                    (challenge.user2, challenge.user1)
                }
                _ => {
                    event.reply("未知错误");
                    return;
                }
            };

            let winner = sql::duel::user::get_user(winner).await.unwrap();
            let winner_id = winner.cf_id.unwrap();

            let loser = sql::duel::user::get_user(loser).await.unwrap();

            let result = format!(
                "比赛结束，{winner_id} 取得了胜利。\nrating 变化: \n{}: {} + {} = {}\n{}: {} - {} = {}",
                user1.cf_id.unwrap(),
                user1.rating,
                winner.rating - user1.rating,
                winner.rating,
                user2.cf_id.unwrap(),
                user2.rating,
                loser.rating - user2.rating,
                loser.rating
            );
            event.reply(result);
            let _ = super::challenge::remove_challenge(challenge.user1, challenge.user2).await;
        }
        Err(e) => {
            event.reply(e.to_string());
        }
    }
}

pub async fn change(event: &MsgEvent) {
    let user_id = event.user_id;

    match super::challenge::get_challenge(user_id).await {
        Some(mut challenge) => {
            if challenge.started == 0 {
                event.reply("你还没有开始决斗");
            } else if challenge.changed == Some(user_id) {
                event.reply("你已经发起了换题请求，请等待对手确认");
            } else {
                let problem = match challenge.change().await {
                    Ok(problem) => problem,
                    Err(e) => {
                        event.reply(e.to_string());
                        return;
                    }
                };
                challenge.changed = None;

                let problem = format!(
                    "题目链接：https://codeforces.com/problemset/problem/{}/{}",
                    problem.contest_id, problem.index
                );

                event.reply(problem);

                sql::duel::challenge::change_problem(&challenge)
                    .await
                    .unwrap();
                return;
            }

            super::challenge::add_challenge(challenge).await;
        }
        None => {
            let challenge = match sql::duel::challenge::get_chall_ongoing_by_user(user_id).await {
                Ok(challenge) => challenge,
                Err(_) => {
                    event.reply("你没有正在进行的决斗");
                    return;
                }
            };

            super::challenge::add_challenge(challenge).await;

            let user = match sql::duel::user::get_user(user_id).await {
                Ok(user) => user,
                Err(_) => {
                    event.reply("未知错误");
                    return;
                }
            };

            event.reply(format!(
                "{} 发起了换题请求，请输入 /duel change 确认",
                user.cf_id.unwrap()
            ));
        }
    }
}

pub async fn decline(event: &MsgEvent) {
    let user2 = event.user_id;
    let user1 = match crate::duel::challenge::get_challenge_by_user2(user2).await {
        Some(challenge) => challenge.user1,
        None => {
            event.reply("你没有收到挑战");
            return;
        }
    };

    let _challenge = crate::duel::challenge::remove_challenge(user1, user2)
        .await
        .unwrap();

    event.reply("你拒绝了挑战");
}

pub async fn cancel(event: &MsgEvent) {
    let user1 = event.user_id;
    let user2 = match crate::duel::challenge::get_challenge_by_user1(user1).await {
        Some(challenge) => challenge.user2,
        None => {
            event.reply("你没有发起挑战");
            return;
        }
    };

    let _challenge = crate::duel::challenge::remove_challenge(user1, user2)
        .await
        .unwrap();

    event.reply("你取消了挑战");
}

pub async fn accept(event: &MsgEvent) {
    let user2 = event.user_id;
    let user1 = match crate::duel::challenge::get_challenge_by_user2(user2).await {
        Some(challenge) => challenge.user1,
        None => {
            event.reply("你没有收到挑战");
            return;
        }
    };

    let mut challenge = crate::duel::challenge::remove_challenge(user1, user2)
        .await
        .unwrap();

    let problem = match challenge.start().await {
        Ok(problem) => problem,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    let problem = format!(
        "题目链接：https://codeforces.com/problemset/problem/{}/{}",
        problem.contest_id, problem.index
    );

    event.reply(problem);
}

pub async fn challenge(event: &MsgEvent, args: &[String]) {
    let user1 = event.user_id;
    let user2 = match args.get(2).and_then(|s| s.parse::<i64>().ok()) {
        Some(user2) => user2,
        None => {
            event.reply("参数非法");
            return;
        }
    };

    if user1 == user2 {
        event.reply("你知道吗，人不能逃离自己的影子");
        return;
    }

    let u1 = match sql::duel::user::get_user(user1).await {
        Ok(user) => user,
        Err(_) => {
            event.reply("你没有绑定 CF 账号");
            return;
        }
    };

    let u2 = match sql::duel::user::get_user(user2).await {
        Ok(user) => user,
        Err(_) => {
            event.reply("对方没有绑定 CF 账号");
            return;
        }
    };

    if super::challenge::user_inside(user1).await || super::challenge::user_inside(user2).await {
        event.reply("你或对方正在决斗中");
        return;
    }

    let rating = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);

    if rating < 800 || rating > 3500 || rating % 100 != 0 {
        event.reply("rating 应该是 800 到 3500 之间的整数");
        return;
    }

    let tags = if args.len() >= 4 {
        args[4..].to_vec()
    } else {
        Vec::new()
    };
    let time = today_utc();

    let challenge = Challenge::new(user1, user2, time, tags, rating, None, None, 0);

    crate::duel::challenge::add_challenge(challenge).await;

    let msg = format!(
        "{} 向 {} 发起了挑战，请输入 /duel accept 接受挑战，或 /duel decline 拒绝挑战",
        u1.cf_id.unwrap(),
        u2.cf_id.unwrap()
    );

    event.reply(msg);
}

pub async fn problem(event: &MsgEvent, args: &[String]) {
    let rating = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let tags = if args.len() >= 3 { &args[3..] } else { &[] };

    let problems = match super::problem::get_problems_by(tags, rating, event.user_id).await {
        Ok(problems) => problems,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    let problem = match problems.choose(&mut rand::thread_rng()) {
        Some(problem) => problem,
        None => {
            event.reply("没有找到题目");
            return;
        }
    };

    let problem = format!(
        "题目链接：https://codeforces.com/problemset/problem/{}/{}",
        problem.contest_id, problem.index
    );
    event.reply(problem);
}

pub async fn bind(event: &MsgEvent, args: &[String]) {
    let Some(cf_id) = args.get(2) else {
        event.reply("请告知 cf 账号");
        return;
    };

    if crate::duel::user::user_inside(event.user_id).await {
        event.reply("你正在绑定一个账号，请先输入 /bind finish 结束绑定");
        return;
    }

    let mut user = match crate::sql::duel::user::get_user(event.user_id).await {
        Ok(user) => user,
        Err(_) => {
            let Ok(user) = crate::sql::duel::user::add_user(event.user_id).await else {
                event.reply("未知错误");
                return;
            };
            user
        }
    };

    user.bind(cf_id.clone());
    crate::duel::user::add_to(user).await;
    event.reply(format!("你正在绑定 CF 账号：{}，请在 120 秒内向 https://codeforces.com/contest/1/problem/A 提交一个 CE，之后输入 /bind finish 完成绑定。", cf_id));
}

pub async fn finish_bind(event: &MsgEvent) {
    let Some(mut user) = crate::duel::user::get_user(event.user_id).await else {
        event.reply("你似乎没有在绑定哦");
        return;
    };

    match user.finish_bind().await {
        Ok(_) => {
            event.reply("绑定成功");
        }
        Err(e) => {
            event.reply(e.to_string());
        }
    }
}
