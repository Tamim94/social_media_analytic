<script setup lang="ts">
const links = [
  { label: 'GITHUB', url: 'https://github.com/Tamim94', hint: 'this repo + everything else I build' },
  { label: 'LINKEDIN', url: 'https://www.linkedin.com/in/tamim-golam', hint: 'the formal version' },
  { label: 'PORTFOLIO', url: 'https://tamimg-portfolio.web.app', hint: 'the other projects' },
]
</script>

<template>
  <div class="card" style="max-width: 760px">
    <h3>About · Tamim GOLAM <i>· golam.tamim94@gmail.com</i></h3>
    <p class="bio">
      I wanted a portfolio project that proves something a scaffolded UI can't: that I can keep a
      system alive. This one ingests Wikimedia's global edit stream with a Rust daemon, holds 90
      days of history in a 500 MB free-tier database, records its own gaps instead of hiding them,
      and has been running — without me touching it — since August 2026. The hard parts here aren't
      the charts; they're the constraints the stream forces on every design decision.
    </p>
    <p class="bio" style="margin-top: 10px">
      If something on this page shows an estimate, it says so. If data was ever lost, there's a row
      in a gaps table admitting it. That's the kind of engineering I'm interested in.
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
