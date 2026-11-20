mod store;

use store::*;
use tauri::{ipc::Request, Manager, State};

// All commands are `async` so Tauri runs them off the main/UI thread.

#[tauri::command]
async fn list_entries(store: State<'_, Store>, query: Option<EntryQuery>) -> Result<Vec<EntrySummary>> {
    store.list_entries(&query.unwrap_or_default())
}

#[tauri::command]
async fn get_entry(store: State<'_, Store>, id: String) -> Result<Entry> {
    store.get_entry(&id)
}

#[tauri::command]
async fn save_entry(store: State<'_, Store>, input: EntryInput) -> Result<Entry> {
    store.save_entry(input)
}

#[tauri::command]
async fn set_favorite(store: State<'_, Store>, id: String, favorite: bool) -> Result<()> {
    store.set_favorite(&id, favorite)
}

#[tauri::command]
async fn list_folders(store: State<'_, Store>) -> Result<Vec<Folder>> {
    store.list_folders()
}

#[tauri::command]
async fn save_folder(store: State<'_, Store>, input: FolderInput) -> Result<String> {
    store.save_folder(input)
}

#[tauri::command]
async fn delete_folder(store: State<'_, Store>, id: String) -> Result<()> {
    store.delete_folder(&id)
}

#[tauri::command]
async fn list_notes(store: State<'_, Store>, folder_id: Option<String>, tag: Option<String>) -> Result<Vec<NoteSummary>> {
    store.list_notes(folder_id, tag)
}

#[tauri::command]
async fn get_note(store: State<'_, Store>, id: String) -> Result<Note> {
    store.get_note(&id)
}

#[tauri::command]
async fn save_note(store: State<'_, Store>, input: NoteInput) -> Result<Note> {
    store.save_note(input)
}

#[tauri::command]
async fn list_tags(store: State<'_, Store>) -> Result<Vec<Tag>> {
    store.list_tags()
}

#[tauri::command]
async fn trash_item(store: State<'_, Store>, kind: String, id: String) -> Result<()> {
    store.trash(&kind, &id)
}

#[tauri::command]
async fn restore_item(store: State<'_, Store>, kind: String, id: String) -> Result<()> {
    store.restore(&kind, &id)
}

#[tauri::command]
async fn purge_item(store: State<'_, Store>, kind: String, id: String) -> Result<()> {
    store.purge(&kind, &id)
}

#[tauri::command]
async fn list_trash(store: State<'_, Store>) -> Result<Vec<TrashItem>> {
    store.list_trash()
}

#[tauri::command]
async fn search(store: State<'_, Store>, q: String, limit: Option<i64>) -> Result<Vec<SearchHit>> {
    store.search(&q, limit.unwrap_or(50))
}

/// Media upload uses a raw binary IPC body (no JSON/base64 blow-up); metadata rides in headers.
#[tauri::command]
async fn add_media(store: State<'_, Store>, request: Request<'_>) -> Result<Media> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected a binary body".into());
    };
    let h = request.headers();
    let get = |k: &str| -> Result<String> {
        let v = h.get(k).ok_or(format!("missing header {k}"))?;
        Ok(percent_decode(v.to_str().map_err(|e| e.to_string())?))
    };
    let (kind, id, name, mime) = (get("x-item-kind")?, get("x-item-id")?, get("x-filename")?, get("x-mime")?);
    store.add_media(NewMedia { item_kind: &kind, item_id: &id, filename: &name, mime: &mime, bytes })
}

#[tauri::command]
async fn delete_media(store: State<'_, Store>, id: String) -> Result<()> {
    store.delete_media(&id)
}

#[tauri::command]
async fn list_reminders(store: State<'_, Store>) -> Result<Vec<Reminder>> {
    store.list_reminders()
}

#[tauri::command]
async fn save_reminder(store: State<'_, Store>, reminder: Reminder) -> Result<Reminder> {
    store.save_reminder(reminder)
}

#[tauri::command]
async fn delete_reminder(store: State<'_, Store>, id: String) -> Result<()> {
    store.delete_reminder(&id)
}

#[tauri::command]
async fn stats(store: State<'_, Store>) -> Result<Stats> {
    store.stats()
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let store = Store::open(&dir).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            app.manage(store);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_entries,
            get_entry,
            save_entry,
            set_favorite,
            list_folders,
            save_folder,
            delete_folder,
            list_notes,
            get_note,
            save_note,
            list_tags,
            trash_item,
            restore_item,
            purge_item,
            list_trash,
            search,
            add_media,
            delete_media,
            list_reminders,
            save_reminder,
            delete_reminder,
            stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running LifeLogr");
}

#[cfg(test)]
mod tests {
    #[test]
    fn decodes_percent_encoding() {
        assert_eq!(super::percent_decode("caf%C3%A9%20photo.jpg"), "café photo.jpg");
        assert_eq!(super::percent_decode("100%"), "100%");
    }
}
