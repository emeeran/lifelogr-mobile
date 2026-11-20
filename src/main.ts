import { createApp } from "vue";
import { createRouter, createWebHashHistory } from "vue-router";
import App from "./App.vue";
import "./style.css";
import "./lib/theme";

const routes = [
  { path: "/", redirect: "/journal" },
  { path: "/journal", component: () => import("./views/JournalView.vue") },
  { path: "/entry/:id", component: () => import("./views/EntryView.vue"), props: true },
  { path: "/notes", component: () => import("./views/NotesView.vue") },
  { path: "/note/:id", component: () => import("./views/NoteView.vue"), props: true },
  { path: "/search", component: () => import("./views/SearchView.vue") },
  { path: "/reminders", component: () => import("./views/RemindersView.vue") },
  { path: "/settings", component: () => import("./views/SettingsView.vue") },
  { path: "/trash", component: () => import("./views/TrashView.vue") },
];

async function boot() {
  if (!("__TAURI_INTERNALS__" in window)) {
    const { installMock } = await import("./lib/mock");
    installMock();
  }
  const router = createRouter({ history: createWebHashHistory(), routes });
  createApp(App).use(router).mount("#app");
  const { startReminderLoop } = await import("./lib/reminders");
  startReminderLoop();
}
boot();
