<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { api, type TrashItem } from "../lib/api";
import { relTime } from "../lib/dates";
import Icon from "../components/Icon.vue";

const router = useRouter();
const items = ref<TrashItem[]>([]);
const armed = ref<string | null>(null);
const load = async () => (items.value = await api.listTrash());
onMounted(load);

async function restore(i: TrashItem) {
  await api.restore(i.kind, i.id);
  await load();
}
async function purge(i: TrashItem) {
  if (armed.value !== i.id) {
    armed.value = i.id;
    setTimeout(() => armed.value === i.id && (armed.value = null), 3000);
    return;
  }
  await api.purge(i.kind, i.id);
  await load();
}
</script>

<template>
  <div class="page">
    <header class="page-head">
      <button class="icon-btn" aria-label="Back" @click="router.push('/settings')"><Icon name="back" /></button>
      <h1>Trash</h1>
    </header>
    <p class="muted small">Deleted entries and notes stay here until you delete them for good.</p>
    <div v-if="!items.length" class="empty"><Icon name="trash" :size="40" /><p>Trash is empty.</p></div>
    <ul class="trash-list">
      <li v-for="i in items" :key="i.kind + i.id" class="card">
        <Icon :name="i.kind === 'entry' ? 'book' : 'note'" :size="18" />
        <span class="grow">{{ i.title }} <span class="muted small">· deleted {{ relTime(i.deleted_at) }}</span></span>
        <button class="btn ghost small" @click="restore(i)"><Icon name="restore" :size="16" /> Restore</button>
        <button class="btn ghost small danger-text" @click="purge(i)">{{ armed === i.id ? "Delete forever?" : "Delete" }}</button>
      </li>
    </ul>
  </div>
</template>
