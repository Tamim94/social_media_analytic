<script setup lang="ts">
import { computed } from 'vue'
import type { WikiRow } from '../types'

const props = defineProps<{ rows: WikiRow[] }>()

const sorted = computed(() => [...props.rows].sort((a, b) => b.edits - a.edits).slice(0, 20))
const max = computed(() => sorted.value[0]?.edits ?? 1)
</script>

<template>
  <div class="card">
    <h3>Top wikis <i>· edits in window</i></h3>
    <div v-if="sorted.length">
      <div v-for="r in sorted" :key="r.server_name" class="row">
        <span class="n">{{ r.server_name }}</span>
        <div><div class="bar" :style="{ width: (r.edits / max) * 100 + '%' }"></div></div>
        <span class="num">{{ r.edits.toLocaleString() }}</span>
      </div>
    </div>
    <p v-else class="foot">waiting for data…</p>
  </div>
</template>
