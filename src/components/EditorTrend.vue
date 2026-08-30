<script setup lang="ts">
import { computed } from 'vue'
import type { TrendRow } from '../types'

const props = defineProps<{ rows: TrendRow[] }>()

const W = 460, H = 230, P = 10
const IVORY = '#e8e6df', DIM = '#3a382f'

const chart = computed(() => {
  const rows = [...props.rows].sort(
    (a, b) => Date.parse(a.hour) - Date.parse(b.hour),
  )
  if (rows.length < 2) return null
  const vals = rows.map(r => r.editors_est ?? 0)
  const max = Math.max(...vals, 1) * 1.15
  const n = vals.length
  const pts = vals.map((v, i) =>
    `${(i / (n - 1)) * (W - 2 * P) + P},${H - P - (v / max) * (H - 2 * P)}`)
  let html = ''
  for (let i = 1; i < 4; i++) {
    const gy = P + (i / 4) * (H - 2 * P)
    html += `<line x1="${P}" x2="${W - P}" y1="${gy}" y2="${gy}" stroke="${DIM}" stroke-width="1"/>`
  }
  html += `<polygon fill="${IVORY}" opacity="0.22" points="${P},${H - P} ${pts.join(' ')} ${W - P},${H - P}"/>`
  html += `<polyline fill="none" stroke="${IVORY}" stroke-width="1.4" points="${pts.join(' ')}"/>`
  return html
})
</script>

<template>
  <div class="card">
    <h3>Unique editors <i>· per hour · 30 days</i></h3>
    <svg v-if="chart" viewBox="0 0 460 230" v-html="chart"></svg>
    <p v-else class="foot">waiting for data…</p>
    <div class="foot">
      HyperLogLog estimate · p=12 · ~1.6% typical error — labeled as an estimate, on purpose
    </div>
  </div>
</template>
