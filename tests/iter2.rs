mod common;

use axum::http::StatusCode;
use common::{call, create_user_cookie, login_as_admin, seed_books, test_app, user_id};

async fn exec(dirs: &common::TempDirs, sql: String) {
    dirs.db
        .run(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .unwrap();
}

#[tokio::test]
async fn continue_lists_reading_only() {
    let (app, dirs) = test_app().await;
    let admin = login_as_admin(&app).await;
    let other = create_user_cookie(&app, &admin, "diger").await;
    let other_id = user_id(&dirs, "diger").await;
    let ids = seed_books(&dirs, "S", 25).await;
    let sql: String = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            // ilk 22 reading (üst sınır 20 sınanır), sonrası finished/unread
            let st = if i < 22 { "reading" } else { "finished" };
            format!("INSERT INTO progress(user_id,book_id,percent,status,updated_at) VALUES(1,{id},0.5,'{st}',{});", 1000 + i)
        })
        .collect();
    let extra = format!(
        "INSERT INTO progress(user_id,book_id,percent,status,updated_at) VALUES({other_id},{},0.1,'reading',5000);
         UPDATE books SET missing=1 WHERE id={};
         UPDATE books SET error='corrupt' WHERE id={};",
        ids[24], ids[21], ids[20]
    );
    exec(&dirs, format!("{sql}{extra}")).await;

    let (st, v) = call(&app, "GET", "/api/continue", &admin, None).await;
    assert_eq!(st, StatusCode::OK);
    let items = v["items"].as_array().unwrap();
    // 22 reading - 1 missing - 1 error = 20, updated_at azalan
    assert_eq!(items.len(), 20);
    assert_eq!(items[0]["book_id"], ids[19]);
    assert_eq!(items[19]["book_id"], ids[0]);
    assert_eq!(items[0]["percent"], 0.5);
    assert_eq!(items[0]["series_name"], "S");
    assert_eq!(items[0]["cover_v"], 0);

    let (_, v) = call(&app, "GET", "/api/continue", &other, None).await;
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["book_id"], ids[24]);
}

#[tokio::test]
async fn book_detail_next_prev_and_pace() {
    let (app, dirs) = test_app().await;
    let admin = login_as_admin(&app).await;
    let ids = seed_books(&dirs, "S", 4).await;
    // 3. cilt hatalı: atlanmalı
    exec(&dirs, format!("UPDATE books SET error='corrupt' WHERE id={};", ids[2])).await;

    let get = |id: i64| {
        let app = app.clone();
        let admin = admin.clone();
        async move { call(&app, "GET", &format!("/api/books/{id}"), &admin, None).await.1 }
    };
    let first = get(ids[0]).await;
    assert_eq!(first["next_book_id"], ids[1]);
    assert_eq!(first["next_book_title"], "Cilt 2");
    assert!(first["prev_book_id"].is_null());
    assert_eq!(first["cover_version"], 0);
    let second = get(ids[1]).await;
    assert_eq!(second["next_book_id"], ids[3]);
    assert_eq!(second["prev_book_id"], ids[0]);
    let last = get(ids[3]).await;
    assert!(last["next_book_id"].is_null());
    assert!(last["next_book_title"].is_null());
    assert_eq!(last["prev_book_id"], ids[1]);

    // yetersiz oturum: pace null
    let b = ids[0];
    let ins = |n: u32, from: f64, to: f64| {
        format!(
            "INSERT INTO reading_sessions(user_id,book_id,client_id,started_at,ended_at,active_seconds,start_percent,end_percent,local_date)
             VALUES(1,{b},'{n:032}',1,2,120,{from},{to},'2026-01-01');"
        )
    };
    exec(&dirs, ins(1, 0.0, 0.02)).await;
    assert!(get(b).await["pace_seconds"].is_null());

    // 3 x 120 sn, toplam %6 ilerleme -> 360 / 0.06 = 6000
    exec(&dirs, [ins(2, 0.02, 0.04), ins(3, 0.04, 0.06)].concat()).await;
    assert_eq!(get(b).await["pace_seconds"], 6000);
}

