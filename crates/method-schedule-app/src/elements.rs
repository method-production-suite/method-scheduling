//! The desktop's half of elements' pictures: reading an image file beside the schedule
//! and the Open dialog for one. Every edit to categories, elements and their properties
//! is `method-schedule-session`'s, re-exported here under its old name.

use std::path::{Path, PathBuf};

use tauri::{Runtime, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

pub use method_schedule_session::elements::*;

/// The image a stored path names (an element's or a sheet's) as a `data:` URL for its
/// thumbnail, or `None`: the page then shows it as missing. Read only when it is a plain
/// file (not a link: a schedule from elsewhere may name any path) with an image name, of
/// at most [`MAX_IMAGE_BYTES`], whose bytes are one of the image types [`sniff_image`] knows. Reads
/// the disk, so never under a schedule's lock or on the main thread.
pub fn read_image(doc: Option<&Path>, stored: &str) -> Option<String> {
    use std::io::Read;
    // The untitled sample's pictures are carried in the app (M2c).
    if doc.is_none() {
        if let Some(bytes) = crate::session::sample_image(stored) {
            return image_data_url(bytes);
        }
    }
    let path = resolve_image_path(doc, stored)?;
    image_type(&path)?;
    let meta = std::fs::symlink_metadata(&path).ok()?;
    if !meta.file_type().is_file() || meta.len() > MAX_IMAGE_BYTES {
        return None;
    }
    let file = std::fs::File::open(&path).ok()?;
    if !same_file_as(&meta, &file.metadata().ok()?) {
        // Swapped for something else since it was looked at.
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return None;
    }
    image_data_url(&bytes)
}

/// Whether two looks at a path saw the same plain file.
fn same_file_as(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.dev() == b.dev() && a.ino() == b.ino() && b.is_file()
    }
    #[cfg(not(unix))]
    {
        a.len() == b.len() && b.is_file()
    }
}

