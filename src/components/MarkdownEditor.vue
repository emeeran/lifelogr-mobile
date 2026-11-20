<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { renderMarkdown } from "../lib/markdown";
import Icon from "./Icon.vue";

const props = defineProps<{
  modelValue: string;
  placeholder?: string;
  /** Known tags, for inline `#` autocomplete. */
  tagSuggestions?: string[];
  autofocus?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [string]; tag: [string] }>();

const ta = ref<HTMLTextAreaElement>();
const preview = ref(false);
const html = computed(() => (preview.value ? renderMarkdown(props.modelValue) : ""));

function grow() {
  const el = ta.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = el.scrollHeight + "px";
}
watch(() => props.modelValue, () => nextTick(grow));
onMounted(() => {
  grow();
  if (props.autofocus) ta.value?.focus();
});

function input(e: Event) {
  emit("update:modelValue", (e.target as HTMLTextAreaElement).value);
  updateHashQuery();
}

/** Wrap the selection (or insert a line prefix) – the mobile replacement for hotkeys. */
function apply(kind: "bold" | "italic" | "h" | "ul" | "todo" | "quote") {
  const el = ta.value!;
  const { selectionStart: s, selectionEnd: e, value: v } = el;
  let next: string, cs: number, ce: number;
  if (kind === "bold" || kind === "italic") {
    const m = kind === "bold" ? "**" : "_";
    next = v.slice(0, s) + m + v.slice(s, e) + m + v.slice(e);
    cs = s + m.length;
    ce = e + m.length;
  } else {
    const prefix = { h: "## ", ul: "- ", todo: "- [ ] ", quote: "> " }[kind];
    const ls = v.lastIndexOf("\n", s - 1) + 1;
    const has = v.startsWith(prefix, ls);
    next = has ? v.slice(0, ls) + v.slice(ls + prefix.length) : v.slice(0, ls) + prefix + v.slice(ls);
    const d = has ? -prefix.length : prefix.length;
    cs = s + d;
    ce = e + d;
  }
  emit("update:modelValue", next);
  nextTick(() => {
    el.focus();
    el.setSelectionRange(cs, ce);
  });
}

function onKey(e: KeyboardEvent) {
  if (hashMatches.value.length && (e.key === "Enter" || e.key === "Tab")) {
    e.preventDefault();
    completeTag(hashMatches.value[0]);
    return;
  }
  const mod = e.ctrlKey || e.metaKey;
  if (mod && e.key === "b") (e.preventDefault(), apply("bold"));
  if (mod && e.key === "i") (e.preventDefault(), apply("italic"));
  // Continue lists on Enter.
  if (e.key === "Enter" && !e.shiftKey) {
    const el = ta.value!;
    const v = el.value;
    const ls = v.lastIndexOf("\n", el.selectionStart - 1) + 1;
    const line = v.slice(ls, el.selectionStart);
    const m = line.match(/^(\s*)(- \[[ x]\] |- |\* |\d+\. |> )/);
    if (m) {
      e.preventDefault();
      if (line.trim() === m[2].trim()) {
        // Empty list item: end the list.
        emit("update:modelValue", v.slice(0, ls) + v.slice(el.selectionStart));
        nextTick(() => el.setSelectionRange(ls, ls));
        return;
      }
      let marker = m[2].replace("[x]", "[ ]");
      const num = marker.match(/^(\d+)\. $/);
      if (num) marker = `${Number(num[1]) + 1}. `;
      const ins = "\n" + m[1] + marker;
      const pos = el.selectionStart + ins.length;
      emit("update:modelValue", v.slice(0, el.selectionStart) + ins + v.slice(el.selectionEnd));
      nextTick(() => el.setSelectionRange(pos, pos));
    }
  }
}

// ----- inline #tag autocomplete -----
const hashQuery = ref<string | null>(null);
function updateHashQuery() {
  const el = ta.value;
  if (!el || !props.tagSuggestions) return;
  const before = el.value.slice(0, el.selectionStart);
  const m = before.match(/(?:^|\s)#([\p{L}\p{N}_\-/]*)$/u);
  hashQuery.value = m ? m[1] : null;
}
const hashMatches = computed(() => {
  const q = hashQuery.value;
  if (q === null || !props.tagSuggestions) return [];
  const lq = q.toLowerCase();
  return props.tagSuggestions.filter((t) => t.toLowerCase().startsWith(lq) && t.toLowerCase() !== lq).slice(0, 6);
});
function completeTag(tag: string) {
  const el = ta.value!;
  const pos = el.selectionStart;
  const start = pos - (hashQuery.value?.length ?? 0);
  const v = el.value.slice(0, start) + tag + " " + el.value.slice(pos);
  emit("update:modelValue", v);
  hashQuery.value = null;
  const np = start + tag.length + 1;
  nextTick(() => {
    el.focus();
    el.setSelectionRange(np, np);
  });
}

function previewClick(e: MouseEvent) {
  const a = (e.target as HTMLElement).closest("a.hashtag") as HTMLElement | null;
  if (a?.dataset.tag) emit("tag", a.dataset.tag);
}

defineExpose({ focus: () => ta.value?.focus() });
</script>

<template>
  <div class="md-editor">
    <div class="md-toolbar" role="toolbar" aria-label="Formatting">
      <template v-if="!preview">
        <button type="button" title="Bold (Ctrl+B)" @mousedown.prevent="apply('bold')"><b>B</b></button>
        <button type="button" title="Italic (Ctrl+I)" @mousedown.prevent="apply('italic')"><i>I</i></button>
        <button type="button" title="Heading" @mousedown.prevent="apply('h')">H</button>
        <button type="button" title="Bulleted list" @mousedown.prevent="apply('ul')">•</button>
        <button type="button" title="Checklist" @mousedown.prevent="apply('todo')">☐</button>
        <button type="button" title="Quote" @mousedown.prevent="apply('quote')">❝</button>
      </template>
      <span class="spacer" />
      <button type="button" :class="{ on: preview }" :title="preview ? 'Edit' : 'Preview'" @click="preview = !preview">
        <Icon :name="preview ? 'edit' : 'eye'" :size="18" />
      </button>
    </div>
    <div v-if="preview" class="md-preview prose" @click="previewClick" v-html="html || '<p class=muted>Nothing written yet.</p>'" />
    <div v-else class="md-write">
      <textarea ref="ta" :value="modelValue" :placeholder="placeholder" rows="8" spellcheck="true"
        @input="input" @keydown="onKey" @click="updateHashQuery" @blur="hashQuery = null" />
      <div v-if="hashMatches.length" class="hash-suggest">
        <button v-for="t in hashMatches" :key="t" type="button" class="chip" @mousedown.prevent="completeTag(t)">#{{ t }}</button>
      </div>
    </div>
  </div>
</template>