#[tokio::test]
async fn heatmap_isolated_per_user() {
    let (app, dirs) = test_app().await;
    let admin = login_as_admin(&app).await;
    let other = create_user_cookie(&app, &admin, "diger").await;
    let other_id = user_id(&dirs, "diger").await;
    let b = seed_books(&dirs, "S", 1).await[0];
    let ins = |uid: i64, n: u32, secs: i64, date: &str| {
        format!(
            "INSERT INTO reading_sessions(user_id,book_id,client_id,started_at,ended_at,active_seconds,pages_read,start_percent,end_percent,local_date)
             VALUES({uid},{b},'{n:032}',1,2,{secs},3,0,0,'{date}');"
        )
    };
    let sql = [
        ins(1, 1, 100, "2025-12-31"),
        ins(1, 2, 60, "2026-01-01"),
        ins(1, 3, 40, "2026-01-01"),
        ins(1, 4, 50, "2026-12-31"),
        ins(1, 5, 70, "2027-01-01"),
        ins(other_id, 6, 999, "2026-06-06"),
    ]
    .concat();
    exec(&dirs, sql).await;

    let (st, v) = call(&app, "GET", "/api/stats/heatmap?year=2026", &admin, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["year"], 2026);
    assert_eq!(v["years"], serde_json::json!([2025, 2026, 2027]));
    let days = v["days"].as_array().unwrap();
    assert_eq!(days.len(), 2);
    assert_eq!(days[0]["date"], "2026-01-01");
    assert_eq!(days[0]["seconds"], 100);
    assert_eq!(days[0]["pages"], 6);
    assert_eq!(days[1]["date"], "2026-12-31");

    let (_, v) = call(&app, "GET", "/api/stats/heatmap?year=2026", &other, None).await;
    assert_eq!(v["days"].as_array().unwrap().len(), 1);
    assert_eq!(v["years"], serde_json::json!([2026]));
    let (_, v) = call(&app, "GET", "/api/stats/heatmap?year=2030", &other, None).await;
    assert_eq!(v["years"], serde_json::json!([2026]));
    assert!(v["days"].as_array().unwrap().is_empty());

    for q in ["?year=1999", "?year=2101", "?year=abc", ""] {
        let (st, _) = call(&app, "GET", &format!("/api/stats/heatmap{q}"), &admin, None).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{q}");
    }
}

#[tokio::test]
async fn history_date_filter() {
    let (app, dirs) = test_app().await;
    let admin = login_as_admin(&app).await;
    let other = create_user_cookie(&app, &admin, "diger").await;
    let other_id = user_id(&dirs, "diger").await;
    let b = seed_books(&dirs, "S", 1).await[0];
    let ins = |uid: i64, n: u32, date: &str| {
        format!(
            "INSERT INTO reading_sessions(user_id,book_id,client_id,started_at,ended_at,active_seconds,start_percent,end_percent,local_date)
             VALUES({uid},{b},'{n:032}',{n},{n},10,0,0,'{date}');"
        )
    };
    let sql = [
        ins(1, 1, "2026-03-01"),
        ins(1, 2, "2026-03-02"),
        ins(1, 3, "2026-03-02"),
        ins(other_id, 4, "2026-03-03"),
    ]
    .concat();
    exec(&dirs, sql).await;

    let (st, v) = call(&app, "GET", "/api/history?date=2026-03-02", &admin, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["total"], 2);
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
    let (_, v) = call(&app, "GET", "/api/history", &admin, None).await;
    assert_eq!(v["total"], 3);
    let (st, _) = call(&app, "GET", "/api/history?date=2026-13-40", &admin, None).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (_, v) = call(&app, "GET", "/api/history?date=2026-03-03", &admin, None).await;
    assert_eq!(v["total"], 0);
    let (_, v) = call(&app, "GET", "/api/history?date=2026-03-03", &other, None).await;
    assert_eq!(v["total"], 1);
}
