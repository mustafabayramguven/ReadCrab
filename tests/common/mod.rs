#![allow(dead_code)]

use std::{
    fs::File,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use image::{ImageFormat, Rgb, RgbImage};
use kavita_rs::{
    app::{build_app, AppState},
    auth::LoginLimiter,
    config::Config,
    db::Db,
};
use tempfile::TempDir;
use tower::ServiceExt;
use zip::{write::SimpleFileOptions, ZipWriter};

pub struct TempDirs {
    pub data_dir: TempDir,
    pub books_dir: TempDir,
    pub db: Db,
    pub state: AppState,
}

#[derive(Clone, Debug, Default)]
pub struct EpubOpts {
    pub title: Option<String>,
    pub svg_cover: bool,
}

pub fn png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let img = RgbImage::from_pixel(w, h, Rgb(rgb));
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .unwrap();
    buf
}

pub fn make_cbz(dir: &Path, name: &str, pages: &[(&str, Vec<u8>)]) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let file = File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default();
    for (entry_name, data) in pages {
        zip.start_file(*entry_name, opts).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
    path
}

pub fn make_tar_as_cbr(dir: &Path, name: &str, files: &[(&str, Vec<u8>)]) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let file = File::create(&path).unwrap();
    let mut builder = tar::Builder::new(file);
    for (entry_name, data) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, *entry_name, &data[..])
            .unwrap();
    }
    builder.finish().unwrap();
    path
}

pub fn make_epub(dir: &Path, name: &str, opts: EpubOpts) -> PathBuf {
    let title = opts.title.unwrap_or_else(|| "Test EPUB Kitabı".to_string());
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>{title}</dc:title>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch2" href="ch2.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch3" href="ch3.xhtml" media-type="application/xhtml+xml"/>
    <item id="img1" href="images/pic.png" media-type="image/png"/>
    <item id="cover" href="images/cover.png" media-type="image/png" properties="cover-image"/>
    <item id="secret" href="secret.txt" media-type="text/plain"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
    <itemref idref="ch2"/>
    <itemref idref="ch3"/>
  </spine>
</package>"#
    );

    let nav = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<body>
  <nav epub:type="toc">
    <ol>
      <li><a href="ch1.xhtml">Bölüm Bir</a></li>
      <li><a href="ch2.xhtml">Bölüm İki</a></li>
      <li><a href="ch3.xhtml">Bölüm Üç</a></li>
    </ol>
  </nav>
</body>
</html>"#;

    let ch1 = if opts.svg_cover {
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><image xlink:href="images/cover.png" width="100" height="100"/></svg><p>Birinci bölüm metni</p></body></html>"#
    } else {
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><h1>Birinci Bölüm</h1><p>Birinci bölüm metni</p><img src="images/pic.png" alt="pic"/></body></html>"#
    };
    let ch2 = r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><h1>İkinci Bölüm</h1><p>İkinci bölüm metni</p></body></html>"#;
    let ch3 = r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><h1>Üçüncü Bölüm</h1><p>Üçüncü bölüm metni</p></body></html>"#;

    let pic_png = png(12, 12, [20, 40, 60]);
    let cover_png = png(60, 90, [100, 150, 200]);
    let unlisted_png = png(4, 4, [1, 2, 3]);

    make_cbz(
        dir,
        name,
        &[
            ("META-INF/container.xml", container.to_vec()),
            ("OEBPS/content.opf", opf.into_bytes()),
            ("OEBPS/nav.xhtml", nav.as_bytes().to_vec()),
            ("OEBPS/ch1.xhtml", ch1.as_bytes().to_vec()),
            ("OEBPS/ch2.xhtml", ch2.as_bytes().to_vec()),
            ("OEBPS/ch3.xhtml", ch3.as_bytes().to_vec()),
            ("OEBPS/images/pic.png", pic_png),
            ("OEBPS/images/cover.png", cover_png),
            ("OEBPS/images/unlisted.png", unlisted_png),
            ("OEBPS/secret.txt", b"secret content".to_vec()),
        ],
    )
}

pub fn make_epub_malicious(dir: &Path) -> PathBuf {
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

    let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE lolz [
  <!ENTITY lol "lol">
  <!ENTITY lol1 "&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;">
  <!ENTITY lol2 "&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;&lol1;">
]>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Zararli &lol2;</dc:title>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;

    let ch1 = r#"<html xmlns="http://www.w3.org/1999/xhtml">
<body>
  <script>alert('xss')</script>
  <img src="images/pic.png" onerror="alert(1)"/>
  <img src="../../etc/passwd"/>
  <img src="https://evil.example/tracker.png"/>
  <a href="javascript:alert(2)" target="_blank">kötü bağlantı</a>
  <a href="ch2.xhtml">iç bağlantı</a>
  <a href="https://example.com/safe">dış bağlantı</a>
  <svg><script>alert(3)</script></svg>
  <p style="color:red">Temiz içerik</p>
</body>
</html>"#;

    make_cbz(
        dir,
        "malicious.epub",
        &[
            ("META-INF/container.xml", container.to_vec()),
            ("OEBPS/content.opf", opf.to_vec()),
            ("OEBPS/ch1.xhtml", ch1.as_bytes().to_vec()),
        ],
    )
}

