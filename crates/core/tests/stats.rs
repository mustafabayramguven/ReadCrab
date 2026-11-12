mod common;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use common::*;
use readcrab_core::{Core, CoreError};
use serde_json::{json, Value};

const TODAY: &str = "2026-11-08";

fn summary() -> Value {
    json!({
        "total_seconds": 3600, "total_pages": 120, "finished_count": 3,
        "streak_current": 4, "streak_longest": 9, "streak_at_risk": true,
        "today_seconds": 120, "streak_min_seconds": 300, "freeze_available": true,
        "daily": [], "goal": {"year": 2026, "target": null, "done": 0}
    })
}

fn heat(year: i32) -> Value {
    let days = if year == 2026 {
        json!([
            {"date": "2026-11-03", "seconds": 300, "pages": 4, "qualified": true,
             "frozen": false, "in_current": true},
            {"date": "2026-11-04", "seconds": 0, "pages": 0, "qualified": false,
             "frozen": true, "in_current": true}
        ])
    } else {
        json!([
            {"date": "2025-12-31", "seconds": 0, "pages": 0, "qualified": false,
             "frozen": true, "in_current": false}
        ])
    };
    json!({"year": year, "years": [2025, 2026], "streak_min_seconds": 300, "days": days})
}

fn year_of(path: &str) -> i32 {
    let rest = path.split("year=").nth(1).unwrap();
    rest[..4].parse().unwrap()
}

/// Giriş + istatistik uçları; `unauthorized` açılınca istatistik uçları 401 döner.
fn handler(unauthorized: Arc<AtomicBool>) -> impl Fn(&Req) -> Reply + Send + Sync + 'static {
    move |req: &Req| {
        if is_login(req) {
            return login_ok();
        }
        if !has_cookie(req) || unauthorized.load(Ordering::SeqCst) {
            return login_fail();
        }
        let p = req.path.as_str();
        if p == format!("/api/stats?today={TODAY}") {
            return json(200, &summary());
        }
        if p.starts_with("/api/stats/heatmap?year=") {
            return json(200, &heat(year_of(p)));
        }
        json(404, &json!({"error": "not_found", "message": "Bulunamadı."}))
    }
}

fn logged_in(mock: &Mock) -> (tempfile::TempDir, Arc<Core>) {
    let dir = tempfile::tempdir().unwrap();
    let core = core_in(&dir);
    core.login(mock.base.clone(), "admin".into(), "password1234".into())
        .unwrap();
    (dir, core)
}

#[test]
fn stats_load_parses_and_caches() {
    let flag = Arc::new(AtomicBool::new(false));
    let mock = Mock::start(handler(flag));
    let (_d, core) = logged_in(&mock);

    let s = core.stats_load(2026, TODAY.into()).unwrap();
    assert_eq!((s.total_seconds, s.total_pages, s.finished_count), (3600, 120, 3));
    assert_eq!((s.streak_current, s.streak_longest), (4, 9));
    assert!(s.streak_at_risk);
    assert_eq!((s.today_seconds, s.streak_min_seconds), (120, 300));
    assert!(s.freeze_available);
    assert_eq!(s.year, 2026);
    assert_eq!(s.years, vec![2025, 2026]);
    assert!(!s.from_cache);
    assert_eq!(s.days.len(), 2);
    assert!(s.days[0].qualified && !s.days[0].frozen && s.days[0].in_current);
    assert!(!s.days[1].qualified && s.days[1].frozen && s.days[1].in_current);
    assert_eq!(s.days[1].date, "2026-11-04");

    // today sorgusu ve çerez isteklerde var
    let reqs = mock.requests();
    let sum = reqs
        .iter()
        .find(|r| r.path == format!("/api/stats?today={TODAY}"))
        .expect("özet isteği");
    let hm = reqs
        .iter()
        .find(|r| r.path == format!("/api/stats/heatmap?year=2026&today={TODAY}"))
        .expect("ısı haritası isteği");
    assert!(has_cookie(sum) && has_cookie(hm));
    mock.stop();
}

