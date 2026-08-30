-- 0002b · retention scheduling — Supabase-only (pg_cron).
-- Split from 0002 so CI can verify the core schema against vanilla Postgres.
-- Safe to re-run: cron.schedule upserts by job name.

create extension if not exists pg_cron;

select cron.schedule('retention-raw',    '23 * * * *', $$delete from raw_events    where ts           < now() - interval '6 hours'$$);
select cron.schedule('retention-minute', '41 3 * * *', $$delete from rollup_minute where bucket_start < now() - interval '7 days'$$);
select cron.schedule('retention-hour',   '47 3 * * *', $$delete from rollup_hour   where bucket_start < now() - interval '30 days'$$);
select cron.schedule('retention-day',    '53 3 * * *', $$delete from rollup_day    where bucket_start < now() - interval '90 days'$$);
