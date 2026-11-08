use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use tokio::{io::AsyncReadExt, process::Command, sync::Semaphore};
use zip::ZipArchive;

use crate::formats::{
    is_valid_page_name, natural::natural_cmp, Format, FormatError, MAX_ENTRIES, MAX_ENTRY_BYTES,
    MAX_TOTAL_BYTES,
};

const BSDTAR_TIMEOUT: Duration = Duration::from_secs(30);

static BSDTAR_SEM: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(2));

type PageListCache = Mutex<Option<(PathBuf, i64, Arc<Vec<String>>)>>;
static PAGE_LIST_CACHE: LazyLock<PageListCache> = LazyLock::new(|| Mutex::new(None));

pub fn glob_escape(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if matches!(c, '\\' | '*' | '?' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub async fn bsdtar_run(
    prog: &str,
    args: &[&str],
    timeout_dur: Duration,
    max_output: u64,
) -> Result<Vec<u8>, FormatError> {
    let _permit = BSDTAR_SEM
        .acquire()
        .await
        .map_err(|_| FormatError::Corrupt)?;

    let mut child = Command::new(prog)
        .args(args)
        .env_clear()
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FormatError::ToolMissing
            } else {
                FormatError::Io(e)
            }
        })?;

    let mut stdout = child.stdout.take().ok_or(FormatError::Corrupt)?;

    let res = tokio::time::timeout(timeout_dur, async {
        let mut buf = Vec::new();
        let mut limited = (&mut stdout).take(max_output + 1);
        limited.read_to_end(&mut buf).await?;
        if buf.len() as u64 > max_output {
            let _ = child.kill().await;
            return Err(FormatError::TooLarge);
        }
        let status = child.wait().await?;
        if !status.success() {
            return Err(FormatError::Corrupt);
        }
        Ok(buf)
    })
    .await;

    match res {
        Ok(inner) => inner,
        Err(_) => {
            let _ = child.kill().await;
            Err(FormatError::Timeout)
        }
    }
}

pub async fn bsdtar_list(path: &Path) -> Result<Vec<String>, FormatError> {
    let path_str = path.to_str().ok_or(FormatError::Corrupt)?;
    let raw = bsdtar_run(
        "bsdtar",
        &["-tf", path_str],
        BSDTAR_TIMEOUT,
        MAX_ENTRY_BYTES,
    )
    .await?;
    let text = String::from_utf8(raw).map_err(|_| FormatError::Corrupt)?;

    let mut items = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty()
            || line.starts_with('/')
            || line.ends_with('/')
            || line.contains('\\')
            || line.chars().any(|c| c.is_control())
            || line.split('/').any(|c| c.is_empty() || c == "..")
        {
            continue;
        }
        items.push(line.to_string());
        if items.len() > MAX_ENTRIES {
            return Err(FormatError::TooLarge);
        }
    }
    Ok(items)
}

pub async fn bsdtar_read_limit(
    path: &Path,
    member: &str,
    max_entry: u64,
) -> Result<Vec<u8>, FormatError> {
    let path_str = path.to_str().ok_or(FormatError::Corrupt)?;
    let escaped = glob_escape(member);
    bsdtar_run(
        "bsdtar",
        &["-xOf", path_str, "--", &escaped],
        BSDTAR_TIMEOUT,
        max_entry,
    )
    .await
}

pub async fn bsdtar_read(path: &Path, member: &str) -> Result<Vec<u8>, FormatError> {
    bsdtar_read_limit(path, member, MAX_ENTRY_BYTES).await
}

pub async fn comic_pages(path: &Path, fmt: Format) -> Result<Vec<String>, FormatError> {
    match fmt {
        Format::Cbz | Format::Zip => {
            let p = path.to_path_buf();
            tokio::task::spawn_blocking(move || zip_list_pages(&p))
                .await
                .map_err(|_| FormatError::Corrupt)?
        }
        Format::Rar | Format::Cbr => {
            let members = bsdtar_list(path).await?;
            let mut pages: Vec<String> = members
                .into_iter()
                .filter(|m| is_valid_page_name(m))
                .collect();
            pages.sort_by(|a, b| natural_cmp(a, b));
            Ok(pages)
        }
        _ => Err(FormatError::Corrupt),
    }
}

