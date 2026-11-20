<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api, type Reminder, type Repeat } from "../lib/api";
import { toLocalInput } from "../lib/dates";
import { afterReminderChange } from "../lib/reminders";
import Icon from "../components/Icon.vue";

const list = ref<Reminder[]>([]);
const form = ref<null | { id?: string; title: string; body: string; when: string; repeat: Repeat; enabled: boolean }>(null);
const error = ref("");

async function load() {
  list.value = await api.listReminders();
}
onMounted(load);

function startNew() {
  const d = new Date();
  d.setHours(21, 0, 0, 0);
  if (d < new Date()) d.setDate(d.getDate() + 1);
  form.value = { title: "Write in my journal", body: "", when: toLocalInput(d.toISOString()), repeat: "daily", enabled: true };
}
function edit(r: Reminder) {
  form.value = { id: r.id, title: r.title, body: r.body, when: toLocalInput(r.next_at), repeat: r.repeat, enabled: r.enabled };
}

async function save() {
  if (!form.value) return;
  error.value = "";
  try {
    const f = form.value;
    await api.saveReminder({ id: f.id, title: f.title, body: f.body, next_at: new Date(f.when).toISOString(), repeat: f.repeat, enabled: f.enabled });
    form.value = null;
    await load();
    await afterReminderChange();
  } catch (e) {
    error.value = String(e);
  }
}
async function toggle(r: Reminder) {
  await api.saveReminder({ ...r, enabled: !r.enabled });
  await load();
  await afterReminderChange();
}
async function remove(id: string) {
  await api.deleteReminder(id);
  form.value = null;
  await load();
  await afterReminderChange();
}

const fmt = (iso: string) =>
  new Date(iso).toLocaleString(undefined, { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
const repeatLabel: Record<Repeat, string> = { none: "Once", daily: "Every day", weekly: "Every week", monthly: "Every month" };
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h1>Reminders</h1>
      <button class="btn primary" @click="startNew"><Icon name="plus" :size="18" /> New</button>
    </header>

    <form v-if="form" class="card reminder-form" @submit.prevent="save">
      <label>Title <input v-model="form.title" required /></label>
      <label>Note <input v-model="form.body" placeholder="Optional" /></label>
      <div class="row">
        <label>When <input v-model="form.when" type="datetime-local" required /></label>
        <label>Repeat
          <select v-model="form.repeat">
            <option v-for="(l, k) in repeatLabel" :key="k" :value="k">{{ l }}</option>
          </select>
        </label>
      </div>
      <p v-if="error" class="error small">{{ error }}</p>
      <div class="row">
        <button class="btn primary" type="submit">Save</button>
        <button class="btn ghost" type="button" @click="form = null">Cancel</button>
        <span class="spacer" />
        <button v-if="form.id" class="btn ghost danger-text" type="button" @click="remove(form.id!)">Delete</button>
      </div>
    </form>

    <div v-if="!list.length && !form" class="empty">
      <Icon name="bell" :size="40" />
      <p>No reminders. A daily nudge helps the habit stick.</p>
      <button class="btn primary" @click="startNew">Add a reminder</button>
    </div>

    <ul class="reminder-list">
      <li v-for="r in list" :key="r.id" class="card reminder" :class="{ off: !r.enabled }">
        <button class="reminder-main" @click="edit(r)">
          <strong>{{ r.title }}</strong>
          <span class="muted small">{{ r.enabled ? fmt(r.next_at) : "Off" }} · {{ repeatLabel[r.repeat] }}</span>
        </button>
        <label class="switch" :aria-label="r.enabled ? 'Turn off' : 'Turn on'">
          <input type="checkbox" :checked="r.enabled" @change="toggle(r)" /><span />
        </label>
      </li>
    </ul>
    <p class="muted small note">Reminders are stored on this device. On Android they fire even when the app is closed; on desktop they fire while LifeLogr is running (missed ones show on next launch).</p>
  </div>
</template>
