use anyhow::Result;
use kovi::MsgEvent;
use rand::Rng;
use rand::seq::SliceRandom;
use std::collections::HashSet;
use std::sync::Arc;

use crate::{
    duel::{problem::get_problems, submission::get_recent_submissions},
    utils::{IdOrText, get_user_rating},
};

#[derive(Debug, Clone)]
pub enum RecommendDifficulty {
    Easy,      // 80% 概率解出
    Moderate,  // 50% 概率解出
    Difficult, // 20% 概率解出
}

impl RecommendDifficulty {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "easy" | "简单" => Some(Self::Easy),
            "moderate" | "medium" | "中等" => Some(Self::Moderate),
            "difficult" | "hard" | "困难" => Some(Self::Difficult),
            _ => None,
        }
    }

    #[allow(dead_code)]
    fn get_target_solve_probability(&self) -> f64 {
        match self {
            Self::Easy => 0.8,
            Self::Moderate => 0.5,
            Self::Difficult => 0.2,
        }
    }

    fn get_rating_range(&self, user_rating: i64) -> (i64, i64) {
        // 基于ELO系统的解题概率计算，估算合适的rating范围
        match self {
            Self::Easy => ((user_rating - 400).max(800), (user_rating - 100).max(800)),
            Self::Moderate => ((user_rating - 200).max(800), (user_rating + 200).min(3500)),
            Self::Difficult => ((user_rating + 100).min(3500), (user_rating + 600).min(3500)),
        }
    }
}

// 解析命令参数结构
struct CommandArgs {
    difficulty: RecommendDifficulty,
    exclude_solved: bool,
    count: usize,
    specific_rating: Option<i64>,
}

// 解析命令行参数
fn parse_args(args: &[String]) -> CommandArgs {
    let mut difficulty = RecommendDifficulty::Moderate;
    let mut exclude_solved = false;
    let mut count = 1;
    let mut specific_rating = None;

    let mut i = 2;
    while i < args.len() {
        let arg = &args[i];
        if let Some(diff) = RecommendDifficulty::from_str(arg) {
            difficulty = diff;
        } else if arg == "--exclude-solved" || arg == "-e" {
            exclude_solved = true;
        } else if (arg == "--count" || arg == "-c")
            && i + 1 < args.len()
            && let Ok(c) = args[i + 1].parse::<usize>()
        {
            count = c.min(10); // 最多推荐10个
            i += 1;
        } else if (arg == "--rating" || arg == "-r")
            && i + 1 < args.len()
            && let Ok(r) = args[i + 1].parse::<i64>()
            && (800..=3500).contains(&r)
            && r % 100 == 0
        {
            specific_rating = Some(r);
            i += 1;
        }

        i += 1;
    }

    CommandArgs {
        difficulty,
        exclude_solved,
        count,
        specific_rating,
    }
}

// 计算题目的tag权重分数
fn calculate_tag_score(
    problem: &crate::duel::problem::Problem,
    tag_weight: &std::collections::HashMap<String, usize>,
) -> usize {
    problem
        .tags
        .iter()
        .map(|t| tag_weight.get(t).copied().unwrap_or(0))
        .sum::<usize>()
}

// 过滤出符合条件的候选题目
fn filter_candidate_problems<'a>(
    problems: &'a Arc<Vec<Arc<crate::duel::problem::Problem>>>,
    min_rating: i64,
    max_rating: i64,
    solved_problems: &Option<HashSet<(i64, String)>>,
) -> Vec<&'a Arc<crate::duel::problem::Problem>> {
    problems
        .iter()
        .filter(|p| {
            // 按rating过滤
            if let Some(r) = p.rating {
                if r < min_rating || r > max_rating {
                    return false;
                }
            } else {
                return false;
            }

            // 过滤已解决题目
            if let Some(solved) = solved_problems
                && solved.contains(&(p.contest_id, p.index.clone()))
            {
                return false;
            }

            // 过滤特殊题目
            !p.tags.iter().any(|tag| tag == "*special")
        })
        .collect()
}

// 格式化推荐结果输出
fn format_recommendation_output(
    problems: &[&Arc<crate::duel::problem::Problem>],
    cmd_args: &CommandArgs,
) -> String {
    if problems.len() == 1 {
        let problem = &problems[0];
        let link = format!(
            "https://codeforces.com/problemset/problem/{}/{}",
            problem.contest_id, problem.index
        );
        format!(
            "推荐题目：\n题目：{} {}\n难度：{}\n标签：{}\n链接：{}",
            problem.contest_id,
            problem.index,
            problem.rating.unwrap_or(0),
            problem.tags.join(", "),
            link
        )
    } else {
        let list_title = if let Some(specific_r) = cmd_args.specific_rating {
            format!("推荐的rating {} 题目：\n", specific_r)
        } else {
            format!("推荐的{:?}难度题目：\n", cmd_args.difficulty)
        };
        let mut msg = list_title;
        for (i, problem) in problems.iter().enumerate() {
            let link = format!(
                "https://codeforces.com/problemset/problem/{}/{}",
                problem.contest_id, problem.index
            );
            msg += &format!(
                "{}. {} {} (难度: {})\n   标签: {}\n   链接: {}\n\n",
                i + 1,
                problem.contest_id,
                problem.index,
                problem.rating.unwrap_or(0),
                problem.tags.join(", "),
                link
            );
        }
        msg
    }
}

