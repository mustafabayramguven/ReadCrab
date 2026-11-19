//! ReadCrab terminal okuyucusu.
//!
//! Android uygulamasıyla aynı Rust çekirdeğini (`readcrab_core`) kullanır: kütüphane, indirme,
//! çevrimdışı okuma, ilerleme/yer imi eşitlemesi ve okuma istatistikleri aynı mantıkla çalışır.

mod app;
mod comic_reader;
mod html;
mod library;
mod login;
mod reader;
mod stats;
mod text_reader;
mod theme;
mod util;
mod widgets;
mod wrap;

use std::{path::PathBuf, process::ExitCode, time::Duration};

use ratatui_image::picker::Picker;
use readcrab_core::Core;

const USAGE: &str = "\
readcrab-tui — ReadCrab terminal okuyucusu

Kullanım: readcrab-tui [seçenekler]

Seçenekler:
  --data-dir DİZİN   Veri dizini (varsayılan: $XDG_DATA_HOME/readcrab-tui
                     ya da ~/.local/share/readcrab-tui)
  --halfblocks       Görselleri grafik protokolü yerine yarım blok karakterlerle çiz
  -h, --help         Bu yardımı göster
  -V, --version      Sürümü göster

Ortam değişkenleri:
  READCRAB_SERVER    Giriş formunda önerilecek sunucu adresi
  READCRAB_TUI_DIR   --data-dir ile aynı

Uygulama içinde ? tuşu o ekranın tuşlarını gösterir.";

struct Args {
    data_dir: Option<PathBuf>,
    halfblocks: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        data_dir: None,
        halfblocks: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("readcrab-tui {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--halfblocks" => a.halfblocks = true,
            "--data-dir" => {
                let v = it.next().ok_or("--data-dir bir dizin bekler")?;
                a.data_dir = Some(PathBuf::from(v));
            }
            s if s.starts_with("--data-dir=") => {
                a.data_dir = Some(PathBuf::from(&s["--data-dir=".len()..]));
            }
            other => return Err(format!("bilinmeyen seçenek: {other}\n\n{USAGE}")),
        }
    }
    Ok(a)
}

fn default_data_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("READCRAB_TUI_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("readcrab-tui"))
}

/// Oturum belirteci veritabanında durur: dizin yalnız kullanıcıya açık olmalı.
fn prepare_dir(dir: &PathBuf) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let Some(dir) = args.data_dir.or_else(default_data_dir) else {
        eprintln!("Veri dizini bulunamadı; --data-dir ile verin.");
        return ExitCode::FAILURE;
    };
    if let Err(e) = prepare_dir(&dir) {
        eprintln!("Veri dizini hazırlanamadı ({}): {e}", dir.display());
        return ExitCode::FAILURE;
    }
    let core = match Core::new(dir.to_string_lossy().into_owned()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Yerel veritabanı açılamadı: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut terminal = match ratatui::try_init() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Terminal başlatılamadı: {e}");
            return ExitCode::FAILURE;
        }
    };
    let picker = if args.halfblocks {
        Picker::halfblocks()
    } else {
        Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks())
    };
    let mut app = app::App::new(core, dir, picker);
    let result = app.run(&mut terminal);
    ratatui::restore();

    // bekleyen ilerleme/okuma süresi; ağ yoksa bir sonraki açılışta gönderilir
    if let Some(Err(e)) = app::push_with_timeout(&app.ctx().core, Duration::from_secs(3)) {
        eprintln!("Eşitleme yapılamadı, değişiklikler bir sonraki açılışta gönderilecek: {e}");
    }
    if let Err(e) = result {
        eprintln!("Hata: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
