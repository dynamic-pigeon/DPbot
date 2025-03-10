use kovi::MsgEvent;

pub async fn daily_problem(event: &MsgEvent) {
    let problem = super::problem::get_daily_problem().await.unwrap();

    let contest_id = problem["contestId"].as_i64().unwrap();
    let index = problem["index"].as_str().unwrap();
    let problem = format!(
        "https://codeforces.com/problemset/problem/{}/{}",
        contest_id, index
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

    let mut user = match crate::sql::get_user(event.user_id).await {
        Ok(user) => user,
        Err(_) => {
            let Ok(user) = crate::sql::add_user(event.user_id).await else {
                event.reply("未知错误");
                return;
            };
            user
        }
    };

    user.bind(cf_id.clone());
    crate::duel::user::add_to(user).await;
    event.reply("你正在绑定 CF 账号：Dynamic_Pigeon，请在 120 秒内向 https://codeforces.com/contest/1/problem/A 提交一个 CE，之后输入 /bind finish 完成绑定。");
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
