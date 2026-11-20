// In-memory backend used only when the UI runs in a plain browser (`npm run dev` without Tauri).
// It lets you click around the UI and take screenshots; nothing is persisted.
import { mockIPC } from "@tauri-apps/api/mocks";
import { api, type Entry, type Folder, type Media, type Note, type Reminder } from "./api";
import { today } from "./dates";

export function installMock() {
  (window as any).__LIFELOGR_MOCK__ = true;
  const now = () => new Date().toISOString();
  const uid = () => crypto.randomUUID();
  const entries = new Map<string, Entry>();
  const notes = new Map<string, Note>();
  const folders = new Map<string, Folder>();
  const reminders = new Map<string, Reminder>();
  const media = new Map<string, Media>();

  const day = (offset: number) => today(new Date(Date.now() - offset * 86_400_000));
  const seed: [number, string, string, string[], boolean][] = [
    [0, "Slow morning", "Coffee on the balcony before anyone woke up. Finally finished **Piranesi**.\n\n- Call mom\n- Book the train for Saturday", ["reading", "morning"], true],
    [1, "Ridge walk", "Took the long route up the ridge. Fog burned off by 9 and the whole valley opened up.", ["outdoors"], false],
    [1, "", "Idea: keep a one-line-a-day habit for December.", ["ideas"], false],
    [4, "Team offsite", "Good energy. We agreed to ship the beta *before* the holidays.", ["work"], false],
  ];
  for (const [o, title, body, tags, fav] of seed) {
    const id = uid();
    entries.set(id, { id, entry_date: day(o), title, body, favorite: fav, tags, media: [], created_at: now(), updated_at: now(), deleted_at: null });
  }
  const fid = uid();
  folders.set(fid, { id: fid, parent_id: null, name: "Projects", note_count: 0 });
  const nid = uid();
  notes.set(nid, { id: nid, folder_id: fid, title: "Garden plan", body: "Tomatoes along the fence. #garden #spring\n\n- [ ] Order seeds\n- [x] Build the raised bed", pinned: true, tags: ["garden", "spring"], media: [], created_at: now(), updated_at: now(), deleted_at: null });

  const hashtags = (b: string) => [...new Set([...b.matchAll(/(?:^|\s)#([\p{L}\p{N}_\-/]*\p{L}[\p{L}\p{N}_\-/]*)/gu)].map((m) => m[1]))];
  const excerpt = (b: string, n: number) => {
    const f = b.replace(/\[[ x]\]/g, "").replace(/[#>*_`-]/g, "").replace(/\s+/g, " ").trim();
    return f.length > n ? f.slice(0, n) + "…" : f;
  };
  const withMedia = <T extends { id: string; media: Media[] }>(x: T): T => ({ ...x, media: [...media.values()].filter((m) => m.item_id === x.id) });

  mockIPC((cmd, a: any) => {
    switch (cmd) {
      case "list_entries": {
        const q = a.query || {};
        return [...entries.values()]
          .filter((e) => !e.deleted_at && (!q.favorites_only || e.favorite) && (!q.tag || e.tags.includes(q.tag)))
          .sort((x, y) => y.entry_date.localeCompare(x.entry_date) || y.created_at.localeCompare(x.created_at))
          .map((e) => {
            const m = [...media.values()].filter((m) => m.item_id === e.id);
            return { ...e, excerpt: excerpt(e.body, 160), media_count: m.length, cover_thumb: m.find((x) => x.kind === "image")?.path ?? null };
          });
      }
      case "get_entry":
        if (!entries.has(a.id)) throw "Entry not found";
        return withMedia(entries.get(a.id)!);
      case "save_entry": {
        const i = a.input;
        const id = i.id || uid();
        const prev = entries.get(id);
        entries.set(id, { ...(prev || { created_at: now(), deleted_at: null, media: [] }), ...i, id, updated_at: now(), tags: i.tags.map((t: string) => t.replace(/^#/, "")) } as Entry);
        return withMedia(entries.get(id)!);
      }
      case "set_favorite":
        entries.get(a.id)!.favorite = a.favorite;
        return null;
      case "list_folders":
        return [...folders.values()].map((f) => ({ ...f, note_count: [...notes.values()].filter((n) => n.folder_id === f.id && !n.deleted_at).length }));
      case "save_folder": {
        const id = a.input.id || uid();
        folders.set(id, { id, parent_id: a.input.parent_id, name: a.input.name, note_count: 0 });
        return id;
      }
      case "delete_folder":
        folders.delete(a.id);
        notes.forEach((n) => n.folder_id === a.id && (n.folder_id = null));
        return null;
      case "list_notes":
        return [...notes.values()]
          .filter((n) => !n.deleted_at && (a.folderId == null || (a.folderId === "" ? !n.folder_id : n.folder_id === a.folderId)) && (!a.tag || n.tags.includes(a.tag)))
          .sort((x, y) => Number(y.pinned) - Number(x.pinned) || y.updated_at.localeCompare(x.updated_at))
          .map((n) => ({ ...n, excerpt: excerpt(n.body, 120) }));
      case "get_note":
        if (!notes.has(a.id)) throw "Note not found";
        return withMedia(notes.get(a.id)!);
      case "save_note": {
        const i = a.input;
        const id = i.id || uid();
        const prev = notes.get(id);
        notes.set(id, { ...(prev || { created_at: now(), deleted_at: null, media: [] }), ...i, id, updated_at: now(), tags: hashtags(i.body) } as Note);
        return withMedia(notes.get(id)!);
      }
      case "list_tags": {
        const c = new Map<string, number>();
        [...entries.values(), ...notes.values()].filter((x) => !x.deleted_at).forEach((x) => x.tags.forEach((t) => c.set(t, (c.get(t) || 0) + 1)));
        return [...c].map(([name, count]) => ({ name, count })).sort((a, b) => b.count - a.count);
      }
      case "trash_item":
      case "restore_item": {
        const m = a.kind === "entry" ? entries : notes;
        m.get(a.id)!.deleted_at = cmd === "trash_item" ? now() : null;
        return null;
      }
      case "purge_item":
        (a.kind === "entry" ? entries : notes).delete(a.id);
        return null;
      case "list_trash":
        return [
          ...[...entries.values()].filter((e) => e.deleted_at).map((e) => ({ kind: "entry", id: e.id, title: e.title || e.entry_date, deleted_at: e.deleted_at })),
          ...[...notes.values()].filter((n) => n.deleted_at).map((n) => ({ kind: "note", id: n.id, title: n.title || "Untitled note", deleted_at: n.deleted_at })),
        ];
      case "search": {
        const words = String(a.q).toLowerCase().split(/\s+/).filter(Boolean);
        if (!words.length) return [];
        const hit = (s: string) => words.every((w) => s.toLowerCase().includes(w));
        const snip = (b: string) => b.replace(new RegExp(`(${words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})`, "gi"), "\u0001$1\u0002").slice(0, 160);
        return [
          ...[...entries.values()].filter((e) => !e.deleted_at && hit(e.title + " " + e.body)).map((e) => ({ kind: "entry", id: e.id, title: e.title, snippet: snip(e.body), date: e.entry_date })),
          ...[...notes.values()].filter((n) => !n.deleted_at && hit(n.title + " " + n.body)).map((n) => ({ kind: "note", id: n.id, title: n.title, snippet: snip(n.body), date: n.updated_at.slice(0, 10) })),
        ];
      }
      case "delete_media":
        media.delete(a.id);
        return null;
      case "list_reminders":
        return [...reminders.values()].sort((x, y) => x.next_at.localeCompare(y.next_at));
      case "save_reminder": {
        const id = a.reminder.id || uid();
        reminders.set(id, { ...a.reminder, id });
        return reminders.get(id);
      }
      case "delete_reminder":
        reminders.delete(a.id);
        return null;
      case "stats":
        return { entries: entries.size, notes: notes.size, media: media.size, media_bytes: [...media.values()].reduce((s, m) => s + m.size, 0), db_bytes: 0, data_dir: "(browser preview: in memory)" };
      default:
        if (cmd.startsWith("plugin:notification")) return cmd.endsWith("is_permission_granted") ? false : "denied";
        throw new Error(`mock: unknown command ${cmd}`);
    }
  });

  api.addMedia = async (itemKind, itemId, file, filename) => {
    const url = URL.createObjectURL(file);
    const t = file.type || "application/octet-stream";
    const kind = t.startsWith("image/") ? "image" : t.startsWith("audio/") ? "audio" : t.startsWith("video/") ? "video" : "file";
    const m: Media = { id: uid(), item_kind: itemKind, item_id: itemId, kind, filename, mime: t, size: file.size, path: url, thumb_path: kind === "image" ? url : null, created_at: now() };
    media.set(m.id, m);
    return m;
  };
}
