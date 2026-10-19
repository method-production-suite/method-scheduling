//! Edit > Find and Edit > Replace (decisions.md, entries 28 and 34). The engine does the
//! work (`method_schedule_core::find`); this reads what the page sends, turns the results
//! into what the Find panel draws (ui/js/find.js), and makes a replace one undo step with
//! its refusals as plain sentences.
//!
//! A result travels as a key (`sheet:12:set`, `element:5`, `category:2`,
//! `banner:1:3:40`, `event:1:3:0`) and comes back the same way for Replace; every key is
//! parsed and checked here, and the engine refuses a pick that no longer matches.
//! Method Budget's `src/find.rs` is the model (at most 1,000 results listed, text cut
//! short), scaled down to the engine's typed search.
use crate::app::{finish, CmdResult};
use crate::docs::Doc;
use crate::view::View;
use tauri::{Runtime, WebviewWindow};

pub use method_schedule_session::find::*;

/// Every place `query` matches, in board order.
#[tauri::command]
pub fn find_text(state: Doc, query: QueryArg) -> CmdResult<FoundDto> {
    find(state.lock().schedule(), &query)
}

/// Replaces `query`'s matches by `with` in the checked results `picks` (their keys), as
/// one undo step. The view carries a notice saying how many places changed.
#[tauri::command]
pub fn replace_text<R: Runtime>(
    window: WebviewWindow<R>,
    state: Doc,
    query: QueryArg,
    with: String,
    picks: Vec<String>,
) -> CmdResult<View> {
    let mut s = state.lock();
    let notice = replace(&mut s, &query, &with, &picks)?;
    finish(&window, &s, Some(notice))
}
