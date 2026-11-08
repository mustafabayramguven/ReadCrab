mod common;

use std::{path::Path, time::Duration};

use axum::{http::StatusCode, response::IntoResponse};
use common::{
    body_json, make_cbz, make_epub, make_epub_drm, make_epub_malicious, make_mobi,
    make_mobi_with_encryption, make_tar_as_cbr, png, EpubOpts,
};
use kavita_rs::{
    error::AppError,
    formats::{
        comic::{bsdtar_list, bsdtar_read, bsdtar_run, comic_pages, comic_read, glob_escape},
        epub::{epub_asset, epub_chapter_html, epub_open, normalize_zip_path},
        mobi::mobi_open,
        sanitize::{sanitize_chapter, svg_image_to_img, SrcMode, FRAME_CSP},
        Format, FormatError, MAX_ENTRY_BYTES,
    },
};

#[tokio::test]
async fn cbz_pages_natural_order() {
    let tmp = tempfile::tempdir().unwrap();
    let img = png(4, 4, [255, 0, 0]);
    let cbz = make_cbz(
        tmp.path(),
        "order.cbz",
        &[
            ("p10.png", img.clone()),
            ("p2.png", img.clone()),
            ("p1.png", img),
        ],
    );

    let pages = comic_pages(&cbz, Format::Cbz).await.unwrap();
    assert_eq!(pages, vec!["p1.png", "p2.png", "p10.png"]);
}

#[tokio::test]
async fn cbz_skips_zipslip_and_hidden_names() {
    let tmp = tempfile::tempdir().unwrap();
    let img = png(4, 4, [0, 255, 0]);
    let cbz = make_cbz(
        tmp.path(),
        "filter.cbz",
        &[
            ("valid.png", img.clone()),
            ("../escape.png", img.clone()),
            (".hidden.png", img.clone()),
            ("sub/.dot/p1.png", img.clone()),
            ("__MACOSX/p2.png", img.clone()),
            ("notes.txt", b"not an image".to_vec()),
        ],
    );

    let pages = comic_pages(&cbz, Format::Cbz).await.unwrap();
    assert_eq!(pages, vec!["valid.png"]);
}

#[tokio::test]
async fn cbz_entry_over_limit_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let big = vec![b'A'; 1024];
    let cbz = make_cbz(tmp.path(), "big.cbz", &[("p1.png", big)]);

    let res = comic_read(&cbz, Format::Cbz, "p1.png", 512).await;
    assert!(matches!(res, Err(FormatError::TooLarge)));
}

#[tokio::test]
async fn cbz_corrupt_returns_422() {
    let tmp = tempfile::tempdir().unwrap();
    let bad_path = tmp.path().join("corrupt.cbz");
    std::fs::write(&bad_path, b"not a zip file").unwrap();

    let err = comic_pages(&bad_path, Format::Cbz).await.unwrap_err();
    let res = AppError::from(err).into_response();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(res).await;
    assert_eq!(json["error"], "corrupt_book");
}

#[test]
fn rar_glob_escape_unit() {
    assert_eq!(glob_escape("a[1]*?.png"), r"a\[1\]\*\?.png");
    assert_eq!(glob_escape(r"x\y.png"), r"x\\y.png");
}

#[tokio::test]
async fn rar_via_bsdtar_lists_and_reads() {
    let tmp = tempfile::tempdir().unwrap();
    let img1 = png(4, 4, [10, 20, 30]);
    let img2 = png(4, 4, [99, 88, 77]);
    let cbr = make_tar_as_cbr(
        tmp.path(),
        "test.cbr",
        &[("p1.png", img1.clone()), ("[x]*.png", img2.clone())],
    );

    let pages = comic_pages(&cbr, Format::Cbr).await.unwrap();
    assert_eq!(pages, vec!["[x]*.png", "p1.png"]);

    let fut1 = comic_read(&cbr, Format::Cbr, "[x]*.png", MAX_ENTRY_BYTES);
    let read_glob = fut1.await.unwrap();
    assert_eq!(read_glob, img2);

    let fut2 = comic_read(&cbr, Format::Cbr, "p1.png", MAX_ENTRY_BYTES);
    let read_p1 = fut2.await.unwrap();
    assert_eq!(read_p1, img1);
}

