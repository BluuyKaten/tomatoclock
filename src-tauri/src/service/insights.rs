//! 规则型学习分析（对齐上游 §7.6 /get/insights/rules-summary）
//!
//! 基于阈值/趋势生成学习问题与建议（纯本地）。
//! TODO(设计待确认)：severity 枚举未定义；当前使用 info / warn / critical。

use crate::db::DbPool;
use crate::domain::responses::*;
use crate::error::AppResult;
use crate::service::stats::StatsService;

pub struct InsightsService;

impl InsightsService {
    pub fn rules_summary(
        pool: &DbPool,
        user_id: i64,
        from: i64,
        to: i64,
    ) -> AppResult<RulesSummaryResponse> {
        let mut insights: Vec<InsightItem> = Vec::new();

        // 1. 高频分心小时：找出 count 最高的小时
        let hotspot = StatsService::distraction_hotspot(pool, user_id, from, to)?;
        if let Some(max_h) = hotspot.by_hour.iter().max_by_key(|h| h.count) {
            if max_h.count >= 3 {
                insights.push(InsightItem {
                    r#type: "high_distraction_hour".into(),
                    severity: "warn".into(),
                    message: format!(
                        "{} 点时段分心 {} 次，为今日最高",
                        max_h.hour, max_h.count
                    ),
                });
            }
        }

        // 2. 分心率过高：整体 distraction_rate > 1.5 视为偏高
        let overview = StatsService::overview(pool, user_id, from, to)?;
        if overview.distraction_rate > 1.5 && overview.completed_pomos > 0 {
            insights.push(InsightItem {
                r#type: "camera_distraction_high".into(),
                severity: "critical".into(),
                message: format!(
                    "本周平均每番茄分心 {:.1} 次，专注质量需提升",
                    overview.distraction_rate
                ),
            });
        }

        // 3. 时长下降：与上一个相同长度窗口对比（简化：与本窗口前半段对比）
        let mid = (from + to) / 2;
        let first_half = StatsService::overview(pool, user_id, from, mid)?;
        let second_half = StatsService::overview(pool, user_id, mid, to)?;
        if first_half.total_minutes > 0
            && second_half.total_minutes < first_half.total_minutes / 2
        {
            insights.push(InsightItem {
                r#type: "duration_decline".into(),
                severity: "warn".into(),
                message: format!(
                    "后半段专注时长 {} 分钟，较前半段 {} 分钟明显下降",
                    second_half.total_minutes, first_half.total_minutes
                ),
            });
        }

        // 4. 连续记录（正向激励）：若连续 3 天以上有完成番茄
        let trend = StatsService::trend(pool, user_id, from, to, "day")?;
        let mut streak = 0i64;
        for p in &trend.points {
            if p.pomodoros > 0 {
                streak += 1;
            } else {
                streak = 0;
            }
        }
        if streak >= 3 {
            insights.push(InsightItem {
                r#type: "streak_record".into(),
                severity: "info".into(),
                message: format!("连续 {} 天完成番茄，保持节奏！", streak),
            });
        }

        // 5. 科目忽视：有科目超过 7 天未被专注（简化：本窗口内未出现的已有科目）
        // 这里仅做占位：统计本窗口内科目数是否 <=1
        if overview.subject_distribution.len() <= 1 && overview.completed_pomos >= 5 {
            insights.push(InsightItem {
                r#type: "subject_neglect".into(),
                severity: "info".into(),
                message: "近期仅涉及单一科目，注意合理切换避免疲劳".into(),
            });
        }

        if insights.is_empty() {
            insights.push(InsightItem {
                r#type: "no_issue".into(),
                severity: "info".into(),
                message: "近期学习状态良好，继续保持！".into(),
            });
        }

        Ok(RulesSummaryResponse { insights })
    }
}
