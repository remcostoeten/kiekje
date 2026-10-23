<script setup lang="ts">
import { withBase } from "vitepress";
import { data as releases } from "./releases.data";

function formatDate(date: string): string {
  if (date === "") {
    return "";
  }
  return new Date(`${date}T00:00:00Z`).toLocaleDateString("en-GB", {
    day: "numeric",
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  });
}
</script>

<template>
  <ol class="release-list">
    <li v-for="release in releases" :key="release.version" class="release">
      <div class="release-meta">
        <a class="release-version" :href="withBase(release.url)">{{ release.version }}</a>
        <time v-if="release.date" class="release-date" :datetime="release.date">{{
          formatDate(release.date)
        }}</time>
      </div>
      <div class="release-summary" v-html="release.summary"></div>
    </li>
  </ol>
</template>

<style scoped>
.release-list {
  list-style: none;
  margin: 24px 0 0;
  padding: 0;
  border-top: 1px solid var(--vp-c-divider);
}

.release {
  display: grid;
  grid-template-columns: 220px 1fr;
  gap: 8px 24px;
  padding: 20px 0;
  border-bottom: 1px solid var(--vp-c-divider);
}

.release-meta {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.release-version {
  font-family: var(--vp-font-family-mono);
  font-size: 15px;
  font-weight: 500;
  color: var(--vp-c-text-1);
  text-decoration: none;
}

.release-version:hover {
  color: var(--vp-c-brand-1);
}

.release-date {
  font-family: var(--vp-font-family-mono);
  font-size: 11px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--vp-c-text-2);
}

.release-summary {
  line-height: 1.6;
  color: var(--vp-c-text-1);
}

.release-summary :deep(p) {
  margin: 0;
}

@media (max-width: 640px) {
  .release {
    grid-template-columns: 1fr;
  }
}
</style>
