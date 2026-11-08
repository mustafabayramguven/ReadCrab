use std::{collections::HashMap, fs::File, io::Read, path::Path};

use percent_encoding::percent_decode_str;
use quick_xml::{events::Event, Reader};
use zip::ZipArchive;

use crate::formats::{
    FormatError, MAX_CHAPTER_BYTES, MAX_ENTRIES, MAX_TOTAL_BYTES, MAX_XML_BYTES,
};

#[derive(Clone, Debug)]
pub struct EpubInfo {
    pub title: Option<String>,
    pub spine: Vec<String>,
    pub toc: Vec<(String, usize)>,
    pub cover: Option<String>,
    pub manifest_images: HashMap<String, String>,
}

#[derive(Clone, Debug)]
struct ManifestItem {
    href: String,
    media_type: String,
    properties: String,
}

pub fn normalize_zip_path(base_dir: &str, href: &str) -> Option<String> {
    let clean = href
        .split('#')
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .trim();
    if clean.is_empty() || clean.starts_with('/') || clean.contains('\\') || clean.contains(':') {
        return None;
    }

    let decoded = percent_decode_str(clean).decode_utf8().ok()?;
    if decoded.is_empty()
        || decoded.starts_with('/')
        || decoded.contains('\\')
        || decoded.contains('\0')
        || decoded.chars().any(|c| c.is_control())
    {
        return None;
    }

    let mut stack: Vec<&str> = Vec::new();
    for seg in base_dir.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            stack.pop()?;
        } else {
            stack.push(seg);
        }
    }

    for seg in decoded.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            stack.pop()?;
        } else {
            stack.push(seg);
        }
    }

    if stack.is_empty() {
        return None;
    }
    Some(stack.join("/"))
}

pub fn parent_dir(zip_path: &str) -> &str {
    match zip_path.rfind('/') {
        Some(idx) => &zip_path[..idx],
        None => "",
    }
}

pub fn epub_open(path: &Path) -> Result<EpubInfo, FormatError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(|_| FormatError::Corrupt)?;
    if archive.len() > MAX_ENTRIES {
        return Err(FormatError::TooLarge);
    }

    let mut total_bytes: u64 = 0;
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
    }

    let enc_path = "META-INF/encryption.xml";
    if let Some(enc_xml) = read_zip_string_opt(&mut archive, enc_path, MAX_XML_BYTES)? {
        check_encryption_xml(&enc_xml)?;
    }

    let container_path = "META-INF/container.xml";
    let container_xml = read_zip_string(&mut archive, container_path, MAX_XML_BYTES)?;
    let opf_path = parse_container_xml(&container_xml)?;
    let opf_dir = parent_dir(&opf_path).to_string();

    let opf_xml = read_zip_string(&mut archive, &opf_path, MAX_XML_BYTES)?;
    let parsed = parse_opf_xml(&opf_xml, &opf_dir)?;

    if parsed.spine.is_empty() {
        return Err(FormatError::Corrupt);
    }

    let mut toc = Vec::new();
    if let Some(ref nav_path) = parsed.nav_href {
        if let Ok(Some(nav_xml)) = read_zip_string_opt(&mut archive, nav_path, MAX_XML_BYTES) {
            let nav_dir = parent_dir(nav_path);
            toc = parse_nav_toc(&nav_xml, nav_dir, &parsed.spine);
        }
    }
    if toc.is_empty() {
        if let Some(ref ncx_path) = parsed.ncx_href {
            if let Ok(Some(ncx_xml)) = read_zip_string_opt(&mut archive, ncx_path, MAX_XML_BYTES) {
                let ncx_dir = parent_dir(ncx_path);
                toc = parse_ncx_toc(&ncx_xml, ncx_dir, &parsed.spine);
            }
        }
    }
    if toc.is_empty() {
        toc = parsed
            .spine
            .iter()
            .enumerate()
            .map(|(i, _)| (format!("Bölüm {}", i + 1), i))
            .collect();
    }

    Ok(EpubInfo {
        title: parsed.title,
        spine: parsed.spine,
        toc,
        cover: parsed.cover,
        manifest_images: parsed.manifest_images,
    })
}

pub fn epub_chapter_html(path: &Path, info: &EpubInfo, n: usize) -> Result<String, FormatError> {
    let member = info.spine.get(n).ok_or(FormatError::Corrupt)?;
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(|_| FormatError::Corrupt)?;
    read_zip_string(&mut archive, member, MAX_CHAPTER_BYTES)
}

pub fn epub_asset(
    path: &Path,
    info: &EpubInfo,
    p: &str,
) -> Result<(Vec<u8>, String), FormatError> {
    if p.contains("..") {
        return Err(FormatError::Corrupt);
    }
    let norm = normalize_zip_path("", p).ok_or(FormatError::Corrupt)?;
    let mime = info
        .manifest_images
        .get(&norm)
        .ok_or(FormatError::Corrupt)?
        .clone();

    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(|_| FormatError::Corrupt)?;
    let bytes = read_zip_bytes(&mut archive, &norm, crate::formats::MAX_ENTRY_BYTES)?;
    Ok((bytes, mime))
}

