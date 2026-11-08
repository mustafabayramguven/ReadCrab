use std::path::Path;

use mobi::{headers::Encryption, Mobi};

use crate::formats::FormatError;

#[derive(Clone, Debug)]
pub struct MobiBook {
    pub title: String,
    pub chapters: Vec<String>,
    pub images: Vec<Vec<u8>>,
}

pub fn mobi_open(path: &Path) -> Result<MobiBook, FormatError> {
    let m = Mobi::from_path(path).map_err(|_| FormatError::Corrupt)?;
    if m.encryption() != Encryption::No {
        return Err(FormatError::Drm);
    }

    let raw = m.content_as_string_lossy();
    let chapters = split_mobi_chapters(&raw);
    if chapters.is_empty() {
        return Err(FormatError::Corrupt);
    }

    let images: Vec<Vec<u8>> = m
        .image_records()
        .into_iter()
        .map(|r| r.content.to_vec())
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
                        break;
                    }
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
