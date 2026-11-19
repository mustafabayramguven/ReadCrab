use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

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
        .route("/api/stats/heatmap", get(get_heatmap))
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

pub const STREAK_MIN_SECONDS: i64 = 300;

/// Pazartesi başlangıçlı takvim haftası numarası (1970-01-01 Perşembe).
pub fn week_of(day: i64) -> i64 {
    (day + 3).div_euclid(7)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Streaks {
    pub current: i64,
    pub longest: i64,
    pub at_risk: bool,
    pub freeze_available: bool,
    pub frozen: BTreeSet<i64>,
    pub current_days: BTreeSet<i64>,
}

struct Chain {
    days: Vec<i64>,
    frozen: Vec<i64>,
}

/// Nitelikli günlerden zincirleri kurar; haftada en çok bir dondurma hakkı harcanır.
pub fn compute_streaks(qualified: &BTreeSet<i64>, today: i64) -> Streaks {
    let mut used: HashSet<i64> = HashSet::new();
    let mut frozen: BTreeSet<i64> = BTreeSet::new();
    let mut chains: Vec<Chain> = Vec::new();
    for &d in qualified.iter().filter(|&&d| d <= today) {
        if chains.is_empty() {
            chains.push(Chain {
                days: vec![d],
                frozen: vec![],
            });
            continue;
        }
        let last = chains.last_mut().unwrap();
        let prev = *last.days.last().unwrap();
        if d == prev + 1 {
            last.days.push(d);
        } else if d == prev + 2 && !used.contains(&week_of(prev + 1)) {
            used.insert(week_of(prev + 1));
            frozen.insert(prev + 1);
            last.frozen.push(prev + 1);
            last.days.push(d);
        } else {
            chains.push(Chain {
                days: vec![d],
                frozen: vec![],
            });
        }
    }
    let longest = chains.iter().map(|c| c.days.len() as i64).max();
    let mut out = Streaks {
        longest: longest.unwrap_or(0),
        ..Default::default()
    };
    if let Some(c) = chains.last_mut() {
        let l = *c.days.last().unwrap();
        let state = if l == today {
            Some(false)
        } else if l == today - 1 {
            Some(true)
        } else if l == today - 2 && !used.contains(&week_of(today - 1)) {
            used.insert(week_of(today - 1));
            frozen.insert(today - 1);
            c.frozen.push(today - 1);
            Some(true)
        } else {
            None
        };
        if let Some(risk) = state {
            out.current = c.days.len() as i64;
            out.at_risk = risk;
            out.current_days = c.days.iter().chain(c.frozen.iter()).copied().collect();
        }
    }
    out.freeze_available = !used.contains(&week_of(today));
    out.frozen = frozen;
    out
}

fn day_number(date: &str) -> Option<i64> {
    match parse_local_date(date) {
        Ok((y, m, d)) => Some(days_from_civil(y, m, d)),
        Err(_) => None,
    }
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
    streak_at_risk: bool,
    today_seconds: i64,
    streak_min_seconds: i64,
    freeze_available: bool,
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
            let mut qualified: BTreeSet<i64> = BTreeSet::new();
            for (d_str, secs, pages) in grouped {
                by_date.insert(d_str.clone(), (secs, pages));
                if secs >= STREAK_MIN_SECONDS {
                    if let Ok((y, m, d)) = parse_local_date(&d_str) {
                        qualified.insert(days_from_civil(y, m, d));
                    }
                }
            }
            let streaks = compute_streaks(&qualified, today_days);
            let (ty, tm, td) = civil_from_days(today_days);
            let today_seconds = by_date
                .get(&format!("{ty:04}-{tm:02}-{td:02}"))
                .map_or(0, |v| v.0);

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
                streak_current: streaks.current,
                streak_longest: streaks.longest,
                streak_at_risk: streaks.at_risk,
                today_seconds,
                streak_min_seconds: STREAK_MIN_SECONDS,
                freeze_available: streaks.freeze_available,
                daily,
                goal: GoalStat { year, target, done },
            })
        })
        .await?;

    Ok(Json(res))
}

#[derive(Deserialize)]
struct HeatmapQuery {
    year: Option<String>,
    today: Option<String>,
}

#[derive(Serialize)]
struct HeatDay {
    date: String,
    seconds: i64,
    pages: i64,
    qualified: bool,
    frozen: bool,
    in_current: bool,
}

#[derive(Serialize)]
struct HeatmapRes {
    year: i32,
    years: Vec<i32>,
    streak_min_seconds: i64,
    days: Vec<HeatDay>,
}

