<script setup lang="ts">
import { computed } from 'vue'
import type { TimelineRow } from '../types'

const props = defineProps<{ rows: TimelineRow[]; grainLabel: string }>()

const W = 940, H = 230, P = 10
const IVORY = '#e8e6df', AMBER = '#d9a53a', GREEN = '#6a9955', DIM = '#3a382f'

const chart = computed(() => {
  const rows = [...props.rows].sort(
    (a, b) => Date.parse(a.bucket_start) - Date.parse(b.bucket_start),
  )
  if (rows.length < 2) return null
  const series = [
    rows.map(r => r.new_pages),
    rows.map(r => r.edits - r.bot_edits), // humans
    rows.map(r => r.bot_edits),
  ]
  const max = Math.max(...series.flat(), 1) * 1.15
  const n = rows.length
  const x = (i: number) => (i / (n - 1)) * (W - 2 * P) + P
  const y = (v: number) => H - P - (v / max) * (H - 2 * P)
  const line = (vals: number[]) => vals.map((v, i) => `${x(i)},${y(v)}`).join(' ')
  let html = ''
  for (let i = 1; i < 4; i++) {
    const gy = P + (i / 4) * (H - 2 * P)
    html += `<line x1="${P}" x2="${W - P}" y1="${gy}" y2="${gy}" stroke="${DIM}" stroke-width="1"/>`
  }
  html += `<polygon fill="${AMBER}" opacity="0.28" points="${P},${H - P} ${line(series[2])} ${W - P},${H - P}"/>`
  html += `<polyline fill="none" stroke="${AMBER}" stroke-width="1" points="${line(series[2])}"/>`
  html += `<polyline fill="none" stroke="${IVORY}" stroke-width="1.4" points="${line(series[1])}"/>`
  html += `<polyline fill="none" stroke="${GREEN}" stroke-width="1" points="${line(series[0])}"/>`
  html += `<text x="${P}" y="${H - 1}" fill="#8a867c" font-size="9">start</text>`
  html += `<text x="${W - P - 22}" y="${H - 1}" fill="#8a867c" font-size="9">now</text>`
  return html
})
</script>

<template>
  <div class="card wide">
    <h3>Edits per {{ props.grainLabel }} — humans vs bots <i>· live</i></h3>
    <svg v-if="chart" viewBox="0 0 940 230" v-html="chart"></svg>
    <p v-else class="foot">waiting for enough history…</p>
    <div class="legend">
      <span><span class="dot" style="background: #e8e6df"></span>human edits</span>
      <span><span class="dot" style="background: #d9a53a"></span>bot edits</span>
      <span><span class="dot" style="background: #6a9955"></span>new pages</span>
    </div>
  </div>
</template>