/// An Open dialog for an image, starting in the schedule's folder (`doc`). `None` when
/// cancelled. Blocks: async commands only, with no lock held.
pub fn pick_image<R: Runtime>(
    window: &WebviewWindow<R>,
    title: &str,
    doc: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    let types: Vec<&str> = IMAGE_TYPES.iter().map(|(e, _)| *e).collect();
    let mut dialog = window
        .dialog()
        .file()
        .set_title(title)
        .add_filter("Image", &types)
        .set_parent(window);
    if let Some(dir) = doc.and_then(Path::parent) {
        dialog = dialog.set_directory(dir);
    }
    match dialog.blocking_pick_file() {
        None => Ok(None),
        Some(p) => p.into_path().map(Some).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{read_script, Session};
    use method_schedule_core::model::ElementId;
    use method_schedule_core::Schedule;
    use std::path::Path;

    fn harbor() -> Session {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scripts/harbor-light.fountain");
        // Cast only, so the category order below is short.
        let mut s = Session::with_schedule(Schedule::new("ZTEST"), None);
        s.import_script(&read_script(&path).unwrap()).unwrap();
        s
    }

    fn el(s: &Session, name: &str) -> u32 {
        s.schedule()
            .elements
            .iter()
            .find(|e| e.name == name)
            .unwrap()
            .id
            .0
    }

    #[test]
    fn images_are_paths_beside_the_schedule() {
        let dir = std::env::temp_dir().join(format!("ms-images-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("images")).unwrap();
        let doc = dir.join("show.mtsc");
        let inside = dir.join("images").join("kit.png");
        std::fs::write(&inside, b"\x89PNG\r\n\x1a\n").unwrap();
        assert_eq!(stored_image_path(Some(&doc), &inside), "images/kit.png");
        let outside = std::env::temp_dir().join("elsewhere.png");
        assert_eq!(
            stored_image_path(Some(&doc), &outside),
            outside.to_string_lossy()
        );
        assert_eq!(
            stored_image_path(None, &inside),
            inside.to_string_lossy(),
            "never saved: absolute"
        );
        assert_eq!(
            resolve_image_path(Some(&doc), "images/kit.png"),
            Some(inside.clone())
        );
        assert_eq!(resolve_image_path(Some(&doc), "../kit.png"), None);
        assert_eq!(resolve_image_path(None, "images/kit.png"), None);
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b""), "");

        let mut s = harbor();
        let kit = el(&s, "KIT");
        assert!(s.edit(|x| add_image(x, kit, "notes.txt".into())).is_err());
        s.edit(|x| add_image(x, kit, "images/kit.png".into()))
            .unwrap();
        assert!(s
            .edit(|x| add_image(x, kit, "images/kit.png".into()))
            .is_err());
        s.edit(|x| add_image(x, kit, "images/gone.jpg".into()))
            .unwrap();
        let read = |at| read_image(Some(&doc), &image_of(s.schedule(), kit, at).unwrap());
        assert_eq!(read(0).unwrap(), "data:image/png;base64,iVBORw0KGgo=");
        assert_eq!(read(1), None);
        assert!(image_of(s.schedule(), kit, 9).is_err());
        s.edit(|x| remove_image(x, kit, 1)).unwrap();
        assert_eq!(
            s.schedule().element(ElementId(kit)).unwrap().images,
            ["images/kit.png"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A schedule from elsewhere may name any path: only plain local files whose bytes
    /// are an image are read, never a link, a network share or a device, and never more
    /// than the size cap.
    #[test]
    fn image_reads_are_careful() {
        let dir = std::env::temp_dir().join(format!("ms-image-reads-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let doc = dir.join("show.mtsc");
        assert_eq!(sniff_image(b"\x89PNG\r\n\x1a\n...."), Some("image/png"));
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff_image(b"GIF89a.."), Some("image/gif"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WAVE"), None);
        assert_eq!(sniff_image(b"II*\0\x08\0\0\0"), Some("image/tiff"));
        assert_eq!(sniff_image(b"MM\0*\0\0\0\x08"), Some("image/tiff"));
        assert_eq!(sniff_image(b"II+\0"), None);
        assert_eq!(
            sniff_image(b"BM\x46\0\0\0\0\0\0\0\x36\0\0\0"),
            Some("image/bmp")
        );
        assert_eq!(sniff_image(b"BM"), None);
        assert_eq!(
            sniff_image(b"\0\0\0\x18ftypheic\0\0\0\0"),
            Some("image/heic")
        );
        assert_eq!(
            sniff_image(b"\0\0\0\x18ftypmif1\0\0\0\0"),
            Some("image/heic")
        );
        assert_eq!(sniff_image(b"\0\0\0\x18ftypheix"), Some("image/heic"));
        assert_eq!(sniff_image(b"\0\0\0\x18ftypmsf1"), Some("image/heic"));
        assert_eq!(sniff_image(b"\0\0\0\x18ftypisom\0\0\0\0"), None);
        assert_eq!(sniff_image(b"<svg onload=x>"), None);
        assert_eq!(sniff_image(b""), None);

        // A text file named like an image is not read.
        std::fs::write(dir.join("fake.png"), b"not an image at all").unwrap();
        assert_eq!(read_image(Some(&doc), "fake.png"), None);
        // A JPEG named .png is read as what it is.
        std::fs::write(dir.join("photo.png"), [0xFF, 0xD8, 0xFF, 0xE0, 1, 2]).unwrap();
        assert!(read_image(Some(&doc), "photo.png")
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        // TIFF, BMP and HEIC files are read with their own media types.
        std::fs::write(dir.join("scan.tif"), b"II*\0\x08\0\0\0").unwrap();
        assert!(read_image(Some(&doc), "scan.tif")
            .unwrap()
            .starts_with("data:image/tiff;base64,"));
        std::fs::write(dir.join("old.bmp"), b"BM\x46\0\0\0\0\0\0\0\x36\0\0\0").unwrap();
        assert!(read_image(Some(&doc), "old.bmp")
            .unwrap()
            .starts_with("data:image/bmp;base64,"));
        std::fs::write(dir.join("phone.HEIC"), b"\0\0\0\x18ftypheic\0\0\0\0").unwrap();
        assert!(read_image(Some(&doc), "phone.HEIC")
            .unwrap()
            .starts_with("data:image/heic;base64,"));
        // A PDF is not an image, whatever its name.
        std::fs::write(dir.join("doc.png"), b"%PDF-1.7").unwrap();
        assert_eq!(read_image(Some(&doc), "doc.png"), None);
        assert_eq!(image_type(Path::new("a.pdf")), None);
        assert_eq!(image_type(Path::new("a.TIFF")), Some("image/tiff"));
        // Over the cap.
        let big = std::fs::File::create(dir.join("big.png")).unwrap();
        big.set_len(MAX_IMAGE_BYTES + 1).unwrap();
        assert_eq!(read_image(Some(&doc), "big.png"), None);
        // A link, even to a real image.
        std::fs::write(dir.join("real.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.join("real.png"), dir.join("link.png")).unwrap();
            assert!(read_image(Some(&doc), "real.png").is_some());
            assert_eq!(read_image(Some(&doc), "link.png"), None);
        }
        // Network shares and devices are never reached (paths that only Windows reads
        // this way; elsewhere they are plain relative names).
        for p in [
            r"\\server\share\kit.png",
            r"\\?\UNC\server\share\kit.png",
            r"\\.\pipe\kit.png",
        ] {
            let resolved = resolve_image_path(Some(&doc), p);
            if cfg!(windows) {
                assert_eq!(resolved, None, "{p}");
            } else {
                assert_eq!(resolved, Some(dir.join(p)), "{p}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
