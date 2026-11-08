use std::collections::{HashMap, HashSet};

use axum::{
    extract::{Query, State},
    routing::{get, put},
    Json, Router,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{
    api::tracking::parse_local_date,
    app::AppState,
    auth::AuthUser,
    error::{AppError, JsonBody},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/stats", get(get_stats))
        .route("/api/goal", put(put_goal))
}

pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let m = u64::from(m);
    let d = u64::from(d);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y as i32, m as u32, d as u32)
}

#[derive(Deserialize)]
struct StatsQuery {
    today: Option<String>,
}

#[derive(Serialize)]
struct DailyStat {
    date: String,
    seconds: i64,
    pages: i64,
}

#[derive(Serialize)]
struct GoalStat {
    year: i32,
    target: Option<i64>,
    done: i64,
}

#[derive(Serialize)]
struct StatsRes {
    total_seconds: i64,
    total_pages: i64,
    finished_count: i64,
    streak_current: i64,
    streak_longest: i64,
    daily: Vec<DailyStat>,
    goal: GoalStat,
}

async fn get_stats(
    user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<StatsRes>, AppError> {
    let Some(today_str) = query.today else {
        return Err(AppError::BadRequest("today parametresi zorunludur."));
    };
    let (year, month, day) = parse_local_date(&today_str)?;
    let today_days = days_from_civil(year, month, day);

    let user_id = user.id;
    let res = state
        .db
        .run(move |conn| {
            let (total_seconds, total_pages): (i64, i64) = conn.query_row(
                "SELECT COALESCE(SUM(active_seconds), 0), COALESCE(SUM(pages_read), 0)
                 FROM reading_sessions WHERE user_id = ?1",
                rusqlite::params![user_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;

            let finished_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM progress WHERE user_id = ?1 AND status = 'finished'",
                rusqlite::params![user_id],
                |r| r.get(0),
            )?;

            let mut stmt = conn.prepare(
                "SELECT local_date, SUM(active_seconds), SUM(pages_read)
                 FROM reading_sessions
                 WHERE user_id = ?1
                 GROUP BY local_date",
            )?;
            let grouped = stmt
                .query_map(rusqlite::params![user_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            let mut by_date: HashMap<String, (i64, i64)> = HashMap::new();
            let mut active_days: Vec<i64> = Vec::new();
            let mut active_set: HashSet<i64> = HashSet::new();

            for (d_str, secs, pages) in grouped {
                by_date.insert(d_str.clone(), (secs, pages));
                if secs >= 60 {
                    if let Ok((y, m, d)) = parse_local_date(&d_str) {
                        let day_num = days_from_civil(y, m, d);
                        active_days.push(day_num);
                        active_set.insert(day_num);
                    }
                }
            }

            active_days.sort_unstable();
            active_days.dedup();

            let mut streak_longest = 0i64;
            let mut cur_run = 0i64;
            let mut prev_day: Option<i64> = None;
            for &d in &active_days {
                if prev_day == Some(d - 1) {
                    cur_run += 1;
                } else {
                    cur_run = 1;
                }
                if cur_run > streak_longest {
                    streak_longest = cur_run;
                }
                prev_day = Some(d);
            }

            let mut streak_current = 0i64;
            let start_check = if active_set.contains(&today_days) {
                Some(today_days)
            } else if active_set.contains(&(today_days - 1)) {
                Some(today_days - 1)
            } else {
                None
            };
            if let Some(mut d) = start_check {
                while active_set.contains(&d) {
                    streak_current += 1;
                    d -= 1;
                }
            }

            let mut daily = Vec::with_capacity(30);
            for d in (today_days - 29)..=today_days {
                let (cy, cm, cd) = civil_from_days(d);
                let date_key = format!("{cy:04}-{cm:02}-{cd:02}");
                let (seconds, pages) = by_date.get(&date_key).copied().unwrap_or((0, 0));
                daily.push(DailyStat {
                    date: date_key,
                    seconds,
                    pages,
                });
            }

            let target: Option<i64> = conn
                .query_row(
                    "SELECT target FROM goals WHERE user_id = ?1 AND year = ?2",
                    rusqlite::params![user_id, i64::from(year)],
                    |r| r.get(0),
                )
                .optional()?;

            let year_prefix = format!("{year:04}-%");
            let done: i64 = conn.query_row(
                "SELECT COUNT(*) FROM progress
                 WHERE user_id = ?1 AND status = 'finished' AND finished_date LIKE ?2",
                rusqlite::params![user_id, year_prefix],
                |r| r.get(0),
            )?;

            Ok(StatsRes {
                total_seconds,
                total_pages,
                finished_count,
                streak_current,
                streak_longest,
                daily,
                goal: GoalStat { year, target, done },
            })
        })
        .await?;

    Ok(Json(res))
}

#[derive(Deserialize)]
struct PutGoalReq {
    year: i32,
    target: i64,
}

#[derive(Serialize)]
struct PutGoalRes {
    year: i32,
    target: i64,
}

async fn put_goal(
    user: AuthUser,
    State(state): State<AppState>,
    JsonBody(req): JsonBody<PutGoalReq>,
) -> Result<Json<PutGoalRes>, AppError> {
    if !(2000..=2100).contains(&req.year) || !(1..=1000).contains(&req.target) {
        return Err(AppError::BadRequest("Geçersiz hedef değerleri."));
    }

    let user_id = user.id;
    let year = req.year;
    let target = req.target;

    state
        .db
        .run(move |conn| {
            conn.execute(
                "INSERT INTO goals(user_id, year, target)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(user_id, year) DO UPDATE SET target = excluded.target",
                rusqlite::params![user_id, i64::from(year), target],
            )?;
            Ok(())
        })
        .await?;

    Ok(Json(PutGoalRes { year, target }))
}
