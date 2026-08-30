export interface Health {
  cursor_updated_at: string | null
  stream_lag: string | null
  gaps: number
  events_24h: number
  db_size: string
}

export interface TimelineRow {
  bucket_start: string
  edits: number
  bot_edits: number
  new_pages: number
}

export interface WikiRow {
  server_name: string
  edits: number
}

export interface TrendRow {
  hour: string
  editors_est: number | null
}

// The one filter: three presets mapped to the three retention grains (ADR 0001)
export interface WindowPreset {
  key: '24h' | '7d' | '90d'
  label: string
  timeline: string
  wikis: string
}

export const WINDOWS: WindowPreset[] = [
  { key: '24h', label: '24H · MINUTE', timeline: 'v_edits_timeline', wikis: 'v_top_wikis_24h' },
  { key: '7d', label: '7D · HOUR', timeline: 'v_edits_timeline_7d', wikis: 'v_top_wikis_7d' },
  { key: '90d', label: '90D · DAY', timeline: 'v_edits_timeline_90d', wikis: 'v_top_wikis_90d' },
]
