# Research: Supabase free-tier capabilities (ticket 02)

Primary sources: [Supabase pricing](https://supabase.com/pricing), [Supabase Cron guide](https://supabase.com/docs/guides/cron), [Edge Functions limits](https://supabase.com/docs/guides/functions/limits), [Supabase blog: "seen by" in Postgres](https://supabase.com/blog/seen-by-in-postgresql) (HLL on Supabase), [postgresql-hll](https://github.com/citusdata/postgresql-hll).

## Free plan limits (pricing page, quoted)

- **"500 MB database size (Shared CPU • 500 MB RAM)"**
- **"5 GB egress"** + 5 GB cached egress
- 500,000 Edge Function invocations/month; 50,000 MAU (irrelevant here)
- **"Free projects are paused after 1 week of inactivity. Limit of 2 active projects."**
- Caveat: the docs do **not** publish what counts as "activity". A daemon writing every few seconds and a dashboard polling every 60 s are the most demanding activity pattern short of paid; verify empirically during provisioning (ticket 03) and, if needed, add a heartbeat.

## Extensions — what the design can rely on

- **pg_cron: available.** "Under the hood, Supabase Cron uses the `pg_cron` Postgres database extension." Limits: ≤ 8 jobs run concurrently, each ≤ 10 minutes, schedulable every second to once a year. Free-tier eligibility not explicitly stated on the page — verify `select * from pg_available_extensions where name = 'pg_cron'` during provisioning (ticket 03).
- **hll (postgresql-hll): available.** No dedicated doc page (404), but Supabase's own engineering blog benchmarks the `hll` extension on Supabase, and it is part of the standard extension set. Verify `pg_available_extensions` during provisioning. Functions that matter for ticket 05: `hll_add_agg`, `hll_union_agg`, `hll_cardinality`, `hll_hash_*` — sketches are `bytea`, merge with `hll_union` in SQL. This makes **SQL-native HLL** the default plan; Rust-side sketches are the fallback only.
- pg_partman: no evidence it's offered; not needed (ticket 04 can bucket by range partitions created by pg_cron or just rely on indexes + retention deletes).

## What the 24/7 ingester cannot use

- **Edge Functions: free wall clock 150 s** (paid 400 s), CPU 2 s/request, 256 MB. A 24/7 SSE consumer cannot live there — confirms the Rust daemon on the Oracle VM as the only viable host.

## Consequences for the design

1. Rollups + retention + HLL all fit in the database proper: pg_cron (or the daemon) for deletes; `hll` for unique-editor estimates — no app-side cardinality machinery needed (ticket 05).
2. 500 MB is the binding constraint; the 5 GB egress is comfortable for a polling dashboard (~KB per request).
3. The 2-active-project limit is a real constraint for ticket 03 (a currently-idle project may need pausing).
4. The pause policy means the demo's health and the project's health are the same thing — another reason the daemon must run from day one.

## Correction (2026-08-30, verified live on project <project-ref>)

Provisioning (ticket 03) ran `pg_available_extensions` against the real project and corrected two claims above:

- **`hll` is NOT available.** The full catalogue (76 extensions) has no hyperloglog extension under any name — the Supabase blog post cited above is from an earlier platform era. Consequence: **Rust-side HLL sketches stored as `bytea` is the plan of record** for ticket 05, not SQL-native HLL.
- **`pg_partman` IS available** (v5.2.0-era) alongside `pg_cron` v1.6.4 (both verified in `pg_available_extensions`).

Also recorded: the user chose region **eu-west-1** (Ireland) for the project; the ingest VM is in Paris — a ~10 ms RTT that batched writes amortize.
