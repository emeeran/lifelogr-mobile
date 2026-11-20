<script setup lang="ts">
import { computed, ref } from "vue";

const props = defineProps<{ modelValue: string[]; suggestions?: string[] }>();
const emit = defineEmits<{ "update:modelValue": [string[]] }>();
const draft = ref("");

const matches = computed(() => {
  const q = draft.value.replace(/^#/, "").toLowerCase();
  if (!q) return [];
  return (props.suggestions ?? [])
    .filter((s) => s.toLowerCase().startsWith(q) && !props.modelValue.some((t) => t.toLowerCase() === s.toLowerCase()))
    .slice(0, 5);
});

function add(raw = draft.value) {
  const t = raw.trim().replace(/^#/, "").replace(/,$/, "").trim();
  if (t && !props.modelValue.some((x) => x.toLowerCase() === t.toLowerCase())) {
    emit("update:modelValue", [...props.modelValue, t]);
  }
  draft.value = "";
}
function remove(t: string) {
  emit("update:modelValue", props.modelValue.filter((x) => x !== t));
}
function onKey(e: KeyboardEvent) {
  if (e.key === "Enter" || e.key === ",") {
    e.preventDefault();
    add();
  } else if (e.key === "Backspace" && !draft.value && props.modelValue.length) {
    remove(props.modelValue[props.modelValue.length - 1]);
  }
}
</script>

<template>
  <div class="tag-input">
    <span v-for="t in modelValue" :key="t" class="chip">
      #{{ t }}
      <button type="button" class="chip-x" :aria-label="`Remove tag ${t}`" @click="remove(t)">×</button>
    </span>
    <input v-model="draft" placeholder="Add tag…" enterkeyhint="done" autocapitalize="off" @keydown="onKey" @blur="add()" />
    <div v-if="matches.length" class="tag-suggest">
      <button v-for="m in matches" :key="m" type="button" class="chip" @mousedown.prevent="add(m)">#{{ m }}</button>
    </div>
  </div>
</template>