// 获取用户提交记录相关的数据
async fn get_user_submission_data(
    cf_id: &str,
    exclude_solved: bool,
) -> (
    std::collections::HashMap<String, usize>,
    Option<HashSet<(i64, String)>>,
) {
    // 统计用户未通过题目的 tag 权重
    let mut tag_weight = std::collections::HashMap::new();
    let mut solved_problems = None;

    // 只调用一次 get_recent_submissions
    if let Ok(submissions) = get_recent_submissions(cf_id).await {
        // 处理tag权重
        for sub in &submissions {
            if sub.is_accepted() {
                for tag in &sub.problem.tags {
                    *tag_weight.entry(tag.clone()).or_insert(0) += 1;
                }
            }
        }

        // 如果需要排除已解决题目，处理已解决集合
        if exclude_solved {
            let solved: HashSet<_> = submissions
                .iter()
                .filter(|s| s.is_accepted())
                .map(|s| (s.problem.contest_id, s.problem.index.clone()))
                .collect();

            if !solved.is_empty() {
                solved_problems = Some(solved);
            }
        }
    }

    (tag_weight, solved_problems)
}

// 根据难度或指定rating获取过滤范围
fn get_rating_range(cmd_args: &CommandArgs, user_rating: i64) -> (i64, i64, String) {
    if let Some(specific_r) = cmd_args.specific_rating {
        // 如果指定了具体rating，就推荐该rating的题目
        (
            specific_r,
            specific_r,
            format!("rating {} 的题目", specific_r),
        )
    } else {
        // 否则根据难度推荐
        let (min, max) = cmd_args.difficulty.get_rating_range(user_rating);
        (
            min,
            max,
            format!(
                "{:?}难度题目（用户rating: {}）",
                cmd_args.difficulty, user_rating
            ),
        )
    }
}

