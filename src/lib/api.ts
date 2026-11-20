// Typed wrappers over the Rust commands in src-tauri/src/lib.rs.
// This is the "store adapter" boundary from the PRD: UI code only talks to `api`.
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

export interface Media {
  id: string;
  item_kind: "entry" | "note";
  item_id: string;
  kind: "image" | "video" | "audio" | "file";
  filename: string;
  mime: string;
  size: number;
  path: string;
  thumb_path: string | null;
  created_at: string;
}

export interface Entry {
  id: string;
  entry_date: string;
  title: string;
  body: string;
  favorite: boolean;
  tags: string[];
  media: Media[];
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
}

export interface EntrySummary {
  id: string;
  entry_date: string;
  title: string;
  excerpt: string;
  favorite: boolean;
  tags: string[];
  media_count: number;
  cover_thumb: string | null;
  updated_at: string;
}

export interface EntryInput {
  id?: string;
  entry_date: string;
  title: string;
  body: string;
  favorite: boolean;
  tags: string[];
}

export interface Folder {
  id: string;
  parent_id: string | null;
  name: string;
  note_count: number;
}

export interface Note {
  id: string;
  folder_id: string | null;
  title: string;
  body: string;
  pinned: boolean;
  tags: string[];
  media: Media[];
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
}

export interface NoteSummary {
  id: string;
  folder_id: string | null;
  title: string;
  excerpt: string;
  pinned: boolean;
  tags: string[];
  updated_at: string;
}

export interface NoteInput {
  id?: string;
  folder_id: string | null;
  title: string;
  body: string;
  pinned: boolean;
}

export interface Tag {
  name: string;
  count: number;
}

export interface SearchHit {
  kind: "entry" | "note";
  id: string;
  title: string;
  /** Matches are wrapped in \u0001 … \u0002 so the UI can highlight without trusting HTML. */
  snippet: string;
  date: string;
}

export type Repeat = "none" | "daily" | "weekly" | "monthly";

export interface Reminder {
  id?: string;
  title: string;
  body: string;
  next_at: string;
  repeat: Repeat;
  enabled: boolean;
  last_fired_at?: string | null;
}

export interface TrashItem {
  kind: "entry" | "note";
  id: string;
  title: string;
  deleted_at: string;
}

export interface Stats {
  entries: number;
  notes: number;
  media: number;
  media_bytes: number;
  db_bytes: number;
  data_dir: string;
}

export const api = {
  listEntries: (query: { tag?: string | null; favorites_only?: boolean; limit?: number; offset?: number } = {}) =>
    invoke<EntrySummary[]>("list_entries", { query }),
  getEntry: (id: string) => invoke<Entry>("get_entry", { id }),
  saveEntry: (input: EntryInput) => invoke<Entry>("save_entry", { input }),
  setFavorite: (id: string, favorite: boolean) => invoke<void>("set_favorite", { id, favorite }),

  listFolders: () => invoke<Folder[]>("list_folders"),
  saveFolder: (input: { id?: string; parent_id: string | null; name: string }) => invoke<string>("save_folder", { input }),
  deleteFolder: (id: string) => invoke<void>("delete_folder", { id }),
  listNotes: (folderId: string | null, tag: string | null = null) =>
    invoke<NoteSummary[]>("list_notes", { folderId, tag }),
  getNote: (id: string) => invoke<Note>("get_note", { id }),
  saveNote: (input: NoteInput) => invoke<Note>("save_note", { input }),

  listTags: () => invoke<Tag[]>("list_tags"),
  trash: (kind: "entry" | "note", id: string) => invoke<void>("trash_item", { kind, id }),
  restore: (kind: "entry" | "note", id: string) => invoke<void>("restore_item", { kind, id }),
  purge: (kind: "entry" | "note", id: string) => invoke<void>("purge_item", { kind, id }),
  listTrash: () => invoke<TrashItem[]>("list_trash"),
  search: (q: string, limit = 50) => invoke<SearchHit[]>("search", { q, limit }),

  async addMedia(itemKind: "entry" | "note", itemId: string, file: Blob, filename: string): Promise<Media> {
    const bytes = new Uint8Array(await file.arrayBuffer());
    return invoke<Media>("add_media", bytes, {
      headers: {
        "x-item-kind": itemKind,
        "x-item-id": itemId,
        "x-filename": encodeURIComponent(filename),
        "x-mime": encodeURIComponent(file.type || "application/octet-stream"),
      },
    });
  },
  deleteMedia: (id: string) => invoke<void>("delete_media", { id }),

  listReminders: () => invoke<Reminder[]>("list_reminders"),
  saveReminder: (reminder: Reminder) => invoke<Reminder>("save_reminder", { reminder }),
  deleteReminder: (id: string) => invoke<void>("delete_reminder", { id }),

  stats: () => invoke<Stats>("stats"),
};

/** URL for a file in the app's media folder (served by Tauri's asset protocol). */
export function fileUrl(path: string): string {
  return path.startsWith("blob:") || path.startsWith("data:") ? path : convertFileSrc(path);
}

export const isTauri = () => "__TAURI_INTERNALS__" in window && !(window as any).__LIFELOGR_MOCK__;
export const isAndroid = () => /Android/i.test(navigator.userAgent);