pub fn make_epub_drm(dir: &Path) -> PathBuf {
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

    let enc = br#"<?xml version="1.0" encoding="UTF-8"?>
<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <EncryptedData xmlns="http://www.w3.org/2001/04/xmlenc#">
    <EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#aes128-cbc"/>
  </EncryptedData>
</encryption>"#;

    let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>DRM Kitap</dc:title>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;

    make_cbz(
        dir,
        "drm.epub",
        &[
            ("META-INF/container.xml", container.to_vec()),
            ("META-INF/encryption.xml", enc.to_vec()),
            ("OEBPS/content.opf", opf.to_vec()),
            ("OEBPS/ch1.xhtml", b"<p>drm</p>".to_vec()),
        ],
    )
}

pub fn make_mobi_with_encryption(dir: &Path, name: &str, encryption: u16) -> PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    let title_bytes = b"Test MOBI Kitabi";
    let text_bytes =
        b"<h1>Bolum 1</h1><p>Ilk metin</p><mbp:pagebreak/><h1>Bolum 2</h1><img recindex=\"00001\"/>";
    let img_bytes = png(8, 8, [12, 34, 56]);

    let num_records: u16 = 3;
    let rec0_offset: u32 = 80 + 8 * (num_records as u32);
    let rec0_len: u32 = 16 + 232 + (title_bytes.len() as u32);
    let rec1_offset: u32 = rec0_offset + rec0_len;
    let rec2_offset: u32 = rec1_offset + (text_bytes.len() as u32);

    let mut out = Vec::new();

    // 1. PalmDB Header (78 bytes)
    let mut pdb_name = [0u8; 32];
    pdb_name[..16].copy_from_slice(title_bytes);
    out.extend_from_slice(&pdb_name);
    out.extend_from_slice(&0u16.to_be_bytes()); // attributes
    out.extend_from_slice(&0u16.to_be_bytes()); // version
    out.extend_from_slice(&1000u32.to_be_bytes()); // created
    out.extend_from_slice(&1000u32.to_be_bytes()); // modified
    out.extend_from_slice(&0u32.to_be_bytes()); // backup
    out.extend_from_slice(&0u32.to_be_bytes()); // modnum
    out.extend_from_slice(&0u32.to_be_bytes()); // app_info_id
    out.extend_from_slice(&0u32.to_be_bytes()); // sort_info_id
    out.extend_from_slice(b"BOOK"); // type_
    out.extend_from_slice(b"MOBI"); // creator
    out.extend_from_slice(&0u32.to_be_bytes()); // unique_id_seed
    out.extend_from_slice(&0u32.to_be_bytes()); // next_record_list_id
    out.extend_from_slice(&num_records.to_be_bytes());

    // 2. PdbRecords (3 * 8 + 2 = 26 bytes)
    out.extend_from_slice(&rec0_offset.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&rec1_offset.to_be_bytes());
    out.extend_from_slice(&2u32.to_be_bytes());
    out.extend_from_slice(&rec2_offset.to_be_bytes());
    out.extend_from_slice(&4u32.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // extra_bytes

    // 3. Record 0: PalmDocHeader (16 bytes)
    out.extend_from_slice(&1u16.to_be_bytes()); // compression = 1 (No)
    out.extend_from_slice(&0u16.to_be_bytes()); // unused0
    out.extend_from_slice(&(text_bytes.len() as u32).to_be_bytes()); // text_length
    out.extend_from_slice(&1u16.to_be_bytes()); // record_count = 1
    out.extend_from_slice(&4096u16.to_be_bytes()); // record_size = 4096
    out.extend_from_slice(&encryption.to_be_bytes()); // encryption
    out.extend_from_slice(&0u16.to_be_bytes()); // unused1

    // 4. Record 0: MobiHeader (232 bytes)
    out.extend_from_slice(b"MOBI"); // identifier
    out.extend_from_slice(&232u32.to_be_bytes()); // header_length
    out.extend_from_slice(&2u32.to_be_bytes()); // mobi_type = 2
    out.extend_from_slice(&65001u32.to_be_bytes()); // text_encoding = UTF8
    out.extend_from_slice(&1u32.to_be_bytes()); // id
    out.extend_from_slice(&6u32.to_be_bytes()); // gen_version
    out.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // ortho_index
    out.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // inflect_index
    out.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // index_names
    out.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // index_keys
    for _ in 0..6 {
        out.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // extra_indices
    }
    out.extend_from_slice(&3u32.to_be_bytes()); // first_non_book_index = 3
    out.extend_from_slice(&248u32.to_be_bytes()); // name_offset = 16 + 232
    out.extend_from_slice(&(title_bytes.len() as u32).to_be_bytes()); // name_length
    out.extend_from_slice(&0u16.to_be_bytes()); // unused
    out.extend_from_slice(&0u8.to_be_bytes()); // locale
    out.extend_from_slice(&0u8.to_be_bytes()); // language_code
    out.extend_from_slice(&0u32.to_be_bytes()); // input_language
    out.extend_from_slice(&0u32.to_be_bytes()); // output_language
    out.extend_from_slice(&6u32.to_be_bytes()); // format_version
    out.extend_from_slice(&2u32.to_be_bytes()); // first_image_index = 2
    out.extend_from_slice(&0u32.to_be_bytes()); // first_huff_record
    out.extend_from_slice(&0u32.to_be_bytes()); // huff_record_count
    out.extend_from_slice(&0u32.to_be_bytes()); // huff_table_offset
    out.extend_from_slice(&0u32.to_be_bytes()); // huff_table_length
    out.extend_from_slice(&0u32.to_be_bytes()); // exth_flags = 0
    out.extend_from_slice(&[0u8; 32]); // unused_0
    out.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // unused_1
    out.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // drm_offset
    out.extend_from_slice(&0u32.to_be_bytes()); // drm_count
    out.extend_from_slice(&0u32.to_be_bytes()); // drm_size
    out.extend_from_slice(&0u32.to_be_bytes()); // drm_flags
    out.extend_from_slice(&[0u8; 8]); // unused_2
    out.extend_from_slice(&1u16.to_be_bytes()); // first_content_record = 1
    out.extend_from_slice(&1u16.to_be_bytes()); // last_content_record = 1
    out.extend_from_slice(&1u32.to_be_bytes()); // unused_3
    out.extend_from_slice(&0u32.to_be_bytes()); // fcis_record
    out.extend_from_slice(&1u32.to_be_bytes()); // unused_4
    out.extend_from_slice(&0u32.to_be_bytes()); // flis_record
    out.extend_from_slice(&1u32.to_be_bytes()); // unused_5
    out.extend_from_slice(&0u64.to_be_bytes()); // unused_6
    out.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // unused_7
    out.extend_from_slice(&0u32.to_be_bytes()); // first_compilation_data_section_count
    out.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // data_section_count
    out.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // unused_8
    out.extend_from_slice(&0u32.to_be_bytes()); // extra_record_data_flags
    out.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // first_index_record

    // 5. Record 0: full book name
    out.extend_from_slice(title_bytes);

    // 6. Record 1: text
    out.extend_from_slice(text_bytes);

    // 7. Record 2: image
    out.extend_from_slice(&img_bytes);

    std::fs::write(&path, out).unwrap();
    path
}

