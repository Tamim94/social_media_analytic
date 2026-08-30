<script setup lang="ts">
import { usePoll, fetchView } from '../lib/supabase'
import type { Health } from '../types'

const { data } = usePoll<Health[]>(() => fetchView<Health>('v_health'), 120_000)

function lagSeconds(lag: string | null): string {
  if (!lag) return '—'
  const [h, m, s] = lag.split(':').map(Number)
  const total = (h || 0) * 3600 + (m || 0) * 60 + (s || 0)
  return total >= 90 ? `${Math.round(total / 60)} m` : `${Math.round(total * 10) / 10} s`
}
</script>

<template>
  <div class="strip">
    <div class="cell">
      <span class="label">stream lag</span>
      <b>{{ data?.[0] ? lagSeconds(data[0].stream_lag) : '—' }}</b>
    </div>
    <div class="cell">
      <span class="label">recorded gaps</span>
      <b>{{ data?.[0]?.gaps ?? '—' }}</b>
    </div>
    <div class="cell">
      <span class="label">events 24 h</span>
      <b>{{ data?.[0]?.events_24h?.toLocaleString() ?? '—' }}</b>
    </div>
    <div class="cell">
      <span class="label">database</span>
      <b>{{ data?.[0]?.db_size ?? '—' }}<em> / 500 MB</em></b>
    </div>
  </div>
</template>
