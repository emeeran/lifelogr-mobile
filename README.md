# LifeLogr

A lightweight, local-first journal and notes app for **desktop (Linux, Windows, macOS) and Android**, from one codebase.

Your journal lives on your device in a single SQLite file plus a media folder. No accounts, no telemetry, no network calls.

## Features

| | |
|---|---|
| **Journal** | Entries by date, grouped by month and day. Markdown with a formatting toolbar (works without hotkeys on phones), favorites, tag chips, tag and favorite filters. |
| **Notes** | Nested folders (with a guard against moving a folder inside itself), pinned notes, `#hashtags` written in the text become tags, inline `#` autocomplete. |
| **Media** | Photos and videos (camera or library), in-app voice notes, audio files. Images get 480px JPEG thumbnails so lists and galleries stay light on memory. |
| **Search** | Full-text search over entries and notes with SQLite FTS5 (prefix matching, highlighted snippets, diacritic-insensitive). |
| **Reminders** | One-off, daily, weekly or monthly. Android hands them to the OS so they fire with the app closed. Desktop fires them while the app runs and catches up on missed ones at launch. |
| **Safety** | Autosave (debounced, and flushed when the app goes to the background). Deletes go to a restorable Trash. Media files are only removed after the database commit succeeds. |
| **Settings** | Light, dark or system theme, text size, storage stats. |

## Stack and why

- **Tauri v2** shell: uses the system webview, so no bundled Chromium (unlike Electron). The same app builds for desktop and Android.
- **Rust core** (`src-tauri/src/store.rs`): SQLite through `rusqlite` with a bundled SQLite build that includes FTS5, so search behaves the same on every platform. WAL mode and a ~4 MB page cache. No Python and no sidecar process.
- **Vue 3 + TypeScript** UI, no CSS framework, hand-written inline icons. About 45 KB gzipped JS for the initial load; markdown rendering is loaded lazily.
- Release profile is tuned for size (`opt-level = "s"`, LTO, stripped).

## Develop

Prerequisites: Node 20+, Rust (stable), and the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
npm install
npm run tauri dev        # desktop app with hot reload
npm test                 # Rust store tests
npm run dev              # UI only, in a browser, with an in-memory sample backend
```

`npm run dev` on its own is handy for UI work. It detects that it isn't running inside Tauri and swaps in `src/lib/mock.ts`, so nothing is persisted.

### Android

Requires Android Studio (SDK + NDK) and `JAVA_HOME`/`ANDROID_HOME`/`NDK_HOME` set, as in the [Tauri Android guide](https://v2.tauri.app/start/prerequisites/#android).

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
npm run tauri android init
# Voice notes and camera capture need these in src-tauri/gen/android/app/src/main/AndroidManifest.xml:
#   <uses-permission android:name="android.permission.RECORD_AUDIO" />
#   <uses-permission android:name="android.permission.CAMERA" />
npm run tauri android dev        # run on a device or emulator
npm run tauri android build --apk
```

The CI workflow (`.github/workflows/build.yml`) runs the tests, builds Linux packages, and builds a debug Android APK on every push, applying the permissions above automatically.

## Where data lives

The app data directory for the `app.lifelogr` identifier, e.g. `~/.local/share/app.lifelogr/` on Linux or `%APPDATA%\app.lifelogr\` on Windows. Settings → Your data shows the exact path.

```
lifelogr.db          SQLite (entries, notes, folders, tags, media index, reminders, FTS index)
media/ab/<uuid>.jpg  attachments, sharded by the first two id characters
media/ab/<uuid>.thumb.jpg
```

## Project layout

```
src-tauri/src/store.rs   schema, queries, media handling, tests
src-tauri/src/lib.rs     Tauri commands (thin wrappers over Store)
src/lib/api.ts           typed client for the commands; the only thing the UI talks to
src/lib/reminders.ts     reminder scheduling and catch-up
src/views/*.vue          screens: Journal, Entry, Notes, Note, Search, Reminders, Settings, Trash
src/components/*.vue     MarkdownEditor, MediaPanel (uploads + voice), TagInput, Icon
```

## Assumptions (to revisit with the PRD and reference app)

This first version was built from `PRD-mobile.md` without access to the existing LifeLogr desktop codebase. Where they differ, the PRD and the existing app should win. Decisions made so far:

1. **New standalone app, not a port.** The PRD describes a Vue + FastAPI app with a Python sidecar and plans a phone app that syncs to it. Since the goal here was a lightweight app for desktop and Android, this goes straight to the PRD's long-term "Rust core embedded everywhere" option (§5.1 option C) instead of a Python backend.
2. **Entries use explicit tags; notes use `#hashtags` in the text**, matching the PRD's FR-6 and FR-9.
3. **Reminders use simple repeats** (once, daily, weekly, monthly) instead of cron expressions.
4. **Tabbed pages within a note** (FR-10) are not built yet. A note is a single markdown page.

### Not built yet (PRD items)

- Per-item AES-256-GCM encryption at rest and the app lock (FR-25, FR-26).
- Pairing and end-to-end-encrypted sync between desktop and phone (FR-17 to FR-24). Records already use UUIDs, `updated_at` and soft-delete timestamps, which is what the planned sync needs.
- Import/export and backups, share-sheet target and home-screen widget (FR-30), on-device speech-to-text, iOS.

## License

Not chosen yet.
