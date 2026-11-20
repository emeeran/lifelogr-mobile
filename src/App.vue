<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import Icon from "./components/Icon.vue";
import { isTauri } from "./lib/api";

const route = useRoute();
const tabs = [
  { to: "/journal", label: "Journal", icon: "book", match: ["/journal", "/entry"] },
  { to: "/notes", label: "Notes", icon: "note", match: ["/notes", "/note"] },
  { to: "/search", label: "Search", icon: "search", match: ["/search"] },
  { to: "/reminders", label: "Reminders", icon: "bell", match: ["/reminders"] },
  { to: "/settings", label: "Settings", icon: "gear", match: ["/settings", "/trash"] },
];
const active = (m: string[]) => m.some((p) => route.path.startsWith(p));
// Editors take the whole screen on phones; the tab bar comes back on list screens.
const inEditor = computed(() => /^\/(entry|note)\//.test(route.path));
const preview = !isTauri();
</script>

<template>
  <div class="shell" :class="{ 'in-editor': inEditor }">
    <nav class="sidebar" aria-label="Main">
      <div class="brand"><img src="/icon.png" alt="" width="28" height="28" /> LifeLogr</div>
      <RouterLink v-for="t in tabs" :key="t.to" :to="t.to" class="nav-item" :class="{ active: active(t.match) }">
        <Icon :name="t.icon" /> <span>{{ t.label }}</span>
      </RouterLink>
      <div v-if="preview" class="preview-badge" title="Running in a plain browser with sample data. Use `npm run tauri dev` for the real app.">Browser preview</div>
    </nav>
    <main class="content">
      <RouterView v-slot="{ Component }">
        <KeepAlive include="JournalView,NotesView">
          <component :is="Component" :key="route.path" />
        </KeepAlive>
      </RouterView>
    </main>
    <nav class="tabbar" aria-label="Main">
      <RouterLink v-for="t in tabs" :key="t.to" :to="t.to" class="tab" :class="{ active: active(t.match) }">
        <Icon :name="t.icon" /> <span>{{ t.label }}</span>
      </RouterLink>
    </nav>
  </div>
</template>
