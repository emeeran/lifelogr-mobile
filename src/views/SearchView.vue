<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { api, type SearchHit } from "../lib/api";
import { dayLabel } from "../lib/dates";
import { snippetParts } from "../lib/markdown";
import Icon from "../components/Icon.vue";

const router = useRouter();
const route = useRoute();
const q = ref(typeof route.query.q === "string" ? route.query.q : "");
const hits = ref<SearchHit[]>([]);
const searched = ref(false);
const input = ref<HTMLInputElement>();
let t: number | undefined;

watch(q, (v) => {
  clearTimeout(t);
  t = window.setTimeout(async () => {
    router.replace({ query: v ? { q: v } : {} });
    hits.value = v.trim() ? await api.search(v) : [];
    searched.value = !!v.trim();
  }, 150);
}, { immediate: true });
onMounted(() => input.value?.focus());

const open = (h: SearchHit) => router.push(h.kind === "entry" ? `/entry/${h.id}` : `/note/${h.id}`);
</script>

<template>
  <div class="page">
    <header class="page-head"><h1>Search</h1></header>
    <label class="search-box">
      <Icon name="search" :size="18" />
      <input ref="input" v-model="q" type="search" placeholder="Search entries and notes" enterkeyhint="search" />
    </label>
    <p v-if="searched && !hits.length" class="empty small">No matches for “{{ q }}”.</p>
    <ul class="hits">
      <li v-for="h in hits" :key="h.kind + h.id">
        <button class="card hit" @click="open(h)">
          <span class="hit-kind"><Icon :name="h.kind === 'entry' ? 'book' : 'note'" :size="16" /></span>
          <span class="hit-main">
            <strong>{{ h.title || (h.kind === "entry" ? dayLabel(h.date) : "Untitled note") }}</strong>
            <span class="snippet">
              <template v-for="(p, i) in snippetParts(h.snippet)" :key="i"><mark v-if="p.hit">{{ p.text }}</mark><template v-else>{{ p.text }}</template></template>
            </span>
          </span>
          <span class="muted small">{{ h.kind === "entry" ? dayLabel(h.date) : "Note" }}</span>
        </button>
      </li>
    </ul>
  </div>
</template>
