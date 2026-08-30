-- 0004 · daemon stats snapshots (ticket 10 follow-up: "everything is logged")
-- Every 30 minutes the daemon records its own pulse: memory, cumulative
-- events, database size. This is the paper trail behind the README's
-- storage-growth story (72 -> 198 -> ~210 MB steady state) and the seed of a
-- future "storage over time" chart.

create table if not exists daemon_stats (
  ts             timestamptz not null default now(),
  rss_kb         bigint not null,
  events_total   bigint not null,
  db_size_bytes  bigint not null
);
create index if not exists daemon_stats_ts_idx on daemon_stats (ts desc);

-- new table => apply the same lockdown as 0003 (default privileges would
-- otherwise expose it to anon) and grant the read path explicitly
alter table daemon_stats enable row level security;
grant select on daemon_stats to anon;