#[tokio::test]
async fn rar_timeout_kills_process() {
    let dur = Duration::from_millis(200);
    let fut = bsdtar_run("sleep", &["5"], dur, MAX_ENTRY_BYTES);
    let res = fut.await;
    assert!(matches!(res, Err(FormatError::Timeout)));
}

#[tokio::test]
async fn rar_output_cap() {
    let tmp = tempfile::tempdir().unwrap();
    let data = vec![b'X'; 2048];
    let cbr = make_tar_as_cbr(tmp.path(), "cap.cbr", &[("p1.png", data)]);

    let res = comic_read(&cbr, Format::Cbr, "p1.png", 512).await;
    assert!(matches!(res, Err(FormatError::TooLarge)));
}

#[tokio::test]
async fn rar_real_fixture() {
    let rar5 = Path::new("tests/fixtures/rar5_stored.rar");
    let rar3 = Path::new("tests/fixtures/rar3.rar");
    assert!(rar5.exists(), "rar5_stored.rar bulunamadı");
    assert!(rar3.exists(), "rar3.rar bulunamadı");

    let list5 = bsdtar_list(rar5).await.unwrap();
    assert_eq!(list5, vec!["helloworld.txt"]);
    let content5 = bsdtar_read(rar5, "helloworld.txt").await.unwrap();
    assert_eq!(content5, b"hello libarchive test suite!\n");

    let list3 = bsdtar_list(rar3).await.unwrap();
    assert!(list3.contains(&"test.txt".to_string()));
    assert!(list3.contains(&"testdir/test.txt".to_string()));
    let content3 = bsdtar_read(rar3, "test.txt").await.unwrap();
    assert_eq!(content3, b"test text document\r\n");
}

#[test]
fn epub_toc_and_chapter() {
    let tmp = tempfile::tempdir().unwrap();
    let epub = make_epub(tmp.path(), "book.epub", EpubOpts::default());

    let info = epub_open(&epub).unwrap();
    assert_eq!(info.title.as_deref(), Some("Test EPUB Kitabı"));
    assert_eq!(info.spine.len(), 3);
    assert_eq!(
        info.toc,
        vec![
            ("Bölüm Bir".to_string(), 0),
            ("Bölüm İki".to_string(), 1),
            ("Bölüm Üç".to_string(), 2),
        ]
    );
    assert_eq!(info.cover.as_deref(), Some("OEBPS/images/cover.png"));

    let ch0 = epub_chapter_html(&epub, &info, 0).unwrap();
    assert!(ch0.contains("Birinci bölüm metni"));
}

#[test]
fn epub_asset_only_manifest_images() {
    let tmp = tempfile::tempdir().unwrap();
    let epub = make_epub(tmp.path(), "book.epub", EpubOpts::default());
    let info = epub_open(&epub).unwrap();

    let (bytes, mime) = epub_asset(&epub, &info, "OEBPS/images/pic.png").unwrap();
    assert_eq!(mime, "image/png");
    assert!(bytes.starts_with(b"\x89PNG"));

    let (bytes_sp, mime_sp) = epub_asset(&epub, &info, "OEBPS/images/foto %20 #1.png").unwrap();
    assert_eq!(mime_sp, "image/png");
    assert!(bytes_sp.starts_with(b"\x89PNG"));

    assert!(epub_asset(&epub, &info, "OEBPS/secret.txt").is_err());
    assert!(epub_asset(&epub, &info, "OEBPS/images/unlisted.png").is_err());
}

#[test]
fn epub_asset_path_traversal_404() {
    let tmp = tempfile::tempdir().unwrap();
    let epub = make_epub(tmp.path(), "book.epub", EpubOpts::default());
    let info = epub_open(&epub).unwrap();

    assert!(normalize_zip_path("OEBPS", "../../etc/passwd").is_none());
    assert!(normalize_zip_path("", "%2e%2e/etc/passwd").is_none());
    assert!(epub_asset(&epub, &info, "../../etc/passwd").is_err());
    assert!(epub_asset(&epub, &info, "%2e%2e/OEBPS/images/pic.png").is_err());
}

