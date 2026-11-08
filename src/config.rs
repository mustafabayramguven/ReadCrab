use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub data_dir: PathBuf,
    pub books_dir: PathBuf,
    pub cookie_secure: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let port = match std::env::var("PORT") {
            Ok(val) => val
                .parse::<u16>()
                .ok()
                .filter(|&p| p > 0)
                .ok_or_else(|| format!("Geçersiz PORT değeri: {val}"))?,
            Err(_) => 3435,
        };
        let data_dir = std::env::var("DATA_DIR")
            .unwrap_or_else(|_| "/data".to_string())
            .into();
        let books_dir = std::env::var("BOOKS_DIR")
            .unwrap_or_else(|_| "/books".to_string())
            .into();
        let cookie_secure = std::env::var("COOKIE_SECURE")
            .map(|v| v == "1")
            .unwrap_or(false);

        Ok(Self {
            port,
            data_dir,
            books_dir,
            cookie_secure,
        })
    }
}
