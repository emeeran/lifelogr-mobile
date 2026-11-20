<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import { api, fileUrl, type Media } from "../lib/api";
import { bytes } from "../lib/dates";
import Icon from "./Icon.vue";

const props = defineProps<{
  media: Media[];
  itemKind: "entry" | "note";
  /** Makes sure the parent item exists in the DB and returns its id. */
  ensureSaved: () => Promise<string>;
}>();
const emit = defineEmits<{ added: [Media]; removed: [string] }>();

const fileInput = ref<HTMLInputElement>();
const busy = ref(false);
const error = ref("");
const viewing = ref<Media | null>(null);

async function upload(files: FileList | null) {
  if (!files?.length) return;
  busy.value = true;
  error.value = "";
  try {
    const id = await props.ensureSaved();
    // One at a time keeps peak memory to a single file.
    for (const f of Array.from(files)) emit("added", await api.addMedia(props.itemKind, id, f, f.name || "attachment"));
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
    if (fileInput.value) fileInput.value.value = "";
  }
}

// Two-tap delete (native confirm() dialogs are unreliable inside mobile webviews).
const armed = ref<string | null>(null);
let armTimer: number | undefined;
async function remove(m: Media) {
  if (armed.value !== m.id) {
    armed.value = m.id;
    clearTimeout(armTimer);
    armTimer = window.setTimeout(() => (armed.value = null), 3000);
    return;
  }
  armed.value = null;
  await api.deleteMedia(m.id);
  emit("removed", m.id);
}

// ----- voice recording -----
const recording = ref(false);
const elapsed = ref(0);
let rec: MediaRecorder | null = null;
let chunks: Blob[] = [];
let timer: number | undefined;
let stream: MediaStream | null = null;

function pickMime() {
  for (const t of ["audio/webm;codecs=opus", "audio/mp4", "audio/ogg;codecs=opus", "audio/webm"]) {
    if (typeof MediaRecorder !== "undefined" && MediaRecorder.isTypeSupported(t)) return t;
  }
  return "";
}

async function startRec() {
  error.value = "";
  try {
    stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  } catch (e) {
    error.value = "Microphone unavailable: " + (e as Error).message;
    return;
  }
  const mime = pickMime();
  rec = new MediaRecorder(stream, mime ? { mimeType: mime, audioBitsPerSecond: 48_000 } : undefined);
  chunks = [];
  rec.ondataavailable = (e) => e.data.size && chunks.push(e.data);
  rec.onstop = async () => {
    stream?.getTracks().forEach((t) => t.stop());
    const type = (rec?.mimeType || mime || "audio/webm").split(";")[0];
    const blob = new Blob(chunks, { type });
    chunks = [];
    const ext = type.includes("mp4") ? "m4a" : type.includes("ogg") ? "ogg" : "webm";
    const stamp = new Date().toISOString().slice(0, 16).replace("T", " ").replace(":", "-");
    const f = new File([blob], `Voice ${stamp}.${ext}`, { type });
    const dt = new DataTransfer();
    dt.items.add(f);
    await upload(dt.files);
  };
  rec.start(1000);
  recording.value = true;
  elapsed.value = 0;
  timer = window.setInterval(() => elapsed.value++, 1000);
}

function stopRec() {
  clearInterval(timer);
  recording.value = false;
  if (rec?.state !== "inactive") rec?.stop();
}
onBeforeUnmount(() => recording.value && stopRec());

const mmss = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
</script>

<template>
  <section class="media-panel">
    <div class="media-actions">
      <button type="button" class="btn ghost" :disabled="busy || recording" @click="fileInput?.click()">
        <Icon name="image" :size="18" /> Photo / video
      </button>
      <input ref="fileInput" type="file" accept="image/*,video/*,audio/*" multiple hidden @change="upload(($event.target as HTMLInputElement).files)" />
      <button v-if="!recording" type="button" class="btn ghost" :disabled="busy" @click="startRec">
        <Icon name="mic" :size="18" /> Voice note
      </button>
      <button v-else type="button" class="btn danger" @click="stopRec">
        <span class="rec-dot" /> {{ mmss(elapsed) }} · Stop
      </button>
      <span v-if="busy" class="muted small">Saving…</span>
    </div>
    <p v-if="error" class="error small">{{ error }}</p>

    <div v-if="media.some((m) => m.kind === 'image' || m.kind === 'video')" class="gallery">
      <div v-for="m in media.filter((m) => m.kind === 'image' || m.kind === 'video')" :key="m.id" class="tile">
        <img v-if="m.kind === 'image'" :src="fileUrl(m.thumb_path || m.path)" :alt="m.filename" loading="lazy" decoding="async" @click="viewing = m" />
        <video v-else :src="fileUrl(m.path)" preload="metadata" controls playsinline />
        <button type="button" class="tile-x" :class="{ armed: armed === m.id }" aria-label="Delete attachment" @click="remove(m)">
          <template v-if="armed === m.id">Delete?</template><Icon v-else name="x" :size="16" />
        </button>
      </div>
    </div>

    <ul v-if="media.some((m) => m.kind === 'audio' || m.kind === 'file')" class="audio-list">
      <li v-for="m in media.filter((m) => m.kind === 'audio' || m.kind === 'file')" :key="m.id">
        <audio v-if="m.kind === 'audio'" :src="fileUrl(m.path)" controls preload="none" />
        <span v-else>{{ m.filename }}</span>
        <span class="muted small">{{ bytes(m.size) }}</span>
        <button type="button" class="icon-btn" :class="{ armed: armed === m.id }" aria-label="Delete attachment" @click="remove(m)">
          <template v-if="armed === m.id">Delete?</template><Icon v-else name="trash" :size="18" />
        </button>
      </li>
    </ul>

    <div v-if="viewing" class="lightbox" @click="viewing = null">
      <img :src="fileUrl(viewing.path)" :alt="viewing.filename" />
    </div>
  </section>
</template>
