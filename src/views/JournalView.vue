<script setup lang="ts">
import { computed, onActivated, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { api, fileUrl, type EntrySummary, type Tag } from "../lib/api";
import { dayLabel, monthLabel } from "../lib/dates";
import Icon from "../components/Icon.vue";

defineOptions({ name: "JournalView" });

const router = useRouter();
const PAGE = 40;
const entries = ref<EntrySummary[]>([]);
const tags = ref<Tag[]>([]);
const filterTag = ref<string | null>(null);
const favOnly = ref(false);
const more = ref(false);
const loading = ref(true);

async function load(reset = true) {
  const offset = reset ? 0 : entries.value.length;
  const page = await api.listEntries({ tag: filterTag.value, favorites_only: favOnly.value, limit: PAGE, offset });
  entries.value = reset ? page : [...entries.value, ...page];
  more.value = page.length === PAGE;
  loading.value = false;
}
async function refresh() {
  await Promise.all([load(true), api.listTags().then((t) => (tags.value = t))]);
}
watch([filterTag, favOnly], () => load(true));
onActivated(refresh); // KeepAlive: also runs on first mount

const groups = computed(() => {
  const out: { month: string; days: { date: string; items: EntrySummary[] }[] }[] = [];
  for (const e of entries.value) {
    const month = monthLabel(e.entry_date);
    let g = out[out.length - 1];
    if (!g || g.month !== month) out.push((g = { month, days: [] }));
    let d = g.days[g.days.length - 1];
    if (!d || d.date !== e.entry_date) g.days.push((d = { date: e.entry_date, items: [] }));
    d.items.push(e);
  }
  return out;
});

async function toggleFav(e: EntrySummary) {
  e.favorite = !e.favorite;
  await api.setFavorite(e.id, e.favorite);
}
</script>

<template>
  <div class="page">
    <header class="page-head">
      <h1>Journal</h1>
      <button class="btn primary hide-mobile" @click="router.push('/entry/new')"><Icon name="plus" :size="18" /> New entry</button>
    </header>

    <div class="filters" role="group" aria-label="Filter entries">
      <button class="chip" :class="{ on: !filterTag && !favOnly }" @click="(filterTag = null), (favOnly = false)">All</button>
      <button class="chip" :class="{ on: favOnly }" @click="favOnly = !favOnly">★ Favorites</button>
      <button v-for="t in tags" :key="t.name" class="chip" :class="{ on: filterTag === t.name }"
        @click="filterTag = filterTag === t.name ? null : t.name">#{{ t.name }}</button>
    </div>

    <div v-if="!loading && !entries.length" class="empty">
      <Icon name="book" :size="40" />
      <p v-if="filterTag || favOnly">Nothing matches this filter.</p>
      <template v-else>
        <p>Your journal is empty. What happened today?</p>
        <button class="btn primary" @click="router.push('/entry/new')">Write the first entry</button>
      </template>
    </div>

    <section v-for="g in groups" :key="g.month" class="month">
      <h2 class="month-label">{{ g.month }}</h2>
      <div v-for="d in g.days" :key="d.date" class="day">
        <div class="day-label">{{ dayLabel(d.date) }}</div>
        <article v-for="e in d.items" :key="e.id" class="card entry-card" tabindex="0"
          @click="router.push(`/entry/${e.id}`)" @keydown.enter="router.push(`/entry/${e.id}`)">
          <img v-if="e.cover_thumb" class="cover" :src="fileUrl(e.cover_thumb)" alt="" loading="lazy" decoding="async" />
          <div class="card-body">
            <h3 v-if="e.title">{{ e.title }}</h3>
            <p class="excerpt">{{ e.excerpt || (e.media_count ? "" : "Empty entry") }}</p>
            <div class="meta">
              <span v-for="t in e.tags" :key="t" class="tag">#{{ t }}</span>
              <span v-if="e.media_count" class="muted small"><Icon name="paperclip" :size="14" /> {{ e.media_count }}</span>
            </div>
          </div>
          <button class="icon-btn star" :class="{ on: e.favorite }" :aria-label="e.favorite ? 'Unfavorite' : 'Favorite'" @click.stop="toggleFav(e)">
            <Icon name="star" :size="18" />
          </button>
        </article>
      </div>
    </section>
    <button v-if="more" class="btn ghost load-more" @click="load(false)">Load more</button>

    <button class="fab" aria-label="New entry" @click="router.push('/entry/new')"><Icon name="plus" :size="26" /></button>
  </div>
</template>