pub async fn comic_pages_cached(
    path: &Path,
    fmt: Format,
    mtime: i64,
) -> Result<Arc<Vec<String>>, FormatError> {
    if let Some((cached_path, cached_mtime, pages)) = PAGE_LIST_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
    {
        if cached_path == path && *cached_mtime == mtime {
            return Ok(pages.clone());
        }
    }
    let pages = Arc::new(comic_pages(path, fmt).await?);
    *PAGE_LIST_CACHE.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((path.to_path_buf(), mtime, pages.clone()));
    Ok(pages)
}

pub async fn comic_read(
    path: &Path,
    fmt: Format,
    member: &str,
    max_entry: u64,
) -> Result<Vec<u8>, FormatError> {
    // shortcut: tek arşivlik sayfa listesi önbelleği ve 2 bsdtar izni; her sayfa okumasında arşiv yeniden açılır, büyük solid RAR'da yavaşlarsa açılmış sayfa önbelleği ekle
    match fmt {
        Format::Cbz | Format::Zip => {
            let p = path.to_path_buf();
            let m = member.to_string();
            tokio::task::spawn_blocking(move || zip_read_member(&p, &m, max_entry))
                .await
                .map_err(|_| FormatError::Corrupt)?
        }
        Format::Rar | Format::Cbr => bsdtar_read_limit(path, member, max_entry).await,
        _ => Err(FormatError::Corrupt),
    }
}

fn zip_list_pages(path: &Path) -> Result<Vec<String>, FormatError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(|_| FormatError::Corrupt)?;
    if archive.len() > MAX_ENTRIES {
        return Err(FormatError::TooLarge);
    }

    let mut total_bytes: u64 = 0;
    let mut pages = Vec::new();

    for i in 0..archive.len() {
        let entry = match archive.by_index_raw(i) {
            Ok(e) => e,
            Err(zip::result::ZipError::UnsupportedArchive(_)) => return Err(FormatError::Drm),
            Err(_) => return Err(FormatError::Corrupt),
        };
        if entry.encrypted() {
            return Err(FormatError::Drm);
        }
        total_bytes = total_bytes.saturating_add(entry.size());
        if total_bytes > MAX_TOTAL_BYTES {
            return Err(FormatError::TooLarge);
        }
        if entry.is_dir() || entry.is_symlink() {
            continue;
        }
        if entry.enclosed_name().is_none() {
            continue;
        }
        let name = entry.name();
        if is_valid_page_name(name) {
            pages.push(name.to_string());
        }
    }

    pages.sort_by(|a, b| natural_cmp(a, b));
    Ok(pages)
}

fn zip_read_member(path: &Path, member: &str, max_entry: u64) -> Result<Vec<u8>, FormatError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(|_| FormatError::Corrupt)?;
    let entry = match archive.by_name(member) {
        Ok(e) => e,
        Err(zip::result::ZipError::UnsupportedArchive(_)) => return Err(FormatError::Drm),
        Err(_) => return Err(FormatError::Corrupt),
    };
    if entry.encrypted() {
        return Err(FormatError::Drm);
    }
    if entry.enclosed_name().is_none() {
        return Err(FormatError::Corrupt);
    }
    if entry.size() > max_entry {
        return Err(FormatError::TooLarge);
    }

    let mut buf = Vec::new();
    entry
        .take(max_entry + 1)
        .read_to_end(&mut buf)
        .map_err(|_| FormatError::Corrupt)?;
    if buf.len() as u64 > max_entry {
        return Err(FormatError::TooLarge);
    }
    Ok(buf)
}
