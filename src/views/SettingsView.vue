<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api, type Stats } from "../lib/api";
import { bytes } from "../lib/dates";
import { fontSize, theme } from "../lib/theme";
import Icon from "../components/Icon.vue";

const stats = ref<Stats | null>(null);
onMounted(async () => (stats.value = await api.stats()));
</script>

<template>
  <div class="page">
    <header class="page-head"><h1>Settings</h1></header>

    <section class="card settings">
      <h2>Appearance</h2>
      <div class="setting">
        <span>Theme</span>
        <div class="seg" role="radiogroup" aria-label="Theme">
          <button v-for="t in ['system', 'light', 'dark'] as const" :key="t" role="radio" :aria-checked="theme === t" :class="{ on: theme === t }" @click="theme = t">{{ t[0].toUpperCase() + t.slice(1) }}</button>
        </div>
      </div>
      <div class="setting">
        <span>Text size</span>
        <div class="seg" role="radiogroup" aria-label="Text size">
          <button v-for="f in ['small', 'medium', 'large'] as const" :key="f" role="radio" :aria-checked="fontSize === f" :class="{ on: fontSize === f }" @click="fontSize = f">{{ f[0].toUpperCase() + f.slice(1) }}</button>
        </div>
      </div>
    </section>

    <section class="card settings">
      <h2>Your data</h2>
      <p class="muted small">Everything stays on this device. LifeLogr makes no network requests.</p>
      <dl v-if="stats" class="stats">
        <div><dt>Entries</dt><dd>{{ stats.entries }}</dd></div>
        <div><dt>Notes</dt><dd>{{ stats.notes }}</dd></div>
        <div><dt>Attachments</dt><dd>{{ stats.media }} · {{ bytes(stats.media_bytes) }}</dd></div>
        <div><dt>Database</dt><dd>{{ bytes(stats.db_bytes) }}</dd></div>
      </dl>
      <p v-if="stats" class="muted small mono">{{ stats.data_dir }}</p>
      <RouterLink to="/trash" class="btn ghost"><Icon name="trash" :size="18" /> Trash</RouterLink>
    </section>

    <p class="muted small center">LifeLogr 0.1.0</p>
  </div>
</template>
