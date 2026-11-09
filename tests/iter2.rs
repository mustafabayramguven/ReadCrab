mod common;

use axum::http::StatusCode;
use common::{
    call, create_user_cookie, login_as_admin, make_cbz, make_epub, make_epub_html, make_mobi, png,
    seed_books, test_app, user_id, EpubOpts,
};
use readcrab::{api::reader::SEARCH_SLOTS, scan::scan_once};

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
    let sql = format!("UPDATE books SET error='corrupt' WHERE id={};", ids[2]);
    exec(&dirs, sql).await;

    let get = |id: i64| {
        let app = app.clone();
        let admin = admin.clone();
        async move {
            let uri = format!("/api/books/{id}");
            call(&app, "GET", &uri, &admin, None).await.1
        }
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
        let uri = format!("/api/stats/heatmap{q}");
        let (st, _) = call(&app, "GET", &uri, &admin, None).await;
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

// Arama uçları paylaşılan 2 izinli semafor kullanır; 429 testi diğerlerini bekletir.
static SEARCH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn search_setup(dirs: &common::TempDirs, rel: &str) -> i64 {
    assert!(scan_once(&dirs.state).await);
    let rel = rel.to_string();
    dirs.db
        .run(move |c| {
            let sql = "SELECT id FROM books WHERE rel_path = ?1";
            Ok(c.query_row(sql, [rel], |r| r.get(0))?)
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn search_epub_turkish_fold() {
    let _g = SEARCH_LOCK.lock().await;
    let (app, dirs) = test_app().await;
    let admin = login_as_admin(&app).await;
    let books = dirs.books_dir.path();
    make_epub(books, "a.epub", EpubOpts::default());
    let html = "<html><head><title>Gizlibaşlık</title></head><body><p>şeker</p>\
        <p>IŞIK ve İŞİK</p></body></html>";
    make_epub_html(books, "b.epub", html);
    let long = format!("<html><body><p>{}</p></body></html>", "ab ".repeat(150));
    make_epub_html(books, "c.epub", &long);
    make_cbz(books, "d.cbz", &[("1.png", png(4, 4, [1, 2, 3]))]);
    make_mobi(books, "e.mobi");
    let a = search_setup(&dirs, "a.epub").await;
    let b = search_setup(&dirs, "b.epub").await;
    let c = search_setup(&dirs, "c.epub").await;
    let d = search_setup(&dirs, "d.cbz").await;
    let e = search_setup(&dirs, "e.mobi").await;
    let get = |id: i64, q: &str| {
        let (app, admin) = (app.clone(), admin.clone());
        let uri = format!("/api/books/{id}/search?q={q}");
        async move { call(&app, "GET", &uri, &admin, None).await }
    };

    // a.epub: "İkinci" iki kez (başlık ve paragraf), yalnız 2. bölümde
    let (st, v) = get(a, "IKINCI").await;
    assert_eq!(st, StatusCode::OK);
    let r = v["results"].as_array().unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0]["ch"], 1);
    assert_eq!(r[0]["p"], 0.0);
    assert_eq!(r[0]["hit"], "İkinci");
    assert_eq!(r[0]["before"], "");
    assert_eq!(r[1]["before"], "İkinci Bölüm ");
    assert_eq!(v["truncated"], false);

    // b.epub: <p> içi bulunur, <title> bulunmaz; ışık/IŞIK/İŞİK eşdeğer
    let (_, v) = get(b, "%C5%9Feker").await;
    assert_eq!(v["results"].as_array().unwrap().len(), 1);
    let (_, v) = get(b, "gizlib").await;
    assert!(v["results"].as_array().unwrap().is_empty());
    let (_, v) = get(b, "%C4%B1%C5%9F%C4%B1k").await;
    assert_eq!(v["results"].as_array().unwrap().len(), 2);
    assert_eq!(v["results"][1]["hit"], "İŞİK");

    // c.epub: 100 sınırı
    let (_, v) = get(c, "ab").await;
    assert_eq!(v["results"].as_array().unwrap().len(), 100);
    assert_eq!(v["truncated"], true);

    // MOBI bölümleri
    let (st, v) = get(e, "bolum").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["results"].as_array().unwrap().len(), 2);

    // kısa sorgu, görsel kitap, boş sorgu: 400
    for (id, q) in [(a, "i"), (a, ""), (a, "%20%20"), (d, "ab")] {
        let (st, _) = get(id, q).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{id}/{q}");
    }
    let (st, _) = get(a, &"x".repeat(65)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn search_concurrency_429() {
    let _g = SEARCH_LOCK.lock().await;
    let (app, dirs) = test_app().await;
    let admin = login_as_admin(&app).await;
    make_epub(dirs.books_dir.path(), "a.epub", EpubOpts::default());
    let a = search_setup(&dirs, "a.epub").await;
    let uri = format!("/api/books/{a}/search?q=bolum");
    let _p1 = SEARCH_SLOTS.try_acquire().unwrap();
    let _p2 = SEARCH_SLOTS.try_acquire().unwrap();
    let (st, _) = call(&app, "GET", &uri, &admin, None).await;
    assert_eq!(st, StatusCode::TOO_MANY_REQUESTS);
    drop(_p2);
    let (st, _) = call(&app, "GET", &uri, &admin, None).await;
    assert_eq!(st, StatusCode::OK);
}

#[tokio::test]
async fn search_huge_ampersand_chapter_stays_within_limit() {
    let _g = SEARCH_LOCK.lock().await;
    let (app, dirs) = test_app().await;
    let admin = login_as_admin(&app).await;
    let html = format!("<html><body><p>{}</p></body></html>", "&".repeat(7 << 20));
    make_epub_html(dirs.books_dir.path(), "buyuk.epub", &html);
    let id = search_setup(&dirs, "buyuk.epub").await;
    let uri = format!("/api/books/{id}/search?q=bolum");
    let start = std::time::Instant::now();
    let (st, v) = call(&app, "GET", &uri, &admin, None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(start.elapsed() < std::time::Duration::from_secs(30));
    assert!(v["results"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn search_collapses_whitespace_in_query() {
    let _g = SEARCH_LOCK.lock().await;
    let (app, dirs) = test_app().await;
    let admin = login_as_admin(&app).await;
    make_epub(dirs.books_dir.path(), "a.epub", EpubOpts::default());
    let a = search_setup(&dirs, "a.epub").await;
    let uri = format!("/api/books/{a}/search?q=ikinci%20%20%20b%C3%B6l%C3%BCm");
    let (st, v) = call(&app, "GET", &uri, &admin, None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(!v["results"].as_array().unwrap().is_empty());
}
