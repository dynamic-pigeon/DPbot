use anyhow::Result;
use kovi::MsgEvent;
use rand::Rng;
use rand::seq::SliceRandom;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::{
    duel::{problem::get_problems, submission::get_recent_submissions},
    utils::{IdOrText, get_user_rating},
};

// 常量定义
const MAX_RECOMMEND_COUNT: usize = 10;
const MIN_RATING: i64 = 800;
const MAX_RATING: i64 = 3500;
const RATING_STEP: i64 = 100;

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
            Self::Easy => (
                (user_rating - 400).max(MIN_RATING),
                (user_rating - 100).max(MIN_RATING),
            ),
            Self::Moderate => (
                (user_rating - 200).max(MIN_RATING),
                (user_rating + 200).min(MAX_RATING),
            ),
            Self::Difficult => (
                (user_rating + 100).min(MAX_RATING),
                (user_rating + 600).min(MAX_RATING),
            ),
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

pub async fn recommend(event: &MsgEvent, args: &[String]) {
    let cmd_args = parse_args(args);
    let user = IdOrText::At(event.user_id);

    // 获取用户CF ID和rating
    let (cf_id, rating) = match get_user_info(&user).await {
        Ok(info) => info,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    // 确定推荐范围并通知用户
    let (min_rating, max_rating, filter_msg) = get_rating_range(&cmd_args, rating);
    event.reply(format!("正在为您推荐{}", filter_msg));

    // 获取用户提交数据和题库
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
        let msg = if cmd_args.exclude_solved {
            "没有找到合适的题目（已排除解决过的题目）"
        } else {
            "没有找到合适的题目"
        };
        event.reply(msg);
        return;
    }

    // 选择题目
    let final_problems = select_problems(&candidates, &tag_weight, &cmd_args, rating);

    // 发送结果
    let msg = format_recommendation_output(&final_problems, &cmd_args);
    event.reply(msg);
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
            count = c.min(MAX_RECOMMEND_COUNT);
            i += 1;
        } else if (arg == "--rating" || arg == "-r")
            && i + 1 < args.len()
            && let Ok(r) = args[i + 1].parse::<i64>()
            && (MIN_RATING..=MAX_RATING).contains(&r)
            && r % RATING_STEP == 0
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

// 选择推荐题目
fn select_problems<'a>(
    candidates: &[&'a Arc<crate::duel::problem::Problem>],
    tag_weight: &HashMap<String, usize>,
    cmd_args: &CommandArgs,
    rating: i64,
) -> Vec<&'a Arc<crate::duel::problem::Problem>> {
    let mut rng = rand::rng();
    if cmd_args.specific_rating.is_some() {
        // 指定rating：随机选择
        let mut shuffled = candidates.to_vec();
        shuffled.shuffle(&mut rng);
        shuffled.into_iter().take(cmd_args.count).collect()
    } else {
        // 根据难度：加权随机选择
        let target_rating = match cmd_args.difficulty {
            RecommendDifficulty::Easy => rating - 250,
            RecommendDifficulty::Moderate => rating,
            RecommendDifficulty::Difficult => rating + 350,
        };

        let weights: Vec<f64> = candidates
            .iter()
            .map(|p| calculate_problem_weight(p, tag_weight, target_rating))
            .collect();

        weighted_random_select(candidates, &weights, cmd_args.count, &mut rng)
    }
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
            // 检查rating是否在范围内
            let rating_in_range = p
                .rating
                .map(|r| (min_rating..=max_rating).contains(&r))
                .unwrap_or(false);

            // 检查是否已解决
            let not_solved = solved_problems
                .as_ref()
                .map(|solved| !solved.contains(&(p.contest_id, p.index.clone())))
                .unwrap_or(true);

            // 检查是否为特殊题目
            let not_special = !p.tags.iter().any(|tag| tag == "*special");

            rating_in_range && not_solved && not_special
        })
        .collect()
}

// 格式化推荐结果输出
fn format_recommendation_output(
    problems: &[&Arc<crate::duel::problem::Problem>],
    cmd_args: &CommandArgs,
) -> String {
    match problems {
        [problem] => format_single_problem(problem),
        problems => format_multiple_problems(problems, cmd_args),
    }
}

// 格式化单个题目
fn format_single_problem(problem: &Arc<crate::duel::problem::Problem>) -> String {
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
}

// 格式化多个题目
fn format_multiple_problems(
    problems: &[&Arc<crate::duel::problem::Problem>],
    cmd_args: &CommandArgs,
) -> String {
    let title = match cmd_args.specific_rating {
        Some(r) => format!("推荐的rating {} 题目：\n", r),
        None => format!("推荐的{:?}难度题目：\n", cmd_args.difficulty),
    };

    problems
        .iter()
        .enumerate()
        .fold(title, |mut acc, (i, problem)| {
            let link = format!(
                "https://codeforces.com/problemset/problem/{}/{}",
                problem.contest_id, problem.index
            );
            acc.push_str(&format!(
                "{}. {} {} (难度: {})\n   标签: {}\n   链接: {}\n\n",
                i + 1,
                problem.contest_id,
                problem.index,
                problem.rating.unwrap_or(0),
                problem.tags.join(", "),
                link
            ));
            acc
        })
}

