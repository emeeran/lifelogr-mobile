<script setup lang="ts">
import { computed, onActivated, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { api, type Folder, type NoteSummary } from "../lib/api";
import { relTime } from "../lib/dates";
import { descendants, flattenFolders } from "../lib/folders";
import Icon from "../components/Icon.vue";

defineOptions({ name: "NotesView" });

const router = useRouter();
const route = useRoute();
const folders = ref<Folder[]>([]);
const notes = ref<NoteSummary[]>([]);
/** null = all notes, "" = unfiled, otherwise a folder id */
const current = ref<string | null>(null);
const tag = ref<string | null>(null);
const editing = ref<null | { id?: string; name: string; parent_id: string | null }>(null);
const error = ref("");
const armedDelete = ref(false);

const tree = computed(() => flattenFolders(folders.value));
const currentFolder = computed(() => tree.value.find((f) => f.id === current.value));
const parentChoices = computed(() => {
  const banned = editing.value?.id ? descendants(folders.value, editing.value.id) : new Set<string>();
  return tree.value.filter((f) => !banned.has(f.id));
});

async function load() {
  [folders.value, notes.value] = await Promise.all([api.listFolders(), api.listNotes(current.value, tag.value)]);
}
watch([current, tag], load);
watch(
  () => route.query.tag,
  (t) => (tag.value = typeof t === "string" ? t : null),
  { immediate: true },
);
onActivated(load);

function newNote() {
  router.push({ path: "/note/new", query: current.value ? { folder: current.value } : {} });
}

async function saveFolder() {
  if (!editing.value) return;
  error.value = "";
  try {
    const id = await api.saveFolder({ ...editing.value, name: editing.value.name.trim() });
    editing.value = null;
    current.value = id;
    await load();
  } catch (e) {
    error.value = String(e);
  }
}

async function deleteFolder() {
  if (!currentFolder.value) return;
  if (!armedDelete.value) {
    armedDelete.value = true;
    setTimeout(() => (armedDelete.value = false), 3000);
    return;
  }
  armedDelete.value = false;
  const parent = currentFolder.value.parent_id;
  await api.deleteFolder(currentFolder.value.id);
  current.value = parent;
  await load();
}
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h1>Notes</h1>
      <button class="btn primary hide-mobile" @click="newNote"><Icon name="plus" :size="18" /> New note</button>
    </header>

    <div class="filters folders" role="group" aria-label="Folders">
      <button class="chip" :class="{ on: current === null }" @click="current = null">All</button>
      <button class="chip" :class="{ on: current === '' }" @click="current = ''">Unfiled</button>
      <button v-for="f in tree" :key="f.id" class="chip" :class="{ on: current === f.id }" :title="f.label" @click="current = f.id">
        <Icon name="folder" :size="14" /> {{ f.depth ? "↳ " : "" }}{{ f.name }} <span class="count">{{ f.note_count }}</span>
      </button>
      <button class="chip dashed" @click="editing = { name: '', parent_id: current || null }"><Icon name="plus" :size="14" /> Folder</button>
    </div>

    <div v-if="tag" class="active-filter">
      Tagged <span class="tag">#{{ tag }}</span>
      <button class="icon-btn" aria-label="Clear tag filter" @click="router.replace('/notes')"><Icon name="x" :size="16" /></button>
    </div>

    <div v-if="currentFolder && !editing" class="folder-bar">
      <strong>{{ currentFolder.label }}</strong>
      <span class="spacer" />
      <button class="btn ghost small" @click="editing = { id: currentFolder.id, name: currentFolder.name, parent_id: currentFolder.parent_id }">Rename / move</button>
      <button class="btn ghost small" :class="{ armed: armedDelete }" @click="deleteFolder">{{ armedDelete ? "Notes move up. Delete?" : "Delete folder" }}</button>
    </div>

    <form v-if="editing" class="folder-form card" @submit.prevent="saveFolder">
      <input v-model="editing.name" placeholder="Folder name" required autofocus />
      <select v-model="editing.parent_id" aria-label="Parent folder">
        <option :value="null">Top level</option>
        <option v-for="f in parentChoices" :key="f.id" :value="f.id">{{ f.label }}</option>
      </select>
      <button class="btn primary" type="submit">{{ editing.id ? "Save" : "Create" }}</button>
      <button class="btn ghost" type="button" @click="editing = null">Cancel</button>
      <p v-if="error" class="error small">{{ error }}</p>
    </form>

    <div v-if="!notes.length" class="empty">
      <Icon name="note" :size="40" />
      <p>No notes here yet.</p>
      <button class="btn primary" @click="newNote">New note</button>
    </div>

    <div class="note-grid">
      <article v-for="n in notes" :key="n.id" class="card note-card" tabindex="0"
        @click="router.push(`/note/${n.id}`)" @keydown.enter="router.push(`/note/${n.id}`)">
        <h3><Icon v-if="n.pinned" name="pin" :size="14" /> {{ n.title || "Untitled note" }}</h3>
        <p class="excerpt">{{ n.excerpt }}</p>
        <div class="meta">
          <span v-for="t in n.tags" :key="t" class="tag">#{{ t }}</span>
          <span class="muted small push">{{ relTime(n.updated_at) }}</span>
        </div>
      </article>
    </div>

    <button class="fab" aria-label="New note" @click="newNote"><Icon name="plus" :size="26" /></button>
  </div>
</template>
