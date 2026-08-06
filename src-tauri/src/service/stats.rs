//! 统计服务（对齐上游 §7.6）

use std::collections::HashMap;

use chrono::{DateTime, Datelike};

use crate::db::DbPool;
use crate::domain::responses::*;
use crate::error::AppResult;
use crate::repository::distractions::DistractionRepo;
use crate::repository::pomodoros::PomodoroRepo;

pub struct StatsService;

impl StatsService {
    /// 总览
    pub fn overview(pool: &DbPool, user_id: i64, from: i64, to: i64) -> AppResult<OverviewResponse> {
        let pomos = PomodoroRepo::list_by_time_range(pool, user_id, from, to)?;
        let dists = DistractionRepo::list_by_time_range(pool, user_id, from, to)?;

        let completed = pomos.iter().filter(|p| p.status == 1).count() as i64;
        let abandoned = pomos.iter().filter(|p| p.status == 2).count() as i64;
        let total_secs: i64 = pomos.iter().filter(|p| p.status == 1)
            .filter_map(|p| p.actual_duration).sum();
        let total_minutes = total_secs / 60;
        let distraction_count = dists.len() as i64;

        // TODO(设计待确认 #7)：distraction_rate 公式未定义；当前 = distraction_count / max(completed,1)
        let distraction_rate = if completed > 0 {
            distraction_count as f64 / completed as f64
        } else {
            0.0
        };

        // 科目分布：按 subject_id 聚合实际时长
        let mut by_subject: HashMap<Option<i64>, i64> = HashMap::new();
        for p in &pomos {
            if p.status == 1 {
                *by_subject.entry(p.subject_id).or_default() += p.actual_duration.unwrap_or(0);
            }
        }
        let mut dists_vec: Vec<SubjectDistribution> = by_subject
            .into_iter()
            .map(|(sid, secs)| {
                let name = sid
                    .and_then(|id| crate::repository::subjects::SubjectRepo::find_by_id(pool, id).ok().flatten())
                    .map(|s| s.name)
                    .unwrap_or_else(|| "未分类".to_string());
                SubjectDistribution {
                    subject_id: sid,
                    name,
                    minutes: secs / 60,
                }
            })
            .collect();
        dists_vec.sort_by_key(|d| -d.minutes);

        Ok(OverviewResponse {
            total_minutes,
            completed_pomos: completed,
            abandoned_pomos: abandoned,
            distraction_count,
            distraction_rate,
            subject_distribution: dists_vec,
        })
    }

    /// 趋势
    pub fn trend(
        pool: &DbPool,
        user_id: i64,
        from: i64,
        to: i64,
        granularity: &str,
    ) -> AppResult<TrendResponse> {
        let pomos = PomodoroRepo::list_by_time_range(pool, user_id, from, to)?;
        let dists = DistractionRepo::list_by_time_range(pool, user_id, from, to)?;

        // 按"粒度起始日期"聚合
        let mut map: HashMap<String, (i64, i64, i64)> = HashMap::new(); // date -> (minutes, pomodoros, distractions)

        for p in &pomos {
            if p.status == 1 {
                let bucket = bucket_key(p.started_at, granularity);
                let e = map.entry(bucket).or_default();
                e.0 += p.actual_duration.unwrap_or(0) / 60;
                e.1 += 1;
            }
        }
        for d in &dists {
            let bucket = bucket_key(d.detected_at, granularity);
            map.entry(bucket).or_default().2 += 1;
        }

        let mut points: Vec<TrendPoint> = map
            .into_iter()
            .map(|(date, (minutes, pomodoros, distractions))| TrendPoint {
                date,
                minutes,
                pomodoros,
                distractions,
            })
            .collect();
        points.sort_by(|a, b| a.date.cmp(&b.date));

        Ok(TrendResponse { points })
    }

    /// 分心热点
    pub fn distraction_hotspot(
        pool: &DbPool,
        user_id: i64,
        from: i64,
        to: i64,
    ) -> AppResult<DistractionHotspotResponse> {
        let by_app = DistractionRepo::count_by_app(pool, user_id, from, to)?
            .into_iter()
            .map(|(app_name, count)| AppHotspot { app_name, count })
            .collect();

        let by_hour = DistractionRepo::count_by_hour(pool, user_id, from, to)?
            .into_iter()
            .map(|(hour, count)| HourHotspot { hour, count })
            .collect();

        let by_type = DistractionRepo::count_by_type(pool, user_id, from, to)?
            .into_iter()
            .map(|(t, count)| TypeHotspot { r#type: t, count })
            .collect();

        Ok(DistractionHotspotResponse { by_app, by_hour, by_type })
    }
}

/// 把 Unix 毫秒按粒度映射到桶起始日期字符串
fn bucket_key(ms: i64, granularity: &str) -> String {
    let secs = ms / 1000;
    // [FIX] DateTime::from_timestamp 替代已弃用的 NaiveDateTime::from_timestamp_opt
    let dt = DateTime::from_timestamp(secs, 0).unwrap_or_default();
    let date = dt.date_naive();

    match granularity {
        "week" => {
            // 本周一
            let wd = date.weekday().num_days_from_monday() as i32;
            let monday = date - chrono::Duration::days(wd as i64);
            monday.format("%Y-%m-%d").to_string()
        }
        "month" => date.format("%Y-%m-01").to_string(),
        _ => date.format("%Y-%m-%d").to_string(), // day
    }
}