pub fn make_mobi(dir: &Path, name: &str) -> PathBuf {
    make_mobi_with_encryption(dir, name, 0)
}

pub async fn test_app_with_secure(cookie_secure: bool) -> (Router, TempDirs) {
    let data_dir = tempfile::tempdir().unwrap();
    let books_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(data_dir.path().join("covers")).unwrap();

    let db = Db::open(&data_dir.path().join("kavita.db")).unwrap();
    let cfg = Config {
        port: 3435,
        data_dir: data_dir.path().to_path_buf(),
        books_dir: books_dir.path().to_path_buf(),
        cookie_secure,
    };
    let state = AppState {
        db: db.clone(),
        cfg: Arc::new(cfg),
        limiter: Arc::new(LoginLimiter::new()),
    };
    let app = build_app(state.clone());
    (
        app,
        TempDirs {
            data_dir,
            books_dir,
            db,
            state,
        },
    )
}

pub async fn test_app() -> (Router, TempDirs) {
    test_app_with_secure(false).await
}

pub async fn login_as_admin(app: &Router) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/setup")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"username":"admin","password":"adminpassword123"}"#,
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    if res.status() == StatusCode::CREATED {
        let set_cookie = res
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        return set_cookie.split(';').next().unwrap().to_string();
    }

    let req = Request::builder()
        .method("POST")
        .uri("/api/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"username":"admin","password":"adminpassword123"}"#,
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let set_cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    set_cookie.split(';').next().unwrap().to_string()
}

pub async fn body_json(res: axum::response::Response) -> serde_json::Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}
