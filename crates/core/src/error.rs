use std::fmt;

use crate::http::HErr;

#[derive(Debug, uniffi::Error)]
#[uniffi(flat_error)]
pub enum CoreError {
    Network(String),      // bağlantı kurulamadı, zaman aşımı
    Tls(String), // TLS el sıkışması/sertifika doğrulaması başarısız (özel CA, kendi imzalı, süresi dolmuş)
    Unauthorized, // 401 veya oturum yok
    Server(String), // diğer 4xx/5xx; metin sunucunun "message" alanı ya da "HTTP <kod>"
    Offline,     // dosya yerelde yok ve ağ gerekli (okuyucu çağrıları)
    NotFound,    // yerel kayıt yok ya da sunucu 404
    InvalidInput(String), // doğrulama
    Storage(String), // SQLite / dosya sistemi
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Network(m) => write!(f, "Sunucuya ulaşılamadı: {m}"),
            CoreError::Tls(m) => write!(
                f,
                "Güvenli bağlantı kurulamadı: sunucunun sertifikası doğrulanamadı. Özel veya kendi imzalı sertifikalar desteklenmez. ({m})"
            ),
            CoreError::Unauthorized => write!(f, "Oturum açmanız gerekiyor."),
            CoreError::Server(m) => write!(f, "{m}"),
            CoreError::Offline => write!(f, "Bağlantı yok ve içerik indirilmemiş."),
            CoreError::NotFound => write!(f, "Bulunamadı."),
            CoreError::InvalidInput(m) => write!(f, "{m}"),
            CoreError::Storage(m) => write!(f, "Depolama hatası: {m}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> Self {
        CoreError::Storage(e.to_string())
    }
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Storage(e.to_string())
    }
}

/// İç HTTP hatasını UniFFI'ye açılan hataya çevirir.
pub fn into_core(e: HErr) -> CoreError {
    match e {
        HErr::Net(m) => {
            let l = m.to_lowercase();
            let tls = [
                "certificate",
                "unknownissuer",
                "invalid peer",
                "handshake",
                "invalid dnsname",
                "tls",
            ]
            .iter()
            .any(|k| l.contains(k));
            if tls {
                CoreError::Tls(m)
            } else {
                CoreError::Network(m)
            }
        }
        HErr::Status { code: 401, .. } => CoreError::Unauthorized,
        HErr::Status { code: 404, .. } => CoreError::NotFound,
        HErr::Status { code, message, .. } => {
            if message.is_empty() {
                CoreError::Server(format!("HTTP {code}"))
            } else {
                CoreError::Server(message)
            }
        }
        HErr::Parse(_) => CoreError::Server("Sunucu yanıtı anlaşılamadı.".to_string()),
    }
}
