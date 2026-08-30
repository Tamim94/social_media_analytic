-- 0003 · dashboard views (window presets) + anon read grants (ticket 10)
-- Per ADR 0003/0004: anon reads VIEWS only — no table grants exist, and none
-- are added here. The views run with their owner's rights, so the anon role
-- can never touch raw_events or the ingestion bookkeeping.

-- ── window presets: 7 d → hour grain, 90 d → day grain ──────────────────
create or replace view v_edits_timeline_7d as
  select bucket_start, edits, bot_edits, new_pages
  from rollup_hour
  where wiki_id is null
    and bucket_start > now() - interval '7 days'
  order by bucket_start;

create or replace view v_edits_timeline_90d as
  select bucket_start, edits, bot_edits, new_pages
  from rollup_day
  where wiki_id is null
    and bucket_start > now() - interval '90 days'
  order by bucket_start;

create or replace view v_top_wikis_7d as
  select w.server_name, sum(r.edits) as edits
  from rollup_hour r
  join wikis w on w.id = r.wiki_id
  where r.wiki_id is not null
    and r.bucket_start > now() - interval '7 days'
  group by w.server_name
  order by edits desc
  limit 20;

create or replace view v_top_wikis_90d as
  select w.server_name, sum(r.edits) as edits
  from rollup_day r
  join wikis w on w.id = r.wiki_id
  where r.wiki_id is not null
    and r.bucket_start > now() - interval '90 days'
  group by w.server_name
  order by edits desc
  limit 20;

-- ── anon read path: the entire public surface ────────────────────────────
grant select on
  v_edits_timeline,
  v_edits_timeline_7d,
  v_edits_timeline_90d,
  v_top_wikis_24h,
  v_top_wikis_7d,
  v_top_wikis_90d,
  v_editor_trend,
  v_health
to anon;

-- ── hardening: Supabase default privileges had granted anon SELECT on every
-- base table (91 objects). Lock them all down — RLS on, no policies = deny-all
-- for anon/authenticated; the daemon and the views run as table owner and are
-- unaffected. Views are the only public surface.
do $$
declare t text;
begin
  for t in
    select tablename from pg_tables
    where schemaname = 'public' and tablename not like 'v_%'
  loop
    execute format('alter table public.%I enable row level security', t);
  end loop;
end $$;