async fn get_heatmap(
    user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<HeatmapQuery>,
) -> Result<Json<HeatmapRes>, AppError> {
    let year: i32 = query
        .year
        .and_then(|y| y.parse().ok())
        .filter(|y| (2000..=2100).contains(y))
        .ok_or(AppError::BadRequest("Geçersiz yıl."))?;
    let mut today_days: Option<i64> = None;
    if let Some(t) = &query.today {
        let (y, m, d) = parse_local_date(t)?;
        today_days = Some(days_from_civil(y, m, d));
    }
    let user_id = user.id;
    let res = state
        .db
        .run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT local_date, SUM(active_seconds), SUM(pages_read)
                 FROM reading_sessions WHERE user_id = ?1 GROUP BY local_date",
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
            let mut by_date: BTreeMap<String, (i64, i64)> = BTreeMap::new();
            let mut qualified: BTreeSet<i64> = BTreeSet::new();
            for (d_str, secs, pages) in grouped {
                if secs >= STREAK_MIN_SECONDS {
                    if let Some(n) = day_number(&d_str) {
                        qualified.insert(n);
                    }
                }
                by_date.insert(d_str, (secs, pages));
            }
            // today yoksa zincir ölü sayılır (in_current hep false)
            let dead_today = qualified.last().map_or(0, |l| l + 3);
            let streaks = compute_streaks(&qualified, today_days.unwrap_or(dead_today));
            let years: BTreeSet<i32> = by_date
                .keys()
                .filter_map(|k| k.get(0..4).and_then(|y| y.parse().ok()))
                .collect();

            let prefix = format!("{year:04}-");
            let mut days: Vec<HeatDay> = Vec::new();
            for (date, &(seconds, pages)) in &by_date {
                if !date.starts_with(&prefix) {
                    continue;
                }
                let day = day_number(date);
                days.push(HeatDay {
                    date: date.clone(),
                    seconds,
                    pages,
                    qualified: seconds >= STREAK_MIN_SECONDS,
                    frozen: day.is_some_and(|n| streaks.frozen.contains(&n)),
                    in_current: day.is_some_and(|n| streaks.current_days.contains(&n)),
                });
            }
            // verisi olmayan donmuş günler de listede yer alır
            for &n in &streaks.frozen {
                let (y, m, d) = civil_from_days(n);
                let date = format!("{y:04}-{m:02}-{d:02}");
                if y == year && !days.iter().any(|x| x.date == date) {
                    days.push(HeatDay {
                        date,
                        seconds: 0,
                        pages: 0,
                        qualified: false,
                        frozen: true,
                        in_current: streaks.current_days.contains(&n),
                    });
                }
            }
            days.sort_by(|a, b| a.date.cmp(&b.date));
            Ok(HeatmapRes {
                year,
                years: years.into_iter().collect(),
                streak_min_seconds: STREAK_MIN_SECONDS,
                days,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> i64 {
        days_from_civil(y, m, day)
    }

    fn nov(day: u32) -> i64 {
        d(2026, 11, day)
    }

    fn oct(day: u32) -> i64 {
        d(2026, 10, day)
    }

    fn set(v: &[i64]) -> BTreeSet<i64> {
        v.iter().copied().collect()
    }

    // 2026-11-08 Pazar: hafta A = 2-8 Kasım, B = 26 Eki-1 Kas, C = 9-15 Kasım

    #[test]
    fn t01_three_days_alive() {
        let s = compute_streaks(&set(&[nov(6), nov(7), nov(8)]), nov(8));
        assert_eq!((s.current, s.longest, s.at_risk), (3, 3, false));
        assert!(s.frozen.is_empty());
        assert!(s.freeze_available);
    }

    #[test]
    fn t02_today_missing_is_at_risk() {
        let s = compute_streaks(&set(&[nov(6), nov(7)]), nov(8));
        assert_eq!((s.current, s.longest, s.at_risk), (2, 2, true));
        assert!(s.frozen.is_empty());
        assert!(s.freeze_available);
    }

    #[test]
    fn t03_one_gap_is_frozen() {
        let s = compute_streaks(&set(&[nov(3), nov(5)]), nov(5));
        assert_eq!((s.current, s.longest, s.at_risk), (2, 2, false));
        assert_eq!(s.frozen, set(&[nov(4)]));
        assert!(!s.freeze_available);
    }

    #[test]
    fn t04_second_gap_in_same_week_breaks() {
        let s = compute_streaks(&set(&[nov(3), nov(5), nov(7)]), nov(7));
        assert_eq!((s.current, s.longest, s.at_risk), (1, 2, false));
        assert_eq!(s.frozen, set(&[nov(4)]));
        assert!(!s.freeze_available);
        assert_eq!(s.current_days, set(&[nov(7)]));
    }

    #[test]
    fn t05_gaps_in_different_weeks() {
        let s = compute_streaks(&set(&[oct(31), nov(2), nov(4)]), nov(4));
        assert_eq!((s.current, s.longest, s.at_risk), (3, 3, false));
        assert_eq!(s.frozen, set(&[nov(1), nov(3)]));
        assert!(!s.freeze_available);
        let days = set(&[oct(31), nov(1), nov(2), nov(3), nov(4)]);
        assert_eq!(s.current_days, days);
    }

    #[test]
    fn t06_two_empty_days_break() {
        let s = compute_streaks(&set(&[nov(2), nov(5)]), nov(5));
        assert_eq!((s.current, s.longest, s.at_risk), (1, 1, false));
        assert!(s.frozen.is_empty());
        assert!(s.freeze_available);
    }

    #[test]
    fn t07_pending_freeze_for_yesterday() {
        let s = compute_streaks(&set(&[nov(6)]), nov(8));
        assert_eq!((s.current, s.longest, s.at_risk), (1, 1, true));
        assert_eq!(s.frozen, set(&[nov(7)]));
        assert!(!s.freeze_available);
        assert_eq!(s.current_days, set(&[nov(6), nov(7)]));
    }

    #[test]
    fn t08_week_freeze_spent_kills_streak() {
        let s = compute_streaks(&set(&[nov(3), nov(5)]), nov(7));
        assert_eq!((s.current, s.longest, s.at_risk), (0, 2, false));
        assert_eq!(s.frozen, set(&[nov(4)]));
        assert!(!s.freeze_available);
    }

    #[test]
    fn t09_three_days_ago_is_dead() {
        let s = compute_streaks(&set(&[nov(5)]), nov(8));
        assert_eq!((s.current, s.longest, s.at_risk), (0, 1, false));
        assert!(s.frozen.is_empty());
        assert!(s.freeze_available);
    }

    #[test]
    fn t10_year_boundary_same_week() {
        let q = set(&[d(2025, 12, 30), d(2026, 1, 1)]);
        let s = compute_streaks(&q, d(2026, 1, 1));
        assert_eq!((s.current, s.longest, s.at_risk), (2, 2, false));
        assert_eq!(s.frozen, set(&[d(2025, 12, 31)]));
        assert!(!s.freeze_available);
    }

    #[test]
    fn t11_future_dates_are_ignored() {
        let s = compute_streaks(&set(&[nov(7), nov(9)]), nov(8));
        assert_eq!((s.current, s.longest, s.at_risk), (1, 1, true));
        assert!(s.frozen.is_empty());
        assert!(s.freeze_available);
    }

    #[test]
    fn t12_gaps_in_weeks_a_and_c() {
        let s = compute_streaks(&set(&[nov(7), nov(9), nov(11)]), nov(11));
        assert_eq!((s.current, s.longest, s.at_risk), (3, 3, false));
        assert_eq!(s.frozen, set(&[nov(8), nov(10)]));
        assert!(!s.freeze_available);
    }

    #[test]
    fn t13_longest_is_an_older_chain() {
        let mut q = set(&[nov(7)]);
        for day in 1..=5 {
            q.insert(oct(day));
        }
        let s = compute_streaks(&q, nov(8));
        assert_eq!((s.current, s.longest, s.at_risk), (1, 5, true));
        assert!(s.frozen.is_empty());
        assert!(s.freeze_available);
    }

    #[test]
    fn t14_empty() {
        let s = compute_streaks(&BTreeSet::new(), nov(8));
        assert_eq!((s.current, s.longest, s.at_risk), (0, 0, false));
        assert!(s.frozen.is_empty());
        assert!(s.freeze_available);
    }

    #[test]
    fn week_of_is_monday_based() {
        // 2026-11-02 Pazartesi ile 2026-11-08 Pazar aynı hafta; ertesi Pazartesi yeni hafta
        assert_eq!(week_of(nov(2)), week_of(nov(8)));
        assert_eq!(week_of(nov(9)), week_of(nov(8)) + 1);
        assert_eq!(week_of(d(1970, 1, 1)), 0);
    }
}
