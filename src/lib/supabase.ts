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

// staggered 60 s poll per view (ticket 09); re-runs when the watched source changes
export function usePoll<T>(fetcher: () => Promise<T>, offsetMs = 0, watch?: WatchSource) {
  const data = ref<T | null>(null)
  const error = ref<string | null>(null)
  async function run() {
    try {
      data.value = await fetcher()
      error.value = null
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }
  onMounted(() => {
    run()
    const id = setInterval(run, 60_000 + offsetMs)
    onUnmounted(() => clearInterval(id))
  })
  if (watch) {
    watch(watch, run)
  }
  return { data, error }
}
