<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { api, type Folder, type Media } from "../lib/api";
import { useAutosave } from "../lib/autosave";
import { flattenFolders } from "../lib/folders";
import Icon from "../components/Icon.vue";
import MarkdownEditor from "../components/MarkdownEditor.vue";
import MediaPanel from "../components/MediaPanel.vue";

const props = defineProps<{ id: string }>();
const router = useRouter();
const route = useRoute();

const startFolder = typeof route.query.folder === "string" ? route.query.folder : null;
const id = ref(props.id === "new" ? crypto.randomUUID() : props.id);
const isNew = props.id === "new" || route.query.new === "1";
if (props.id === "new") router.replace({ path: `/note/${id.value}`, query: { new: "1", ...(startFolder ? { folder: startFolder } : {}) } });

const loaded = ref(isNew);
const exists = ref(false);
const title = ref("");
const body = ref("");
const pinned = ref(false);
const folderId = ref<string | null>(startFolder);
const tags = ref<string[]>([]);
const media = ref<Media[]>([]);
const folders = ref<Folder[]>([]);
const allTags = ref<string[]>([]);
const armedDelete = ref(false);
const tree = computed(() => flattenFolders(folders.value));

const isEmpty = computed(() => !title.value.trim() && !body.value.trim() && !media.value.length);

async function write() {
  const n = await api.saveNote({ id: id.value, folder_id: folderId.value, title: title.value, body: body.value, pinned: pinned.value });
  tags.value = n.tags;
  exists.value = true;
}
const { status, error, touch, flush } = useAutosave(async () => {
  if (!exists.value && isEmpty.value) return;
  await write();
});

async function ensureSaved() {
  if (!exists.value) await write();
  await flush();
  return id.value;
}

onMounted(async () => {
  api.listFolders().then((f) => (folders.value = f));
  api.listTags().then((t) => (allTags.value = t.map((x) => x.name)));
  try {
    const n = await api.getNote(id.value);
    title.value = n.title;
    body.value = n.body;
    pinned.value = n.pinned;
    folderId.value = n.folder_id;
    tags.value = n.tags;
    media.value = n.media;
    exists.value = true;
  } catch (e) {
    if (!isNew) error.value = String(e);
  }
  loaded.value = true;
});

async function remove() {
  if (!armedDelete.value) {
    armedDelete.value = true;
    setTimeout(() => (armedDelete.value = false), 3000);
    return;
  }
  if (exists.value) await api.trash("note", id.value);
  exists.value = false;
  title.value = body.value = "";
  media.value = [];
  router.replace("/notes");
}

async function back() {
  await flush();
  if (window.history.state?.back) router.back();
  else router.replace("/notes");
}

async function openTag(t: string) {
  await flush();
  router.push({ path: "/notes", query: { tag: t } });
}

const statusText = computed(
  () => ({ idle: "", pending: "Editing…", saving: "Saving…", saved: "Saved", error: "Not saved" })[status.value],
);
</script>

<template>
  <div class="page editor-page">
    <header class="editor-head">
      <button class="icon-btn" aria-label="Back" @click="back"><Icon name="back" /></button>
      <select v-model="folderId" class="folder-select" aria-label="Folder" @change="touch">
        <option :value="null">Unfiled</option>
        <option v-for="f in tree" :key="f.id" :value="f.id">{{ f.label }}</option>
      </select>
      <span class="save-status" :class="status" :title="error">{{ statusText }}</span>
      <span class="spacer" />
      <button class="icon-btn star" :class="{ on: pinned }" :aria-label="pinned ? 'Unpin' : 'Pin'" @click="(pinned = !pinned), touch()">
        <Icon name="pin" />
      </button>
      <button class="icon-btn" :class="{ armed: armedDelete }" aria-label="Move to trash" @click="remove">
        <template v-if="armedDelete">Trash?</template><Icon v-else name="trash" />
      </button>
    </header>

    <div v-if="loaded" class="editor-body">
      <input v-model="title" class="title-input" placeholder="Title" @input="touch" />
      <MarkdownEditor v-model="body" placeholder="Write a note. Use #tags anywhere." :tag-suggestions="allTags"
        :autofocus="isNew" @update:model-value="touch" @tag="openTag" />
      <div v-if="tags.length" class="meta">
        <button v-for="t in tags" :key="t" class="tag" @click="openTag(t)">#{{ t }}</button>
      </div>
      <MediaPanel item-kind="note" :media="media" :ensure-saved="ensureSaved"
        @added="(m) => media.push(m)" @removed="(mid) => (media = media.filter((m) => m.id !== mid))" />
    </div>
  </div>
</template>