fn read_zip_bytes(
    archive: &mut ZipArchive<File>,
    name: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, FormatError> {
    let entry = match archive.by_name(name) {
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
    if entry.size() > max_bytes {
        return Err(FormatError::TooLarge);
    }
    let mut buf = Vec::new();
    entry
        .take(max_bytes + 1)
        .read_to_end(&mut buf)
        .map_err(|_| FormatError::Corrupt)?;
    if buf.len() as u64 > max_bytes {
        return Err(FormatError::TooLarge);
    }
    Ok(buf)
}

fn read_zip_string(
    archive: &mut ZipArchive<File>,
    name: &str,
    max_bytes: u64,
) -> Result<String, FormatError> {
    let bytes = read_zip_bytes(archive, name, max_bytes)?;
    String::from_utf8(bytes).map_err(|_| FormatError::Corrupt)
}

fn read_zip_string_opt(
    archive: &mut ZipArchive<File>,
    name: &str,
    max_bytes: u64,
) -> Result<Option<String>, FormatError> {
    match archive.by_name(name) {
        Ok(_) => {}
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(zip::result::ZipError::UnsupportedArchive(_)) => return Err(FormatError::Drm),
        Err(_) => return Err(FormatError::Corrupt),
    }
    read_zip_string(archive, name, max_bytes).map(Some)
}

fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().rposition(|&b| b == b':') {
        Some(idx) => &raw[idx + 1..],
        None => raw,
    }
}

fn attr_value(e: &quick_xml::events::BytesStart<'_>, key_local: &[u8]) -> Option<String> {
    for attr in e.attributes().flatten() {
        if local_name(attr.key.as_ref()).eq_ignore_ascii_case(key_local) {
            if let Ok(val) = attr.decode_and_unescape_value_with(|_| Some("")) {
                return Some(val.into_owned());
            }
        }
    }
    None
}

fn check_encryption_xml(xml: &str) -> Result<(), FormatError> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                if local_name(e.name().as_ref()).eq_ignore_ascii_case(b"EncryptionMethod") {
                    let alg = attr_value(e, b"Algorithm").unwrap_or_default();
                    let allowed = alg == "http://www.idpf.org/2008/embedding"
                        || alg == "http://ns.adobe.com/pdf/enc#RC";
                    if !allowed {
                        return Err(FormatError::Drm);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(FormatError::Corrupt),
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn parse_container_xml(xml: &str) -> Result<String, FormatError> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                if local_name(e.name().as_ref()).eq_ignore_ascii_case(b"rootfile") {
                    if let Some(fp) = attr_value(e, b"full-path") {
                        if let Some(norm) = normalize_zip_path("", &fp) {
                            return Ok(norm);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(FormatError::Corrupt),
            _ => {}
        }
        buf.clear();
    }
    Err(FormatError::Corrupt)
}

struct ParsedOpf {
    title: Option<String>,
    spine: Vec<String>,
    cover: Option<String>,
    manifest_images: HashMap<String, String>,
    nav_href: Option<String>,
    ncx_href: Option<String>,
}

fn parse_opf_xml(xml: &str, opf_dir: &str) -> Result<ParsedOpf, FormatError> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut in_title = false;
    let mut title_buf = String::new();
    let mut title: Option<String> = None;
    let mut meta_cover_id: Option<String> = None;
    let mut ncx_id: Option<String> = None;
    let mut manifest: HashMap<String, ManifestItem> = HashMap::new();
    let mut spine_ids: Vec<String> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let lname = local_name(e.name().as_ref());
                if lname.eq_ignore_ascii_case(b"title") && title.is_none() {
                    in_title = true;
                    title_buf.clear();
                } else if lname.eq_ignore_ascii_case(b"meta") {
                    if attr_value(e, b"name").as_deref() == Some("cover") {
                        meta_cover_id = attr_value(e, b"content");
                    }
                } else if lname.eq_ignore_ascii_case(b"item") {
                    record_manifest_item(e, opf_dir, &mut manifest);
                } else if lname.eq_ignore_ascii_case(b"spine") {
                    ncx_id = attr_value(e, b"toc");
                } else if lname.eq_ignore_ascii_case(b"itemref") {
                    if let Some(idref) = attr_value(e, b"idref") {
                        spine_ids.push(idref);
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let lname = local_name(e.name().as_ref());
                if lname.eq_ignore_ascii_case(b"meta") {
                    if attr_value(e, b"name").as_deref() == Some("cover") {
                        meta_cover_id = attr_value(e, b"content");
                    }
                } else if lname.eq_ignore_ascii_case(b"item") {
                    record_manifest_item(e, opf_dir, &mut manifest);
                } else if lname.eq_ignore_ascii_case(b" itemref")
                    || lname.eq_ignore_ascii_case(b"itemref")
                {
                    if let Some(idref) = attr_value(e, b"idref") {
                        spine_ids.push(idref);
                    }
                }
            }
            Ok(Event::Text(ref e)) if in_title => {
                if let Ok(txt) = e.unescape_with(|_| Some("")) {
                    title_buf.push_str(&txt);
                }
            }
            Ok(Event::End(ref e)) => {
                if local_name(e.name().as_ref()).eq_ignore_ascii_case(b"title") && in_title {
                    in_title = false;
                    let trimmed = title_buf.trim();
                    if !trimmed.is_empty() {
                        title = Some(trimmed.to_string());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(FormatError::Corrupt),
            _ => {}
        }
        buf.clear();
    }

    let mut manifest_images = HashMap::new();
    let mut prop_cover = None;
    let mut nav_href = None;

    for item in manifest.values() {
        if item.media_type.starts_with("image/") {
            manifest_images.insert(item.href.clone(), item.media_type.clone());
        }
        for prop in item.properties.split_whitespace() {
            if prop == "cover-image" && item.media_type.starts_with("image/") {
                prop_cover = Some(item.href.clone());
            }
            if prop == "nav" {
                nav_href = Some(item.href.clone());
            }
        }
    }

    let cover = prop_cover.or_else(|| {
        meta_cover_id
            .and_then(|id| manifest.get(&id))
            .filter(|it| it.media_type.starts_with("image/"))
            .map(|it| it.href.clone())
    });

    let ncx_href = ncx_id
        .and_then(|id| manifest.get(&id))
        .map(|it| it.href.clone());

    let mut spine = Vec::new();
    for idref in spine_ids {
        if let Some(item) = manifest.get(&idref) {
            spine.push(item.href.clone());
        }
    }

    Ok(ParsedOpf {
        title,
        spine,
        cover,
        manifest_images,
        nav_href,
        ncx_href,
    })
}

fn record_manifest_item(
    e: &quick_xml::events::BytesStart<'_>,
    opf_dir: &str,
    manifest: &mut HashMap<String, ManifestItem>,
) {
    let Some(id) = attr_value(e, b"id") else {
        return;
    };
    let Some(href_raw) = attr_value(e, b"href") else {
        return;
    };
    let Some(href) = normalize_zip_path(opf_dir, &href_raw) else {
        return;
    };
    let media_type = attr_value(e, b"media-type")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let properties = attr_value(e, b"properties").unwrap_or_default();
    manifest.insert(
        id,
        ManifestItem {
            href,
            media_type,
            properties,
        },
    );
}

fn parse_nav_toc(xml: &str, nav_dir: &str, spine: &[String]) -> Vec<(String, usize)> {
    let mut spine_map = HashMap::new();
    for (i, s) in spine.iter().enumerate() {
        spine_map.insert(s.as_str(), i);
    }
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let mut current_href: Option<String> = None;
    let mut current_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if local_name(e.name().as_ref()).eq_ignore_ascii_case(b"a") {
                    current_href = attr_value(e, b"href");
                    current_text.clear();
                }
            }
            Ok(Event::Text(ref e)) if current_href.is_some() => {
                if let Ok(t) = e.unescape_with(|_| Some("")) {
                    current_text.push_str(&t);
                }
            }
            Ok(Event::End(ref e)) => {
                if local_name(e.name().as_ref()).eq_ignore_ascii_case(b"a") {
                    if let Some(href) = current_href.take() {
                        if let Some(norm) = normalize_zip_path(nav_dir, &href) {
                            if let Some(&idx) = spine_map.get(norm.as_str()) {
                                let label = current_text.trim().to_string();
                                if !label.is_empty() {
                                    out.push((label, idx));
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn parse_ncx_toc(xml: &str, ncx_dir: &str, spine: &[String]) -> Vec<(String, usize)> {
    let mut spine_map = HashMap::new();
    for (i, s) in spine.iter().enumerate() {
        spine_map.insert(s.as_str(), i);
    }
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let mut in_text = false;
    let mut last_label = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let lname = local_name(e.name().as_ref());
                if lname.eq_ignore_ascii_case(b"text") {
                    in_text = true;
                    last_label.clear();
                } else if lname.eq_ignore_ascii_case(b"content") {
                    if let Some(src) = attr_value(e, b"src") {
                        if let Some(norm) = normalize_zip_path(ncx_dir, &src) {
                            if let Some(&idx) = spine_map.get(norm.as_str()) {
                                let label = last_label.trim().to_string();
                                if !label.is_empty() {
                                    out.push((label, idx));
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                if local_name(e.name().as_ref()).eq_ignore_ascii_case(b"content") {
                    if let Some(src) = attr_value(e, b"src") {
                        if let Some(norm) = normalize_zip_path(ncx_dir, &src) {
                            if let Some(&idx) = spine_map.get(norm.as_str()) {
                                let label = last_label.trim().to_string();
                                if !label.is_empty() {
                                    out.push((label, idx));
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(ref e)) if in_text => {
                if let Ok(t) = e.unescape_with(|_| Some("")) {
                    last_label.push_str(&t);
                }
            }
            Ok(Event::End(ref e)) => {
                if local_name(e.name().as_ref()).eq_ignore_ascii_case(b"text") {
                    in_text = false;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}
