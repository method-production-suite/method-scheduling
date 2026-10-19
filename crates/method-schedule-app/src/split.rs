//! The split screen's data (decisions.md, entry 80) and images on scenes (entry 77):
//! another board drawn beside the board shown, and a sheet's image paths with their
//! thumbnails, as an element's are (src/elements.rs, decision 62). Paths come from a Rust
//! Open dialog, never from the page; files are read as an element's images are
//! (`elements::read_image`).
use crate::app::{finish, CmdResult};
use crate::docs::Doc;
use crate::elements::{pick_image, read_image, stored_image_path};
use crate::view::{self, BoardDto, View};
use method_schedule_core::model::BoardId;
use std::path::PathBuf;
use tauri::{Runtime, WebviewWindow};

pub use method_schedule_session::split::*;

/// Board `board` as the page draws it, for the board beside the board shown (not an
/// edit, and the board shown stays as it is).
#[tauri::command]
pub fn board_view(state: Doc, board: u32) -> CmdResult<BoardDto> {
    let s = state.lock();
    view::board_ref(s.schedule(), board)?;
    view::board(s.schedule(), BoardId(board))
}

/// Shows an Open dialog for an image and adds its path to a sheet, as an element's
/// images are added. `None` when cancelled.
#[tauri::command]
pub async fn add_sheet_image<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sheet: u32,
) -> CmdResult<Option<View>> {
    let doc = state.lock().path().map(PathBuf::from);
    let Some(path) = pick_image(&window, "Add image to the sheet", doc.as_deref())? else {
        return Ok(None);
    };
    let stored = stored_image_path(doc.as_deref(), &path);
    let mut s = state.lock();
    s.edit(|x| add_image(x, sheet, stored))?;
    finish(&window, &s, None).map(Some)
}

/// Takes image `at` off a sheet; the file stays where it is.
#[tauri::command]
pub fn remove_sheet_image<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    sheet: u32,
    at: usize,
) -> CmdResult {
    let mut s = state.lock();
    s.edit(|x| remove_image(x, sheet, at))?;
    finish(&window, &s, None)
}

/// Image `at` of a sheet as a `data:` URL for its thumbnail, or `None` when the file is
/// missing or unreadable. Async, and the file is read with the lock let go.
#[tauri::command]
pub async fn sheet_image(state: Doc, sheet: u32, at: usize) -> CmdResult<Option<String>> {
    let (stored, doc) = {
        let s = state.lock();
        (
            image_of(s.schedule(), sheet, at)?,
            s.path().map(PathBuf::from),
        )
    };
    Ok(read_image(doc.as_deref(), &stored))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use method_schedule_core::model::Sheet;
    use method_schedule_core::model::SheetId;
    use std::path::Path;

    #[test]
    fn sheet_images_are_paths_with_thumbnails() {
        let dir = std::env::temp_dir().join(format!("ms-sheet-images-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("images")).unwrap();
        let doc = dir.join("film.mtsc");
        std::fs::write(
            dir.join("images").join("loft.png"),
            b"\x89PNG\r\n\x1a\nfake",
        )
        .unwrap();
        let mut s = Session::new();
        let mut sheet = 0;
        s.edit(|x| {
            sheet = x
                .add_sheet(Sheet {
                    scenes: "12".into(),
                    ..Sheet::default()
                })
                .0;
            Ok(())
        })
        .unwrap();
        s.edit(|x| add_image(x, sheet, "images/loft.png".into()))
            .unwrap();
        assert!(s
            .edit(|x| add_image(x, sheet, "images/loft.png".into()))
            .unwrap_err()
            .contains("already"));
        assert!(s
            .edit(|x| add_image(x, sheet, "notes.txt".into()))
            .unwrap_err()
            .contains("PNG"));
        s.edit(|x| add_image(x, sheet, "images/gone.jpg".into()))
            .unwrap();
        let x = s.schedule();
        assert_eq!(
            x.sheet(SheetId(sheet)).unwrap().images,
            ["images/loft.png", "images/gone.jpg"]
        );
        let read = |doc: Option<&Path>, at| read_image(doc, &image_of(x, sheet, at).unwrap());
        let url = read(Some(&doc), 0).unwrap();
        assert!(url.starts_with("data:image/png;base64,"), "{url}");
        assert_eq!(read(Some(&doc), 1), None);
        // Never saved: a relative path can't be found.
        assert_eq!(read(None, 0), None);
        assert!(image_of(x, sheet, 5).is_err());
        s.edit(|x| remove_image(x, sheet, 0)).unwrap();
        assert_eq!(
            s.schedule().sheet(SheetId(sheet)).unwrap().images,
            ["images/gone.jpg"]
        );
        assert!(s.edit(|x| remove_image(x, sheet, 3)).is_err());
        assert!(s.edit(|x| remove_image(x, 9999, 0)).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
