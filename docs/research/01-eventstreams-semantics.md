# Research: EventStreams semantics (ticket 01)

Primary sources: [Wikitech EventStreams](https://wikitech.wikimedia.org/wiki/EventStreams), [mediawiki/recentchange schema 1.0.1 at schema.wikimedia.org](https://schema.wikimedia.org/repositories/primary/jsonschema/mediawiki/recentchange/latest.yaml), [Manual:RCFeed](https://www.mediawiki.org/wiki/Manual:RCFeed), plus a live 8-second peek at the stream from this machine (2026-08-29).

## Streams and endpoints

- Base: `https://stream.wikimedia.org/v2/stream/<stream-name>` — SSE (text/event-stream). Sending `Accept: application/json` yields raw NDJSON instead (handy for curl/tests).
- `mediawiki.recentchange` is the global edit firehose: every edit, new page, log entry, and categorization on all WMF wikis. `mediawiki.revision-create` exists as a companion (revision metadata). Multiple streams can be composed comma-separated in one request.
- No server-side filtering: the full global stream arrives; wiki/event-type filtering is client-side.

## Event schema (recentchange 1.0.1) — confirmed live

Top-level: `$schema, meta, id (rcid), type, bot, namespace, title, title_url, user, comment, parsedcomment, timestamp (unix), wiki, server_name, server_url, server_script_path`. `meta`: `domain, dt (ISO-8601 UTC), id (unique event id), offset, partition, request_id, stream, topic, uri`.

- `type` ∈ `edit | new | log | categorize | external`.
- Byte deltas: `length.old` / `length.new` (null for new pages' `old`); delta = `new − old`. Present **only** for `edit`/`new`.
- `revision.old` / `revision.new` (revision ids), same conditionality.
- `log_*` fields (`log_id, log_type, log_action, log_params, log_action_comment`) only for `type=log`, where `namespace` is −1. `categorize` carries only common fields.
- `bot` boolean; `minor`/`patrolled` present only when applicable.
- Live peek: 191 events / 8 s ≈ **23.9 events/s** sustained (earlier session measured 30.7/s — bursty around it), 25 distinct wikis, dominated by `commons.wikimedia.org`, `en.wikipedia.org`, `www.wikidata.org`.

## Resume and retention — the numbers that design the cursor

- Every SSE event carries an `id`. WMF is multi-DC active/active, so **ids are timestamps, not Kafka offsets** (offsets are unreliable across DCs). `Last-Event-ID` header accepts a JSON array of `{topic, partition, timestamp|offset}`.
- Simpler equivalent: `since=` query parameter accepting anything `Date.parse()`-able (UTC ISO-8601). Historical replay available since June 2018.
- **Replay window: 7–31 days of history** depending on the stream ("Depending on the stream configuration, there will likely be between 7 and 31 days of history available").
- **Server-enforced connection timeout: 15 minutes** — WMF's connection-termination layer kills every connection at 15 min; clients MUST auto-reconnect with `Last-Event-ID`. Reconnection is a certainty (~96×/day), not an edge case.
- Discard events where `meta.domain === 'canary'` (they exist to keep connections alive).
- Etiquette: identify with a descriptive `User-Agent` with contact info ([mw:API:Etiquette](https://www.mediawiki.org/wiki/API:Etiquette)); consume only what you need.

## Consequences for the design

1. Cursor = event timestamp (or `meta.id`), stored durably in Postgres; resume with `since=` on boot, `Last-Event-ID` on within-session reconnects (ticket 07).
2. Gap recovery window is bounded at 7–31 days; anything longer is an honest gap in the ledger (ticket 07).
3. The daemon must treat reconnect-on-15-min-kill as the normal path, and count gaps introduced by it (should be ~zero via Last-Event-ID).
4. `type=edit|new` are the counting events; `bot` flag gives the human/bot split for free; byte delta needs `length.new − length.old` with null-handling for `new` pages.
