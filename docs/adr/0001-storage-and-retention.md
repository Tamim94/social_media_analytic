# Storage & retention: tiered dimensions, hot raw window + VM archive, 90/30/7, pg_cron deletes

The Wikimedia firehose produces ~3.8 GB/day of raw events against a 500 MB database, so storage is a set of deliberate exclusions, not a default schema. We decided: **dimensions are tiered** (minute buckets carry only global totals plus a human/bot split; per-wiki rows exist only at hour and day grain — wiki × minute would be ~430k rows/day, the budget killer, and no chart needs per-wiki per-minute resolution); **raw events live outside the rollup path** (a ~6 h trimmed raw window in Postgres, cron-dropped, for auditing rollups against raws and re-deriving a bucket after a bug, plus a 30-day NDJSON.gz archive on the ingest VM for DuckDB experimentation); **retention is 90 days daily / 30 days hourly / 7 days minutely** (matches the moat claim "90 days of queryable history"); and **`pg_cron` owns the deletes** (retention holds even when the daemon is down; the daemon stays a pure writer).

## Considered Options

- **Uniform dimensions at all grains** — rejected: simplest daemon code, but the minute tier balloons to ~430k rows/day and forces ~2-day retention.
- **Totals only, no dimensions** — rejected: loses the top-wikis chart, the best story the data tells.
- **No raw anywhere** — rejected: a buggy aggregator would silently poison history with nothing to re-derive from; the raw window is what makes "rollups verified against raws" a checkable claim.
- **Deleting in the daemon** — rejected: couples retention to daemon uptime, the one thing that must be allowed to fail.
- **pg_partman partition drops** — the right pattern at scale and available on the platform, but more DDL machinery for tables that stay at tens of MB here; the grow-up-later path.

## Budget (steady state)

| Store | Rows/day | Retention | ≈ Size |
|---|---|---|---|
| minute totals + bot split (carries the global HLL sketch, ~4 KB) | 1,440 | 7 d | ~43 MB |
| hour totals (sketch) | 24 | 30 d | ~3 MB |
| hour × wiki (counters only, no sketches) | ~7,200 | 30 d | ~33 MB |
| day totals (sketch) | 24 | 90 d | <1 MB |
| day × wiki (counters only) | ~300 | 90 d | ~4 MB |
| raw hot window (trimmed columns) | ~575k/6 h | 6 h | ~85 MB |
| cursor, gap ledger, misc | — | — | ~1 MB |
| **Postgres total** | | | **~170 MB of 500 MB** (~65% headroom) |
| VM NDJSON.gz archive | ~380 MB/day gzipped | 30 d | ~11 GB of 130 GB |

## Consequences

- **Sketches live only on totals rows.** Per-(bucket, wiki) unique-editor counts would multiply 4 KB sketches by ~300 wikis × 730 buckets — hundreds of MB for a number no chart shows. Wiki rows carry counters only; unique-editors is a global-per-bucket series.
- The raw window's presence is what licenses re-derivation: any rollup bug is fixed by replaying ≤ 6 h from Postgres, or ≤ 30 days from the VM archive, or nothing (honest gap) beyond that.
- Deletion is `pg_cron`'s job and must be reviewed if the archive window or any tier number changes.
