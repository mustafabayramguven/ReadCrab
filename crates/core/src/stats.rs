use serde::Deserialize;
use serde_json::Value;

use crate::{db, error::into_core, http::HErr, Core, CoreError, StatsDay, StatsSnapshot};

#[derive(Deserialize, Default)]
struct SummaryDto {
    #[serde(default)]
    total_seconds: i64,
    #[serde(default)]
    total_pages: i64,
    #[serde(default)]
    finished_count: i64,
    #[serde(default)]
    streak_current: i64,
    #[serde(default)]
    streak_longest: i64,
    #[serde(default)]
    streak_at_risk: bool,
    #[serde(default)]
    today_seconds: i64,
    #[serde(default = "default_min")]
    streak_min_seconds: i64,
    #[serde(default)]
    freeze_available: bool,
}

fn default_min() -> i64 {
    300
}

#[derive(Deserialize)]
struct DayDto {
    date: String,
    #[serde(default)]
    seconds: i64,
    #[serde(default)]
    pages: i64,
    #[serde(default)]
    qualified: bool,
    #[serde(default)]
    frozen: bool,
    #[serde(default)]
    in_current: bool,
}

#[derive(Deserialize)]
struct HeatDto {
    #[serde(default)]
    year: i32,
    #[serde(default)]
    years: Vec<i32>,
    #[serde(default)]
    days: Vec<DayDto>,
}

fn unreadable() -> CoreError {
    CoreError::Server("Sunucu yanıtı anlaşılamadı.".to_string())
}

fn snapshot(
    summary: &Value,
    heat: &Value,
    at: i64,
    cached: bool,
) -> Result<StatsSnapshot, CoreError> {
    let s: SummaryDto = serde_json::from_value(summary.clone()).map_err(|_| unreadable())?;
    let h: HeatDto = serde_json::from_value(heat.clone()).map_err(|_| unreadable())?;
    Ok(StatsSnapshot {
        total_seconds: s.total_seconds,
        total_pages: s.total_pages,
        finished_count: s.finished_count,
        streak_current: s.streak_current,
        streak_longest: s.streak_longest,
        streak_at_risk: s.streak_at_risk,
        today_seconds: s.today_seconds,
        streak_min_seconds: s.streak_min_seconds,
        freeze_available: s.freeze_available,
        year: h.year,
        years: h.years,
        days: h
            .days
            .into_iter()
            .map(|d| StatsDay {
                date: d.date,
                seconds: d.seconds,
                pages: d.pages,
                qualified: d.qualified,
                frozen: d.frozen,
                in_current: d.in_current,
            })
            .collect(),
        fetched_at: at,
        from_cache: cached,
    })
}

fn read_cached(core: &Core, year: i32) -> Result<Option<StatsSnapshot>, CoreError> {
    let raw = core.with_db(|c| {
        Ok((
            db::kv_get(c, "stats_summary"),
            db::kv_get(c, &format!("stats_heat_{year}")),
            db::kv_get(c, "stats_summary_at"),
        ))
    })?;
    let (Some(s), Some(h)) = (raw.0, raw.1) else {
        return Ok(None);
    };
    let at = raw.2.and_then(|v| v.parse().ok()).unwrap_or(0);
    let (Ok(sv), Ok(hv)) = (
        serde_json::from_str::<Value>(&s),
        serde_json::from_str::<Value>(&h),
    ) else {
        return Ok(None);
    };
    snapshot(&sv, &hv, at, true).map(Some)
}

/// `YYYY-MM-DD` biçimi (takvim doğrulaması sunucuda).
fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
}

fn fetch(core: &Core, year: i32, today: &str) -> Result<(Value, Value), HErr> {
    let summary: Value = core.http.get_json(&format!("/api/stats?today={today}"))?;
    let heat: Value = core
        .http
        .get_json(&format!("/api/stats/heatmap?year={year}&today={today}"))?;
    Ok((summary, heat))
}

#[uniffi::export]
impl Core {
    /// Ağdan çeker ve önbelleğe yazar; ağ yoksa (Network/TLS) ve önbellek varsa onu döner
    /// (`from_cache = true`). `today` = `YYYY-MM-DD` (cihazın yerel tarihi); `year` 2000..=2100.
    pub fn stats_load(&self, year: i32, today: String) -> Result<StatsSnapshot, CoreError> {
        self.require_session()?;
        if !(2000..=2100).contains(&year) || !valid_date(&today) {
            return Err(CoreError::InvalidInput(
                "Geçersiz istatistik isteği.".to_string(),
            ));
        }
        match fetch(self, year, &today) {
            Ok((summary, heat)) => {
                let now = self.now();
                // çözülemeyen yanıt önbelleğe girmesin
                let snap = snapshot(&summary, &heat, now, false)?;
                self.with_db(|c| {
                    db::kv_set(c, "stats_summary", &summary.to_string());
                    db::kv_set(c, "stats_summary_at", &now.to_string());
                    db::kv_set(c, &format!("stats_heat_{year}"), &heat.to_string());
                    Ok(())
                })?;
                self.persist_offset();
                Ok(snap)
            }
            Err(HErr::Net(m)) => match read_cached(self, year)? {
                Some(cached) => Ok(cached),
                None => Err(into_core(HErr::Net(m))),
            },
            Err(e) => Err(into_core(e)),
        }
    }

    /// Yalnız önbellek; ağ yok. Özet veya o yılın ısı haritası önbellekte yoksa None.
    pub fn stats_cached(&self, year: i32) -> Result<Option<StatsSnapshot>, CoreError> {
        read_cached(self, year)
    }
}