#[test]
fn epub_xml_entity_not_expanded() {
    let tmp = tempfile::tempdir().unwrap();
    let epub = make_epub_malicious(tmp.path());
    let info = epub_open(&epub).unwrap();
    let title = info.title.unwrap_or_default();
    assert!(!title.contains("lollol"));
    assert!(title.len() < 64);
}

#[tokio::test]
async fn epub_drm_returns_422_drm_protected() {
    let tmp = tempfile::tempdir().unwrap();
    let epub = make_epub_drm(tmp.path());
    let err = epub_open(&epub).unwrap_err();
    assert!(matches!(err, FormatError::Drm));

    let res = AppError::from(err).into_response();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(res).await;
    assert_eq!(json["error"], "drm_protected");
}

#[test]
fn epub_sanitizes_xss() {
    let tmp = tempfile::tempdir().unwrap();
    let epub = make_epub_malicious(tmp.path());
    let info = epub_open(&epub).unwrap();
    let raw = epub_chapter_html(&epub, &info, 0).unwrap();

    let clean = sanitize_chapter(&raw, 42, "OEBPS", SrcMode::Epub);
    assert!(!clean.contains("<script"));
    assert!(!clean.contains("onerror"));
    assert!(!clean.contains("javascript:"));
    assert!(!clean.contains("<svg"));
    assert!(!clean.contains("style="));
    assert!(!clean.contains("evil.example"));
    assert!(!clean.contains("etc/passwd"));
    assert!(clean.contains("/api/books/42/asset?p=OEBPS%2Fimages%2Fpic%2Epng"));
    assert!(clean.contains("href=\"#\""));
    assert!(clean.contains(r#"href="https://example.com/safe""#));
    assert!(clean.contains("Temiz içerik"));

    assert_eq!(
        FRAME_CSP,
        "default-src 'none'; img-src 'self'; style-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'"
    );
    assert!(!FRAME_CSP.contains("sandbox"));
}

#[test]
fn epub_svg_cover_becomes_img() {
    let tmp = tempfile::tempdir().unwrap();
    let epub = make_epub(
        tmp.path(),
        "svg_cover.epub",
        EpubOpts {
            title: None,
            svg_cover: true,
        },
    );
    let info = epub_open(&epub).unwrap();
    let raw = epub_chapter_html(&epub, &info, 0).unwrap();

    let clean = sanitize_chapter(&raw, 7, "OEBPS", SrcMode::Epub);
    assert!(!clean.contains("<svg"));
    assert!(clean.contains(r#"<img src="/api/books/7/asset?p=OEBPS%2Fimages%2Fcover%2Epng">"#));

    let (bytes, mime) = epub_asset(&epub, &info, "OEBPS/images/cover.png").unwrap();
    assert_eq!(mime, "image/png");
    assert!(bytes.starts_with(b"\x89PNG"));
}

#[test]
fn svg_without_image_untouched() {
    let input = r#"<svg viewBox="0 0 10 10"><rect width="10" height="10"/></svg><p>Merhaba</p>"#;
    let step = svg_image_to_img(input);
    assert_eq!(step, input);

    let clean = sanitize_chapter(input, 1, "", SrcMode::Epub);
    assert!(!clean.contains("<svg"));
    assert!(clean.contains("<p>Merhaba</p>"));
}

#[test]
fn mobi_chapters_and_images() {
    let tmp = tempfile::tempdir().unwrap();
    let mobi_path = make_mobi(tmp.path(), "sample.mobi");

    let book = mobi_open(&mobi_path).unwrap();
    assert_eq!(book.title, "Test MOBI Kitabi");
    assert_eq!(book.chapters.len(), 2);
    assert!(book.chapters[0].contains("Bolum 1"));
    assert!(book.chapters[1].contains("Bolum 2"));
    assert_eq!(book.images.len(), 1);
    assert!(book.images[0].starts_with(b"\x89PNG"));

    let clean_ch2 = sanitize_chapter(&book.chapters[1], 9, "", SrcMode::Mobi);
    assert!(clean_ch2.contains(r#"<img src="/api/books/9/asset?p=1">"#));
}

#[test]
fn mobi_drm_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let drm_path = make_mobi_with_encryption(tmp.path(), "drm.mobi", 2);
    let res = mobi_open(&drm_path);
    assert!(matches!(res, Err(FormatError::Drm)));
}
