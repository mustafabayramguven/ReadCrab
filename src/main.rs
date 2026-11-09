use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{Arc, Mutex},
    time::Duration,
};

use readcrab::{
    app::{build_app, AppState},
    auth::{unix_now, LoginLimiter},
    config::Config,
    db::Db,
    scan::{start_scan, ScanStatus},
};

fn run_healthcheck(port: u16) -> ! {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(3)) else {
        std::process::exit(1);
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
    if stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        std::process::exit(1);
    }
    let mut buf = String::new();
    if stream.read_to_string(&mut buf).is_err() {
        std::process::exit(1);
    }
    let first_line = buf.lines().next().unwrap_or("");
    if first_line.contains(" 200 ") {
        std::process::exit(0);
    }
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    let cfg = Config::from_env().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });

    if let Some(arg) = std::env::args().nth(1) {
        if arg == "healthcheck" {
            run_healthcheck(cfg.port);
        }
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let covers_dir = cfg.data_dir.join("covers");
    if let Err(e) = std::fs::create_dir_all(&covers_dir) {
        eprintln!(
            "Veri dizini oluşturulamadı ({}): {e}. Dizinin uid 10001 kullanıcısı için yazılabilir olduğundan emin olun.",
            covers_dir.display()
        );
        std::process::exit(2);
    }

    readcrab::formats::cover::remove_stale_tmp(&covers_dir);

    let db_path = cfg.data_dir.join("readcrab.db");
    let db = Db::open(&db_path).unwrap_or_else(|e| {
        eprintln!(
            "Veritabanı açılamadı ({}): {e}. Dizinin uid 10001 kullanıcısı için yazılabilir olduğundan emin olun.",
            db_path.display()
        );
        std::process::exit(2);
    });

    let port = cfg.port;
    let state = AppState {
        db,
        cfg: Arc::new(cfg),
        scan: Arc::new(ScanStatus::new()),
        limiter: Arc::new(LoginLimiter::new()),
        mobi_cache: Arc::new(Mutex::new(None)),
        cover_lock: Arc::new(tokio::sync::Mutex::new(())),
    };

    let cleanup_db = state.db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let now = unix_now();
            let _ = cleanup_db
                .run(move |conn| {
                    conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", [now])?;
                    Ok(())
                })
                .await;
        }
    });

    std::sync::LazyLock::force(&readcrab::auth::DUMMY_HASH);
    start_scan(state.clone());

    let app = build_app(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("port dinlenemedi ({addr}): {e}");
            std::process::exit(1);
        });

    tracing::info!("ReadCrab dinliyor: {addr}");
    if let Err(e) = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    {
        tracing::error!("sunucu hatası: {e}");
        std::process::exit(1);
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
