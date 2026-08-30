<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { fetchView, usePoll } from './lib/supabase'
import { WINDOWS, type TimelineRow, type TrendRow, type WikiRow } from './types'
import HealthStrip from './components/HealthStrip.vue'
import WindowTabs from './components/WindowTabs.vue'
import EditsTimeline from './components/EditsTimeline.vue'
import TopWikis from './components/TopWikis.vue'
import EditorTrend from './components/EditorTrend.vue'
import About from './views/About.vue'

const view = ref<'dash' | 'about'>('dash')
const winKey = ref('24h')
const win = () => WINDOWS.find(w => w.key === winKey.value) ?? WINDOWS[0]

const timeline = usePoll<TimelineRow[]>(() => fetchView<TimelineRow>(win().timeline), 60_000, watch(winKey))
const wikis = usePoll<WikiRow[]>(() => fetchView<WikiRow>(win().wikis), 300_000, watch(winKey))
const trend = usePoll<TrendRow[]>(() => fetchView<TrendRow>('v_editor_trend'), 600_000)

const GRAIN_LABEL: Record<string, string> = { '24h': 'minute', '7d': 'hour', '90d': 'day' }

// latest-minute tape line, from the freshest timeline sample
const tape = ref<{ t: string; edits: number; human: number; bot: number; np: number } | null>(null)
watch(
  () => timeline.data.value,
  rows => {
    const last = [...(rows ?? [])].sort(
      (a, b) => Date.parse(b.bucket_start) - Date.parse(a.bucket_start),
    )[0]
    if (last) {
      tape.value = {
        t: new Date(last.bucket_start).toISOString().slice(11, 16) + 'Z',
        edits: last.edits,
        human: last.edits - last.bot_edits,
        bot: last.bot_edits,
        np: last.new_pages,
      }
    }
  },
)

const clock = ref('')
let clockId: number | undefined
onMounted(() => {
  clockId = window.setInterval(() => {
    clock.value = new Date().toISOString().replace('T', ' ').slice(0, 19) + ' UTC'
  }, 500)
})
onUnmounted(() => clearInterval(clockId))
</script>

<template>
  <header>
    <b>WIKISTREAM</b><span class="cursor"></span><span class="live">● LIVE</span>
    <span class="right nav">
      <span :class="{ on: view === 'dash' }" @click="view = 'dash'">DASHBOARD</span>
      <span :class="{ on: view === 'about' }" @click="view = 'about'">ABOUT</span>
      <span class="clock">{{ clock }}</span>
    </span>
  </header>
  <main class="wrap">
    <About v-if="view === 'about'" />

    <template v-else>
      <div v-if="tape" class="tape">
        <span>{{ tape.t }}</span>
        <span>edits <b>{{ tape.edits.toLocaleString() }}</b>/{{ GRAIN_LABEL[winKey] === 'day' ? 'day' : GRAIN_LABEL[winKey] === 'hour' ? 'hour' : 'min' }}</span>
        <span>human <b>{{ tape.human.toLocaleString() }}</b></span>
        <span>bot <b>{{ tape.bot.toLocaleString() }}</b></span>
        <span>new pages <b>{{ tape.np.toLocaleString() }}</b></span>
      </div>

      <WindowTabs v-model="winKey" />
      <HealthStrip />

      <div class="grid">
        <EditsTimeline :rows="timeline.data.value ?? []" :grain-label="GRAIN_LABEL[winKey]" />
        <TopWikis :rows="wikis.data.value ?? []" />
        <EditorTrend :rows="trend.data.value ?? []" />
      </div>

      <p v-if="timeline.error" class="error">{{ timeline.error }}</p>
    </template>
  </main>
</template>