pub async fn recommend(event: &MsgEvent, args: &[String]) {
    // 解析参数
    let cmd_args = parse_args(args);

    // 用户钉死为当前用户
    let user = IdOrText::At(event.user_id);

    let cf_id = match get_cf_id(&user).await {
        Ok(cf_id) => cf_id,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    let rating = match get_user_rating(&cf_id).await {
        Ok(r) => r,
        Err(e) => {
            event.reply(format!("获取rating失败: {}", e));
            return;
        }
    };

    // 根据是否指定了具体rating来确定过滤范围
    let (min_rating, max_rating, filter_msg) = get_rating_range(&cmd_args, rating);

    event.reply(format!("正在为您推荐{}", filter_msg));

    // 获取用户提交数据，包括tag权重和已解题列表
    let (tag_weight, solved_problems) =
        get_user_submission_data(&cf_id, cmd_args.exclude_solved).await;

    let problems = match get_problems().await {
        Ok(p) => p,
        Err(e) => {
            event.reply(format!("获取题库失败: {}", e));
            return;
        }
    };

    // 过滤候选题目
    let candidates = filter_candidate_problems(&problems, min_rating, max_rating, &solved_problems);

    if candidates.is_empty() {
        let filter_msg = if cmd_args.exclude_solved {
            "（已排除解决过的题目）"
        } else {
            ""
        };
        event.reply(format!("没有找到合适的题目{}", filter_msg));
        return;
    }

    // 排序逻辑
    let mut rng = rand::rng();

    let final_problems: Vec<_> = if cmd_args.specific_rating.is_some() {
        // 如果指定了具体rating，随机打乱即可
        let mut shuffled = candidates.to_vec();
        shuffled.shuffle(&mut rng);
        shuffled.into_iter().take(cmd_args.count).collect()
    } else {
        let target_rating = match cmd_args.difficulty {
            RecommendDifficulty::Easy => rating - 250,
            RecommendDifficulty::Moderate => rating,
            RecommendDifficulty::Difficult => rating + 350,
        };

        let mut candidate_vec = candidates;
        let mut weights: Vec<f64> = candidate_vec
            .iter()
            .map(|p| {
                let score = calculate_tag_score(p, &tag_weight) as f64;
                let diff = (p.rating.unwrap_or(target_rating) - target_rating).abs() as f64;
                let rating_bonus = 1.0 / (1.0 + diff / 100.0);
                (score + 1.0) * rating_bonus
            })
            .collect();

        let mut selected = Vec::new();
        let mut remaining = cmd_args.count.min(candidate_vec.len());

        while remaining > 0 && !candidate_vec.is_empty() {
            let total_weight: f64 = weights.iter().sum();
            if total_weight <= f64::EPSILON {
                candidate_vec.shuffle(&mut rng);
                selected.extend(candidate_vec.into_iter().take(remaining));
                break;
            }

            let mut pick = rng.random_range(0.0..total_weight);
            let mut idx = 0;
            let mut picked = false;
            while idx < weights.len() {
                if pick < weights[idx] {
                    selected.push(candidate_vec.remove(idx));
                    weights.remove(idx);
                    remaining -= 1;
                    picked = true;
                    break;
                }
                pick -= weights[idx];
                idx += 1;
            }

            if !picked {
                // 浮点误差导致未命中，退化为随机挑选
                let rand_idx = rng.random_range(0..candidate_vec.len());
                let chosen = candidate_vec.remove(rand_idx);
                selected.push(chosen);
                weights.remove(rand_idx);
                remaining -= 1;
            }
        }

        selected
    };

    // 格式化并发送输出结果
    let msg = format_recommendation_output(&final_problems, &cmd_args);
    event.reply(msg);
}

async fn get_cf_id(uit: &IdOrText<'_>) -> Result<String> {
    match uit {
        IdOrText::At(qq) => {
            let user = crate::sql::duel::user::get_user(*qq)
                .await
                .map_err(|_| anyhow::anyhow!("未找到用户"))?;
            Ok(user
                .cf_id
                .ok_or_else(|| anyhow::anyhow!("用户未绑定 cf 账号"))?)
        }
        IdOrText::Text(text) => Ok(text.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommend_difficulty_from_str() {
        // 测试英文
        assert!(matches!(
            RecommendDifficulty::from_str("easy"),
            Some(RecommendDifficulty::Easy)
        ));
        assert!(matches!(
            RecommendDifficulty::from_str("moderate"),
            Some(RecommendDifficulty::Moderate)
        ));
        assert!(matches!(
            RecommendDifficulty::from_str("difficult"),
            Some(RecommendDifficulty::Difficult)
        ));

        // 测试中文
        assert!(matches!(
            RecommendDifficulty::from_str("简单"),
            Some(RecommendDifficulty::Easy)
        ));
        assert!(matches!(
            RecommendDifficulty::from_str("中等"),
            Some(RecommendDifficulty::Moderate)
        ));
        assert!(matches!(
            RecommendDifficulty::from_str("困难"),
            Some(RecommendDifficulty::Difficult)
        ));

        // 测试无效输入
        assert!(RecommendDifficulty::from_str("invalid").is_none());
    }

    #[test]
    fn test_rating_range() {
        let easy = RecommendDifficulty::Easy;
        let moderate = RecommendDifficulty::Moderate;
        let difficult = RecommendDifficulty::Difficult;

        // 测试rating 1500的用户
        let user_rating = 1500;

        let (min, max) = easy.get_rating_range(user_rating);
        assert_eq!(min, 1100); // 1500 - 400
        assert_eq!(max, 1400); // 1500 - 100

        let (min, max) = moderate.get_rating_range(user_rating);
        assert_eq!(min, 1300); // 1500 - 200
        assert_eq!(max, 1700); // 1500 + 200

        let (min, max) = difficult.get_rating_range(user_rating);
        assert_eq!(min, 1600); // 1500 + 100
        assert_eq!(max, 2100); // 1500 + 600

        // 测试边界情况 - 低rating用户
        let low_rating = 900;
        let (min, max) = easy.get_rating_range(low_rating);
        assert_eq!(min, 800); // 最小rating限制
        assert_eq!(max, 800); // 最小rating限制

        // 测试边界情况 - 高rating用户
        let high_rating = 3000;
        let (min, max) = difficult.get_rating_range(high_rating);
        assert_eq!(min, 3100); // 3000 + 100
        assert_eq!(max, 3500); // 最大rating限制
    }

    #[test]
    fn test_solve_probability() {
        let easy = RecommendDifficulty::Easy;
        let moderate = RecommendDifficulty::Moderate;
        let difficult = RecommendDifficulty::Difficult;

        assert!((easy.get_target_solve_probability() - 0.8).abs() < 1e-9);
        assert!((moderate.get_target_solve_probability() - 0.5).abs() < 1e-9);
        assert!((difficult.get_target_solve_probability() - 0.2).abs() < 1e-9);
    }

    #[test]
    fn test_valid_rating_values() {
        // 测试有效的rating值
        let valid_ratings = [800, 900, 1000, 1500, 2000, 3000, 3500];
        for rating in valid_ratings {
            assert!(
                (800..=3500).contains(&rating) && rating % 100 == 0,
                "Rating {} should be valid",
                rating
            );
        }

        // 测试无效的rating值
        let invalid_ratings = [799, 850, 3501, 1234];
        for rating in invalid_ratings {
            assert!(
                !((800..=3500).contains(&rating) && rating % 100 == 0),
                "Rating {} should be invalid",
                rating
            );
        }
    }
}
