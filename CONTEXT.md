# wikistream

A live analytics pipeline over Wikimedia's global edit stream (EventStreams): a Rust daemon ingests the firehose, Postgres holds rollups and a short raw window, a Vue dashboard reads estimates.

## Language

### Counting

**Content edit**:
An EventStreams event of type `edit` or `new`. Counted in `edits`; log and categorize events are observed but never counted as edits.
_Avoid_: change, revision (a revision is the underlying MW object, not the counted event)

**New page**:
A content edit whose type is `new`; counted in `new_pages` and also inside `edits`.
_Avoid_: creation, article (namespace-agnostic)

**Bytes added / bytes removed**:
The positive and negative halves of a byte delta (`length.new − length.old`): added = Σ max(Δ, 0), removed = Σ max(−Δ, 0). Null for events without lengths.
_Avoid_: net bytes (there are two numbers, not one)

### Cardinality

**Editor**:
The `user` field of an event. Unique-editor counts are always HyperLogLog estimates, never exact numbers.
_Avoid_: author, account, user (reserved for the raw field name)

**Sketch**:
The serialized HLL register set (precision 12, Rust-side) for one bucket. Lives only on global totals rows; per-wiki rows carry counters only.
_Avoid_: bitmap, digest

**Editor estimate (`editors_est`)**:
The daemon-computed cardinality of a sketch — the only queryable unique-editor number, since SQL cannot interpret sketches.
_Avoid_: unique users, distinct count (implies exactness)

### Buckets

**Totals row / wiki row**:
In hour/day buckets: `wiki_id IS NULL` is the totals row (owns the sketch and estimate); non-NULL is a per-wiki counter row. Minute grain has only totals rows.
_Avoid_: global row, aggregate row

**Bucket**:
One time grain (minute/hour/day) starting at `bucket_start`, UTC-aligned.
_Avoid_: window, interval

### Ingestion

**Batch**:
One flush cycle of the daemon: raw rows + counter upserts + sketch update + cursor advance, committed as a single Postgres transaction.
_Avoid_: commit, chunk

**Cursor**:
The singleton row holding the last applied event's id and timestamp; the resume point on reconnect or reboot.
_Avoid_: offset (WMF stream ids are timestamps, not Kafka offsets)

**Gap**:
A period with no ingested events that exceeds the stream's 7–31 day replay window — recorded honestly, never back-filled silently.
_Avoid_: outage (that's a cause, not the record)
