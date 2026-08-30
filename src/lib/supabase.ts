import { createClient } from '@supabase/supabase-js'
import { onMounted, onUnmounted, ref, type WatchSource } from 'vue'

// anon key — public by design; RLS + view grants are the gate (ADR 0004)
export const supabase = createClient(
  import.meta.env.VITE_SUPABASE_URL,
  import.meta.env.VITE_SUPABASE_ANON_KEY,
)

export async function fetchView<T>(view: string): Promise<T[]> {
  const { data, error } = await supabase.from(view).select('*')
  if (error) throw new Error(error.message)
  return (data ?? []) as T[]
}

// Poll per view with its own interval; paused while the tab is hidden so an
// always-open dashboard can't burn egress (ADR 0004). Re-runs on watched change.
export function usePoll<T>(fetcher: () => Promise<T>, intervalMs = 60_000, watch?: WatchSource) {
  const data = ref<T | null>(null)
  const error = ref<string | null>(null)
  let id: number | undefined
  async function run() {
    if (document.hidden) return // hidden tabs cost nothing
    try {
      data.value = await fetcher()
      error.value = null
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }
  onMounted(() => {
    run()
    id = window.setInterval(run, intervalMs)
    document.addEventListener('visibilitychange', run)
    onUnmounted(() => {
      clearInterval(id)
      document.removeEventListener('visibilitychange', run)
    })
  })
  if (watch) {
    watch(watch, run)
  }
  return { data, error }
}
