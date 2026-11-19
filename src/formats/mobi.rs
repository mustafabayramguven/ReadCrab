use std::path::Path;

use axum::body::Bytes;
use mobi::{headers::Encryption, Mobi};

use crate::formats::{FormatError, MAX_ENTRIES, MAX_ENTRY_BYTES};

/// Dosya bütünüyle belleğe okunur; görseller de bellekte tutulur.
const MAX_MOBI_FILE_BYTES: u64 = 256 * 1024 * 1024;
/// Açılmış metin (PalmDOC ~5 kat, HuffCDIC sınırsız açılabilir).
const MAX_MOBI_TEXT_BYTES: usize = MAX_ENTRY_BYTES as usize;

#[derive(Clone, Debug)]
pub struct MobiBook {
    pub title: String,
    pub chapters: Vec<String>,
    pub images: Vec<Bytes>,
}

pub fn mobi_open(path: &Path) -> Result<MobiBook, FormatError> {
    let size = std::fs::metadata(path)
        .map_err(|_| FormatError::Corrupt)?
        .len();
    if size > MAX_MOBI_FILE_BYTES {
        return Err(FormatError::TooLarge);
    }
    let m = Mobi::from_path(path).map_err(|_| FormatError::Corrupt)?;
    if m.encryption() != Encryption::No {
        return Err(FormatError::Drm);
    }
    // başlıktaki beyan açmadan önce; gerçek boyut açtıktan sonra denetlenir
    if m.metadata.palmdoc.text_length as usize > MAX_MOBI_TEXT_BYTES {
        return Err(FormatError::TooLarge);
    }

    let raw = m.content_as_string_lossy();
    if raw.len() > MAX_MOBI_TEXT_BYTES {
        return Err(FormatError::TooLarge);
    }
    let chapters = split_mobi_chapters(&raw);
    if chapters.is_empty() {
        return Err(FormatError::Corrupt);
    }
    if chapters.len() > MAX_ENTRIES {
        return Err(FormatError::TooLarge);
    }

    let records = m.image_records();
    if records.len() > MAX_ENTRIES {
        return Err(FormatError::TooLarge);
    }
    let images: Vec<Bytes> = records
        .into_iter()
        .map(|r| Bytes::from(r.content.to_vec()))
        .collect();

    let title = m.title().trim().trim_matches('\0').to_string();

    Ok(MobiBook {
        title,
        chapters,
        images,
    })
}

fn split_mobi_chapters(raw: &str) -> Vec<String> {
    let mut chapters = Vec::new();
    let mut rest = raw;
    let needle = b"<mbp:pagebreak";

    loop {
        let bytes = rest.as_bytes();
        let mut found = None;
        if bytes.len() >= needle.len() {
            for i in 0..=(bytes.len() - needle.len()) {
                if bytes[i..i + needle.len()].eq_ignore_ascii_case(needle) {
                    if let Some(end_rel) = rest[i..].find('>') {
                        found = Some((i, i + end_rel + 1));
                    }
                    break;
                }
            }
        }

        match found {
            Some((start, after)) => {
                let part = rest[..start].trim();
                if !part.is_empty() {
                    chapters.push(part.to_string());
                }
                rest = &rest[after..];
            }
            None => {
                let part = rest.trim();
                if !part.is_empty() {
                    chapters.push(part.to_string());
                }
                break;
            }
        }
    }

    chapters
}
