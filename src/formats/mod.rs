use std::path::Path;

use crate::error::AppError;

pub mod comic;
pub mod epub;
pub mod mobi;
pub mod natural;
pub mod sanitize;

pub const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 20_000;
pub const MAX_XML_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_CHAPTER_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 64_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Epub,
    Mobi,
    Cbz,
    Zip,
    Rar,
    Cbr,
}

impl Format {
    pub fn from_path(p: &Path) -> Option<Format> {
        let ext = p.extension()?.to_str()?;
        Self::from_ext(ext)
    }

    pub fn from_ext(ext: &str) -> Option<Format> {
        match ext.to_ascii_lowercase().as_str() {
            "epub" => Some(Format::Epub),
            "mobi" => Some(Format::Mobi),
            "cbz" => Some(Format::Cbz),
            "zip" => Some(Format::Zip),
            "rar" => Some(Format::Rar),
            "cbr" => Some(Format::Cbr),
            _ => None,
        }
    }

    pub fn is_comic(self) -> bool {
        matches!(self, Format::Cbz | Format::Zip | Format::Rar | Format::Cbr)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Format::Epub => "epub",
            Format::Mobi => "mobi",
            Format::Cbz => "cbz",
            Format::Zip => "zip",
            Format::Rar => "rar",
            Format::Cbr => "cbr",
        }
    }
}

#[derive(Debug)]
pub enum FormatError {
    Corrupt,
    Drm,
    TooLarge,
    Timeout,
    ToolMissing,
    Io(std::io::Error),
}

impl From<std::io::Error> for FormatError {
    fn from(err: std::io::Error) -> Self {
        FormatError::Io(err)
    }
}

impl From<FormatError> for AppError {
    fn from(err: FormatError) -> Self {
        match err {
            FormatError::Drm => AppError::Drm,
            FormatError::Corrupt | FormatError::TooLarge | FormatError::Timeout => {
                AppError::Corrupt
            }
            FormatError::ToolMissing | FormatError::Io(_) => AppError::Internal,
        }
    }
}

pub fn is_valid_page_name(name: &str) -> bool {
    if name.is_empty()
        || name.starts_with('/')
        || name.ends_with('/')
        || name.contains('\\')
        || name.starts_with("__MACOSX/")
        || name.chars().any(|c| c.is_control())
    {
        return false;
    }

    for comp in name.split('/') {
        if comp.is_empty() || comp.starts_with('.') || comp == ".." || comp == "__MACOSX" {
            return false;
        }
    }

    image_mime_from_name(name).is_some()
}

pub fn image_mime_from_name(name: &str) -> Option<&'static str> {
    let ext = name.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}
