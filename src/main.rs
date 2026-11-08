use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::Arc,
    time::Duration,
};

use kavita_rs::{
    app::{AppState, build_app},
    config::Config,
    db::Db,
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

    if let Err(e) = std::fs::create_dir_all(cfg.data_dir.join("covers")) {
        tracing::error!("veri dizini oluşturulamadı: {e}");
        std::process::exit(1);
    }

    let db_path = cfg.data_dir.join("kavita.db");
    let db = Db::open(&db_path).unwrap_or_else(|e| {
        tracing::error!("veritabanı açılamadı: {e}");
        std::process::exit(1);
    });

    let port = cfg.port;
    let state = AppState {
        db,
        cfg: Arc::new(cfg),
    };
    let app = build_app(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
        tracing::error!("port dinlenemedi ({addr}): {e}");
        std::process::exit(1);
    });

    tracing::info!("kavita-rs dinliyor: {addr}");
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
