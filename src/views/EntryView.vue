<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { api, type Media } from "../lib/api";
import { today } from "../lib/dates";
import { useAutosave } from "../lib/autosave";
import Icon from "../components/Icon.vue";
import MarkdownEditor from "../components/MarkdownEditor.vue";
import TagInput from "../components/TagInput.vue";
import MediaPanel from "../components/MediaPanel.vue";

const props = defineProps<{ id: string }>();
const router = useRouter();
const route = useRoute();

const id = ref(props.id === "new" ? crypto.randomUUID() : props.id);
const isNew = ref(props.id === "new" || route.query.new === "1");
if (props.id === "new") router.replace({ path: `/entry/${id.value}`, query: { new: "1" } });

const loaded = ref(isNew.value);
const exists = ref(!isNew.value);
const entryDate = ref(today());
const title = ref("");
const body = ref("");
const favorite = ref(false);
const tags = ref<string[]>([]);
const media = ref<Media[]>([]);
const allTags = ref<string[]>([]);
const armedDelete = ref(false);

const isEmpty = computed(() => !title.value.trim() && !body.value.trim() && !tags.value.length && !media.value.length);

async function persist() {
  if (!exists.value && isEmpty.value) return; // don't create blank entries
  await api.saveEntry({ id: id.value, entry_date: entryDate.value, title: title.value, body: body.value, favorite: favorite.value, tags: tags.value });
  exists.value = true;
}
const { status, error, touch, flush } = useAutosave(persist);

async function ensureSaved() {
  if (!exists.value) {
    await api.saveEntry({ id: id.value, entry_date: entryDate.value, title: title.value, body: body.value, favorite: favorite.value, tags: tags.value });
    exists.value = true;
  }
  await flush();
  return id.value;
}

onMounted(async () => {
  api.listTags().then((t) => (allTags.value = t.map((x) => x.name)));
  if (isNew.value && !exists.value) {
    try {
      // Route was reloaded after a save: pick up what's there.
      await load();
    } catch {
      /* genuinely new */
    }
    return;
  }
  await load();
});

async function load() {
  const e = await api.getEntry(id.value);
  entryDate.value = e.entry_date;
  title.value = e.title;
  body.value = e.body;
  favorite.value = e.favorite;
  tags.value = e.tags;
  media.value = e.media;
  exists.value = true;
  loaded.value = true;
}

async function remove() {
  if (!armedDelete.value) {
    armedDelete.value = true;
    setTimeout(() => (armedDelete.value = false), 3000);
    return;
  }
  if (exists.value) await api.trash("entry", id.value);
  exists.value = false;
  title.value = body.value = "";
  tags.value = [];
  media.value = [];
  router.replace("/journal");
}

async function back() {
  await flush();
  if (window.history.state?.back) router.back();
  else router.replace("/journal");
}

const statusText = computed(
  () => ({ idle: "", pending: "Editing…", saving: "Saving…", saved: "Saved", error: "Not saved" })[status.value],
);
</script>

<template>
  <div class="page editor-page">
    <header class="editor-head">
      <button class="icon-btn" aria-label="Back" @click="back"><Icon name="back" /></button>
      <input v-model="entryDate" type="date" class="date-input" aria-label="Entry date" @change="touch" />
      <span class="save-status" :class="status" :title="error">{{ statusText }}</span>
      <span class="spacer" />
      <button class="icon-btn star" :class="{ on: favorite }" :aria-label="favorite ? 'Unfavorite' : 'Favorite'"
        @click="(favorite = !favorite), touch()"><Icon name="star" /></button>
      <button class="icon-btn" :class="{ armed: armedDelete }" aria-label="Move to trash" @click="remove">
        <template v-if="armedDelete">Trash?</template><Icon v-else name="trash" />
      </button>
    </header>

    <div v-if="loaded" class="editor-body">
      <input v-model="title" class="title-input" placeholder="Title (optional)" @input="touch" />
      <MarkdownEditor v-model="body" placeholder="What's on your mind?" :autofocus="isNew" @update:model-value="touch" />
      <TagInput v-model="tags" :suggestions="allTags" @update:model-value="touch" />
      <MediaPanel item-kind="entry" :media="media" :ensure-saved="ensureSaved"
        @added="(m) => media.push(m)" @removed="(mid) => (media = media.filter((m) => m.id !== mid))" />
    </div>
  </div>
</template>
