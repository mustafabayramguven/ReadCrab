use std::{io::Cursor, path::Path};

use image::{codecs::jpeg::JpegEncoder, ImageReader, Limits};

use crate::{
    formats::{
        comic::{comic_pages, comic_read},
        epub::{epub_asset, epub_open},
        mobi::mobi_open,
        Format, MAX_ENTRY_BYTES, MAX_PIXELS,
    },
    safepath::resolve_book_path,
};

#[derive(Clone, Debug)]
pub struct BookRow {
    pub id: i64,
    pub rel_path: String,
    pub format: Format,
}

pub async fn extract_cover(root: &Path, book: &BookRow, covers_dir: &Path) -> bool {
    let Ok(full_path) = resolve_book_path(root, &book.rel_path) else {
        return false;
    };

    let raw_bytes = match book.format {
        Format::Cbz | Format::Zip | Format::Rar | Format::Cbr => {
            let Ok(pages) = comic_pages(&full_path, book.format).await else {
                return false;
            };
            let Some(first) = pages.first() else {
                return false;
            };
            let Ok(bytes) = comic_read(&full_path, book.format, first, MAX_ENTRY_BYTES).await
            else {
                return false;
            };
            bytes
        }
        Format::Epub => {
            let p = full_path.clone();
            let Ok(Some(bytes)) = tokio::task::spawn_blocking(move || {
                let info = epub_open(&p).ok()?;
                let cover_href = info.cover.as_deref()?;
                let (bytes, _) = epub_asset(&p, &info, cover_href).ok()?;
                Some(bytes)
            })
            .await
            else {
                return false;
            };
            bytes
        }
        Format::Mobi => {
            let p = full_path.clone();
            let Ok(Some(bytes)) = tokio::task::spawn_blocking(move || {
                let m = mobi_open(&p).ok()?;
                m.images.into_iter().next()
            })
            .await
            else {
                return false;
            };
            bytes
        }
    };

    let out_path = covers_dir.join(format!("{}.jpg", book.id));
    tokio::task::spawn_blocking(move || resize_and_save_jpeg(&raw_bytes, &out_path))
        .await
        .unwrap_or(false)
}

fn resize_and_save_jpeg(raw_bytes: &[u8], out_path: &Path) -> bool {
    let Ok(dim_reader) = ImageReader::new(Cursor::new(raw_bytes)).with_guessed_format() else {
        return false;
    };
    let Ok((w, h)) = dim_reader.into_dimensions() else {
        return false;
    };
    if w == 0 || h == 0 || (w as u64) * (h as u64) > MAX_PIXELS {
        return false;
    }

    let Ok(mut reader) = ImageReader::new(Cursor::new(raw_bytes)).with_guessed_format() else {
        return false;
    };
    let mut limits = Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);

    let Ok(img) = reader.decode() else {
        return false;
    };

    let new_h = ((h as u64 * 300) / (w as u64)).max(1) as u32;
    let thumb = img.thumbnail_exact(300, new_h);
    let rgb = thumb.to_rgb8();

    let mut jpeg_buf = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_buf, 80);
    if encoder.encode_image(&rgb).is_err() {
        return false;
    }

    if let Some(parent) = out_path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    std::fs::write(out_path, jpeg_buf).is_ok()
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use image::{ImageFormat, Rgb, RgbImage};
    use zip::{write::SimpleFileOptions, ZipWriter};

    use super::*;

    fn test_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(w, h, |_, _| Rgb([10, 20, 30]));
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    #[tokio::test]
    async fn extract_cover_resizes_to_300px_jpeg() {
        let root = tempfile::tempdir().unwrap();
        let covers = tempfile::tempdir().unwrap();
        let cbz_path = root.path().join("book.cbz");
        let f = File::create(&cbz_path).unwrap();
        let mut zip = ZipWriter::new(f);
        zip.start_file("01.png", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&test_png(600, 900)).unwrap();
        zip.finish().unwrap();

        let book = BookRow {
            id: 7,
            rel_path: "book.cbz".to_string(),
            format: Format::Cbz,
        };
        assert!(extract_cover(root.path(), &book, covers.path()).await);

        let cover_file = covers.path().join("7.jpg");
        assert!(cover_file.exists());
        let bytes = std::fs::read(&cover_file).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), 300);
        assert_eq!(decoded.height(), 450);
    }
}
