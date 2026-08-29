-- 0001 · bootstrap marker (ticket 03: proves the daemon-host → Postgres path)
-- The real schema lands with ticket 05 (Rollup & HLL schema).
create table if not exists _pivot_bootstrap (
  applied_at timestamptz not null default now()
);