// 获取用户提交记录相关的数据
async fn get_user_submission_data(
    cf_id: &str,
    exclude_solved: bool,
) -> (HashMap<String, usize>, Option<HashSet<(i64, String)>>) {
    let Ok(submissions) = get_recent_submissions(cf_id).await else {
        return (HashMap::new(), None);
    };

    // 统计用户未通过题目的 tag 权重
    let tag_weight = submissions
        .iter()
        .filter(|s| !s.is_accepted())
        .flat_map(|s| &s.problem.tags)
        .fold(HashMap::new(), |mut acc, tag| {
            *acc.entry(tag.clone()).or_insert(0) += 1;
            acc
        });

    // 如果需要排除已解决题目，处理已解决集合
    let solved_problems = if exclude_solved {
        let solved: HashSet<_> = submissions
            .iter()
            .filter(|s| s.is_accepted())
            .map(|s| (s.problem.contest_id, s.problem.index.clone()))
            .collect();

        (!solved.is_empty()).then_some(solved)
    } else {
        None
    };

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

// 加权随机选择题目
fn weighted_random_select<'a>(
    candidates: &[&'a Arc<crate::duel::problem::Problem>],
    weights: &[f64],
    count: usize,
    rng: &mut impl Rng,
) -> Vec<&'a Arc<crate::duel::problem::Problem>> {
    let mut indices: Vec<usize> = (0..candidates.len()).collect();
    let desired_count = count.min(candidates.len());
    let mut selected_indices = Vec::with_capacity(desired_count);

    while selected_indices.len() < desired_count && !indices.is_empty() {
        let total_weight: f64 = indices.iter().map(|&i| weights[i]).sum();

        // 如果所有权重为零或接近零，则随机选择剩余题目
        if total_weight <= f64::EPSILON {
            indices.shuffle(rng);
            let remaining = desired_count - selected_indices.len();
            selected_indices.extend(indices.iter().take(remaining).copied());
            break;
        }

        // 加权随机选择一个题目
        let pick = rng.random_range(0.0..total_weight);
        let selected_pos = indices
            .iter()
            .enumerate()
            .scan(0.0, |acc, (pos, &idx)| {
                *acc += weights[idx];
                Some((pos, *acc))
            })
            .find(|(_, cumulative)| *cumulative >= pick)
            .map(|(pos, _)| pos)
            .unwrap_or_else(|| rng.random_range(0..indices.len()));

        selected_indices.push(indices.remove(selected_pos));
    }

    selected_indices.iter().map(|&i| candidates[i]).collect()
}

// 计算题目权重
fn calculate_problem_weight(
    problem: &crate::duel::problem::Problem,
    tag_weight: &HashMap<String, usize>,
    target_rating: i64,
) -> f64 {
    let score = calculate_tag_score(problem, tag_weight) as f64;
    let diff = (problem.rating.unwrap_or(target_rating) - target_rating).abs() as f64;
    let rating_bonus = 1.0 / (1.0 + diff / 100.0);
    (score + 1.0) * rating_bonus
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

// 获取用户信息（CF ID 和 rating）
async fn get_user_info(user: &IdOrText<'_>) -> Result<(String, i64)> {
    let cf_id = get_cf_id(user).await?;
    let rating = get_user_rating(&cf_id)
        .await
        .map_err(|e| anyhow::anyhow!("获取rating失败: {}", e))?;
    Ok((cf_id, rating))
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
        assert_eq!(min, MIN_RATING); // 最小rating限制
        assert_eq!(max, MIN_RATING); // 最小rating限制

        // 测试边界情况 - 高rating用户
        let high_rating = 3000;
        let (min, max) = difficult.get_rating_range(high_rating);
        assert_eq!(min, 3100); // 3000 + 100
        assert_eq!(max, MAX_RATING); // 最大rating限制
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
        let valid_ratings = [MIN_RATING, 900, 1000, 1500, 2000, 3000, MAX_RATING];
        for rating in valid_ratings {
            assert!(
                (MIN_RATING..=MAX_RATING).contains(&rating) && rating % RATING_STEP == 0,
                "Rating {} should be valid",
                rating
            );
        }

        // 测试无效的rating值
        let invalid_ratings = [MIN_RATING - 1, 850, MAX_RATING + 1, 1234];
        for rating in invalid_ratings {
            assert!(
                !((MIN_RATING..=MAX_RATING).contains(&rating) && rating % RATING_STEP == 0),
                "Rating {} should be invalid",
                rating
            );
        }
    }
}
