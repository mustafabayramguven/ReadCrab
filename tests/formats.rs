mod common;

use std::{path::Path, time::Duration};

use common::{make_cbz, make_tar_as_cbr, png};
use kavita_rs::formats::{
    comic::{bsdtar_list, bsdtar_read, bsdtar_run, comic_pages, comic_read, glob_escape},
    Format, FormatError, MAX_ENTRY_BYTES,
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
    let fut = bsdtar_run("sleep", &["5"], Duration::from_millis(200), MAX_ENTRY_BYTES);
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