#[test]
fn stats_load_falls_back_to_cache_when_offline() {
    let flag = Arc::new(AtomicBool::new(false));
    let mock = Mock::start(handler(flag));
    let (_d, core) = logged_in(&mock);
    let (_d2, core2) = logged_in(&mock); // önbelleği olmayan ikinci çekirdek
    let first = core.stats_load(2026, TODAY.into()).unwrap();
    mock.stop();

    let again = core.stats_load(2026, TODAY.into()).unwrap();
    assert!(again.from_cache);
    assert_eq!(again.fetched_at, first.fetched_at);
    assert_eq!(again.total_seconds, first.total_seconds);
    assert_eq!(again.days.len(), first.days.len());

    assert!(matches!(
        core2.stats_load(2026, TODAY.into()),
        Err(CoreError::Network(_))
    ));
}

#[test]
fn stats_cached_is_per_year() {
    let flag = Arc::new(AtomicBool::new(false));
    let mock = Mock::start(handler(flag));
    let (_d, core) = logged_in(&mock);
    assert!(core.stats_cached(2026).unwrap().is_none());

    core.stats_load(2026, TODAY.into()).unwrap();
    assert!(core.stats_cached(2026).unwrap().is_some());
    assert!(core.stats_cached(2025).unwrap().is_none());

    core.stats_load(2025, TODAY.into()).unwrap();
    let c25 = core.stats_cached(2025).unwrap().unwrap();
    assert!(c25.from_cache);
    assert!(c25.days[0].frozen);
    assert!(core.stats_cached(2026).unwrap().is_some());
    mock.stop();
}

#[test]
fn stats_old_server_without_new_fields() {
    let mock = Mock::start(|req: &Req| {
        if is_login(req) {
            return login_ok();
        }
        if req.path.starts_with("/api/stats/heatmap") {
            return json(
                200,
                &json!({"year": 2026, "years": [2026],
                        "days": [{"date": "2026-11-03", "seconds": 50, "pages": 1}]}),
            );
        }
        json(
            200,
            &json!({"total_seconds": 10, "total_pages": 2, "finished_count": 0,
                    "streak_current": 1, "streak_longest": 1, "daily": []}),
        )
    });
    let (_d, core) = logged_in(&mock);
    let s = core.stats_load(2026, TODAY.into()).unwrap();
    assert_eq!(s.streak_min_seconds, 300);
    assert!(!s.streak_at_risk && !s.freeze_available);
    assert_eq!(s.today_seconds, 0);
    let d = &s.days[0];
    assert_eq!((d.seconds, d.pages), (50, 1));
    assert!(!d.qualified && !d.frozen && !d.in_current);
    mock.stop();
}

#[test]
fn stats_unauthorized_does_not_use_cache() {
    let flag = Arc::new(AtomicBool::new(false));
    let mock = Mock::start(handler(flag.clone()));
    let (_d, core) = logged_in(&mock);
    core.stats_load(2026, TODAY.into()).unwrap();
    assert!(core.stats_cached(2026).unwrap().is_some());

    flag.store(true, Ordering::SeqCst);
    assert!(matches!(
        core.stats_load(2026, TODAY.into()),
        Err(CoreError::Unauthorized)
    ));
    mock.stop();
}

#[test]
fn stats_invalid_arguments() {
    let flag = Arc::new(AtomicBool::new(false));
    let mock = Mock::start(handler(flag));
    let (_d, core) = logged_in(&mock);
    assert!(matches!(
        core.stats_load(1999, TODAY.into()),
        Err(CoreError::InvalidInput(_))
    ));
    assert!(matches!(
        core.stats_load(2101, TODAY.into()),
        Err(CoreError::InvalidInput(_))
    ));
    assert!(matches!(
        core.stats_load(2026, "x".into()),
        Err(CoreError::InvalidInput(_))
    ));
    assert!(matches!(
        core.stats_load(2026, "2026-1-8".into()),
        Err(CoreError::InvalidInput(_))
    ));
    mock.stop();
}

#[test]
fn logout_wipes_stats_cache() {
    let flag = Arc::new(AtomicBool::new(false));
    let mock = Mock::start(handler(flag));
    let (_d, core) = logged_in(&mock);
    core.stats_load(2026, TODAY.into()).unwrap();
    assert!(core.stats_cached(2026).unwrap().is_some());

    core.logout().unwrap();
    assert!(core.stats_cached(2026).unwrap().is_none());
    mock.stop();
}
