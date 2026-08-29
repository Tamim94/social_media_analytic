-- 0002 · rollup schema (ticket 05) — decisions in docs/adr/0002-rollup-schema.md
-- Tiered dimensions per docs/adr/0001-storage-and-retention.md:
--   minute = global totals + bot split only; hour/day add nullable per-wiki rows.
-- HLL sketches (p=12, computed by the Rust daemon) ride only on totals rows;
-- per-wiki rows carry counters only. SQL cannot interpret the sketches (no hll
-- extension), so totals rows also carry the daemon-computed editors_est.

-- ── dimension ────────────────────────────────────────────────────────────
create table if not exists wikis (
  id          smallint generated always as identity primary key,
  server_name text not null unique,
  added_at    timestamptz not null default now()
);

-- ── raw hot window (6 h, cron-dropped; audit + re-derivation) ────────────
create table if not exists raw_events (
  event_id    text primary key,          -- meta.id: replay dedup for free
  ts          timestamptz not null,      -- meta.dt
  wiki_id     smallint not null references wikis(id),
  type        text not null,             -- edit | new | log | categorize | external
  bot         boolean not null,
  bytes_delta integer,                   -- length.new − length.old (null for new/log/categorize)
  user_text   text not null
);
create index if not exists raw_events_ts_idx on raw_events (ts);

-- ── rollups ──────────────────────────────────────────────────────────────
-- Counting vocabulary (see CONTEXT.md):
--   edits     = count of type in (edit, new)
--   new_pages = count of type = new
--   bot_edits = subset of edits where bot
--   bytes_added   = Σ max(bytes_delta, 0)
--   bytes_removed = Σ max(−bytes_delta, 0)

create table if not exists rollup_minute (
  bucket_start   timestamptz primary key,
  edits          bigint not null default 0,
  new_pages      bigint not null default 0,
  bot_edits      bigint not null default 0,
  bytes_added    bigint not null default 0,
  bytes_removed  bigint not null default 0,
  editors_sketch bytea,                  -- HLL p=12, global per minute
  editors_est    bigint                  -- daemon-computed cardinality of editors_sketch
);

create table if not exists rollup_hour (
  bucket_start   timestamptz not null,
  wiki_id        smallint references wikis(id),  -- NULL = totals row (owns sketch); non-NULL = per-wiki counters
  edits          bigint not null default 0,
  new_pages      bigint not null default 0,
  bot_edits      bigint not null default 0,
  bytes_added    bigint not null default 0,
  bytes_removed  bigint not null default 0,
  editors_sketch bytea,
  editors_est    bigint
);
-- coalesce lets the NULL totals row and per-wiki rows share one uniqueness space;
-- the daemon's upserts target this expression directly.
create unique index if not exists rollup_hour_bucket_wiki_idx
  on rollup_hour (bucket_start, coalesce(wiki_id, 0));

create table if not exists rollup_day (
  bucket_start   timestamptz not null,
  wiki_id        smallint references wikis(id),
  edits          bigint not null default 0,
  new_pages      bigint not null default 0,
  bot_edits      bigint not null default 0,
  bytes_added    bigint not null default 0,
  bytes_removed  bigint not null default 0,
  editors_sketch bytea,
  editors_est    bigint
);
create unique index if not exists rollup_day_bucket_wiki_idx
  on rollup_day (bucket_start, coalesce(wiki_id, 0));

-- ── ingestion bookkeeping (semantics: ADR 0003 / ticket 07) ──────────────
create table if not exists ingest_cursor (
  id            boolean primary key default true check (id),  -- singleton row
  last_event_id text not null,         -- meta.id of last applied event
  last_event_ts timestamptz not null,
  updated_at    timestamptz not null default now()
);

-- Audit ledger; the unique last_event_id makes a replayed batch range fail loudly.
create table if not exists ingest_batches (
  batch_id       bigint generated always as identity primary key,
  last_event_id  text not null,
  first_event_ts timestamptz not null,
  last_event_ts  timestamptz not null,
  events         integer not null,
  applied_at     timestamptz not null default now()
);
create unique index if not exists ingest_batches_last_event_idx on ingest_batches (last_event_id);

create table if not exists ingest_gaps (
  id         bigint generated always as identity primary key,
  gap_start  timestamptz not null,
  gap_end    timestamptz not null check (gap_end > gap_start),
  cause      text,
  noticed_at timestamptz not null default now()
);

-- ── dashboard query surface (anon grants land in ticket 10) ─────────────
create or replace view v_edits_timeline as
  select bucket_start, edits, bot_edits, new_pages
  from rollup_minute
  where bucket_start > now() - interval '24 hours'
  order by bucket_start;

create or replace view v_top_wikis_24h as
  select w.server_name, sum(r.edits) as edits
  from rollup_hour r
  join wikis w on w.id = r.wiki_id
  where r.bucket_start > now() - interval '24 hours'
    and r.wiki_id is not null
  group by w.server_name
  order by edits desc
  limit 20;

create or replace view v_bot_ratio_24h as
  select sum(edits) as edits,
         sum(bot_edits) as bot_edits,
         sum(bot_edits)::float8 / nullif(sum(edits), 0) as bot_ratio
  from rollup_minute
  where bucket_start > now() - interval '24 hours';

create or replace view v_editor_trend as
  select bucket_start as hour, editors_est
  from rollup_hour
  where wiki_id is null
    and bucket_start > now() - interval '30 days'
  order by bucket_start;

create or replace view v_health as
  select
    (select updated_at from ingest_cursor)                    as cursor_updated_at,
    now() - (select last_event_ts from ingest_cursor)         as stream_lag,
    (select count(*) from ingest_gaps)                        as gaps,
    (select coalesce(sum(events), 0) from ingest_batches
      where applied_at > now() - interval '24 hours')         as events_24h,
    pg_size_pretty(pg_database_size(current_database()))      as db_size;

-- ── retention (ADR 0001: pg_cron owns deletes; daemon never deletes) ─────
-- "available" ≠ installed: the extension must be created before scheduling.
create extension if not exists pg_cron;
select cron.schedule('retention-raw',    '23 * * * *', $$delete from raw_events    where ts           < now() - interval '6 hours'$$);
select cron.schedule('retention-minute', '41 3 * * *', $$delete from rollup_minute where bucket_start < now() - interval '7 days'$$);
select cron.schedule('retention-hour',   '47 3 * * *', $$delete from rollup_hour   where bucket_start < now() - interval '30 days'$$);
select cron.schedule('retention-day',    '53 3 * * *', $$delete from rollup_day    where bucket_start < now() - interval '90 days'$$);
