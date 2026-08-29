# Rollup schema: nullable-wiki hour/day tables, p=12 sketches with daemon-computed estimates, atomic batch+cursor batches

The rollup schema turns ADR 0001's storage budget into tables: three rollup tables (`rollup_minute` totals-only; `rollup_hour`/`rollup_day` with a nullable `wiki_id` where NULL means the global totals row that owns the sketch and non-NULL means per-wiki counters, unified under a unique index on `(bucket_start, coalesce(wiki_id, 0))`), a 6-hour `raw_events` window keyed on the stream's unique event id, and ingestion bookkeeping (`ingest_cursor`, `ingest_batches`, `ingest_gaps`). HLL sketches are Rust-side (`p=12`, ~1.6% typical error): the daemon unions minute sketches into hour and hour into day as buckets close, which is mathematically identical to recomputing from raws and needs no replay machinery. Because Postgres has no `hll` extension (verified live), SQL cannot read a sketch — so totals rows carry both the `editors_sketch` (for future merges/audit) and a daemon-computed `editors_est` bigint, which is what views and charts read.

Counting semantics: every flush is **one Postgres transaction** — raw rows, counter upserts, sketch update, and the cursor advance commit atomically, so a crash can never leave data and cursor disagreeing (no double-counting by construction, not by bookkeeping); flushes fire at 5 s or 256 events, whichever comes first. `ingest_batches` is an audit ledger whose unique index on `last_event_id` makes a replayed batch range fail loudly. Retention deletes belong to `pg_cron` (ADR 0001), scheduled in the same migration.

## Consequences

- `editors_est` is the only queryable unique-editor number; sketches exist for merge correctness and audit, not for SQL. The dashboard and README must label it an estimate (~1.6% typical error).
- The `coalesce(wiki_id, 0)` expression index means upserts must target that exact expression — a plain `(bucket_start, wiki_id)` conflict target will not match anything.
- Per-wiki unique-editor counts would multiply sketches by ~300 wikis × 730 buckets (hundreds of MB) — deliberately not a feature; if ever wanted, it's a day-grain recompute from the VM archive, not a column.
- The ledger's `last_event_id` tripwire assumes the daemon skips empty flushes (no ledger row for a zero-event batch).
