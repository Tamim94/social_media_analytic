<script setup lang="ts">
const links = [
  { label: 'GITHUB', url: 'https://github.com/Tamim94', hint: 'this repo + everything else I build' },
  { label: 'LINKEDIN', url: 'https://www.linkedin.com/in/tamim-golam', hint: 'the formal version' },
  { label: 'PORTFOLIO', url: 'https://tamimg-portfolio.web.app', hint: 'the other projects' },
]
</script>

<template>
  <div class="card" style="max-width: 760px">
    <h3>About this project <i>· built by Tamim GOLAM</i></h3>
    <p class="bio">
      wikistream is a live analytics pipeline over Wikimedia's global edit stream. A Rust daemon
      consumes every edit on every wiki — about 30 events per second, 2.6 million a day — and rolls
      it up inside a 500 MB free-tier Postgres: minute/hour/day buckets, HyperLogLog estimates of
      unique editors, a durable cursor for crash-safe resume, and a gap ledger that records lost
      data instead of hiding it. The raw stream is ~3.8 GB/day against that 500 MB budget, so every
      storage and retention decision is deliberate; the reasoning lives in the repo's ADRs.
    </p>
    <p class="bio" style="margin-top: 10px">
      The numbers on the dashboard are measured from the running system, the editor counts are
      labeled as estimates, and the daemon has been ingesting without supervision since
      August 2026. Everything runs on free tiers: Oracle Cloud (the daemon), Supabase (Postgres),
      Cloudflare (this page).
    </p>
    <div class="links">
      <a v-for="l in links" :key="l.label" :href="l.url" target="_blank" rel="noopener">
        <b>{{ l.label }}</b> <span class="hint">{{ l.hint }} ↗</span>
      </a>
    </div>
  </div>
</template>

<style scoped>
.bio { line-height: 1.7; color: #c9c6bd; }
.links { margin-top: 16px; display: flex; flex-direction: column; gap: 8px; }
.links a { color: #e8e6df; text-decoration: none; border: 1px solid #2a2822; padding: 10px 14px; display: flex; justify-content: space-between; }
.links a:hover { border-color: #d9a53a; }
.links b { color: #d9a53a; letter-spacing: .1em; font-size: 12px; }
.links .hint { color: #8a867c; font-size: 12px; }
</style>
