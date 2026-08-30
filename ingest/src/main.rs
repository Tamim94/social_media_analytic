//! wikistream-ingest — v0.2 (ticket 07: delivery semantics per ADR 0003)
//!
//! Consumes Wikimedia EventStreams `recentchange` (SSE), batches events every
//! 5 s / 256 events, and commits each batch as ONE Postgres transaction:
//! raws (6 h window) + rollup upserts at all three grains (minute/hour/day
//! totals with p=12 HLL sketch + estimate; per-wiki counter rows at
//! hour/day) + cursor advance + ledger row. Crash mid-batch rolls back
//! atomically — no double-counting by construction (ADR 0002).
//!
//! Resume: sessions re-send Last-Event-ID on the server's 15-minute kill
//! (reqwest-eventsource); boot resumes from the durable cursor via `since=`
//! (strictly newer). Gaps (data older than the replay window) are recorded
//! honestly in ingest_gaps — loss only, never routine reboots (ADR 0003).
//! Every event's full JSON is archived to hourly .ndjson.gz files under
//! /var/lib/wikistream/archive/ (30-day retention, cleaned on rotation).

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use chrono::{DateTime, SecondsFormat, Utc};
use futures_util::StreamExt;
use hyperloglogplus::{HyperLogLog, HyperLogLogPlus};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use tokio::sync::mpsc;

const STREAM_URL: &str = "https://stream.wikimedia.org/v2/stream/recentchange";
const FLUSH_EVERY_SECS: u64 = 5;
const FLUSH_EVERY_EVENTS: usize = 256;
const HLL_PRECISION: u8 = 12;
const GAP_THRESHOLD_DAYS: i64 = 6; // conservative vs the 7–31 day replay window
const ARCHIVE_DIR: &str = "/var/lib/wikistream/archive";
const ARCHIVE_RETENTION_DAYS: i64 = 30;
const UA: &str = "wikistream-ingest/0.2 (portfolio project; contact golam.tamim94@gmail.com)";

type Sketch = HyperLogLogPlus<String, SerHasher>;

/// Serde-serializable BuildHasher: the crate's serde impl bounds B: Serialize,
/// and std's BuildHasherDefault does not implement it. (Same shape as the
/// crate's own test hasher.)
#[derive(Serialize, Deserialize)]
struct SerHasher;

impl std::hash::BuildHasher for SerHasher {
    type Hasher = std::collections::hash_map::DefaultHasher;
    fn build_hasher(&self) -> Self::Hasher {
        Self::Hasher::new()
    }
}

fn log(msg: &str) {
    println!(
        "[{}] {}",
        Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        msg
    );
}

fn rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1).and_then(|p| p.parse().ok()))
        .map(|pages: u64| pages * 4096 / 1024)
        .unwrap_or(0)
}

// ── event parsing ─────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct Meta {
    id: String,
    dt: String,
    #[serde(default)]
    domain: String,
}

/// First-stage parse: canary keepalives are filtered before the full event
/// shape is required (they lack `type` — the soak's only json errors).
#[derive(serde::Deserialize)]
struct Envelope {
    meta: Meta,
}

#[derive(serde::Deserialize)]
struct Length {
    old: Option<i64>,
    new: Option<i64>,
}

#[derive(serde::Deserialize)]
struct RcEvent {
    meta: Meta,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    bot: bool,
    server_name: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    length: Option<Length>,
}

// ── buffers (ADR 0003: uniform grain path) ────────────────────────────────

const G_MINUTE: u8 = 0;
const G_HOUR: u8 = 1;
const G_DAY: u8 = 2;
const GRAINS: [u8; 3] = [G_MINUTE, G_HOUR, G_DAY];

fn bucket_key(ts: &DateTime<Utc>, grain: u8) -> i64 {
    let s = ts.timestamp();
    match grain {
        G_MINUTE => s.div_euclid(60) * 60,
        G_HOUR => s.div_euclid(3600) * 3600,
        _ => s.div_euclid(86400) * 86400,
    }
}

#[derive(Default)]
struct BucketAcc {
    edits: i64,
    new_pages: i64,
    bot_edits: i64,
    bytes_added: i64,
    bytes_removed: i64,
    sketch: Option<Sketch>,
}

impl BucketAcc {
    fn sketch(&mut self) -> &mut Sketch {
        self.sketch.get_or_insert_with(|| {
            HyperLogLogPlus::new(HLL_PRECISION, SerHasher).expect("hll init with valid precision")
        })
    }
}

struct RawRow {
    event_id: String,
    ts: DateTime<Utc>,
    server_name: String,
    kind: String,
    bot: bool,
    bytes_delta: Option<i64>,
    user: String,
}

#[derive(Default)]
struct Buffer {
    buckets: HashMap<(u8, i64), BucketAcc>,
    raws: Vec<RawRow>,
    last_event_id: String,
    last_event_ts: DateTime<Utc>,
}

impl Buffer {
    fn push(&mut self, ev: RcEvent) -> anyhow::Result<()> {
        let ts = DateTime::parse_from_rfc3339(&ev.meta.dt)
            .with_context(|| format!("bad meta.dt {:?}", ev.meta.dt))?
            .with_timezone(&Utc);
        self.last_event_id = ev.meta.id.clone();
        self.last_event_ts = ts;

        let bytes_delta = match (ev.kind.as_str(), &ev.length) {
            ("edit", Some(l)) => l.new.zip(l.old).map(|(n, o)| n - o),
            ("new", Some(l)) => l.new, // new page: bytes created
            _ => None,
        };
        self.raws.push(RawRow {
            event_id: ev.meta.id,
            ts,
            server_name: ev.server_name.clone(),
            kind: ev.kind.clone(),
            bot: ev.bot,
            bytes_delta,
            user: ev.user.clone(),
        });

        // counters + sketch: content edits only (CONTEXT.md vocabulary)
        if ev.kind == "edit" || ev.kind == "new" {
            for g in GRAINS {
                let acc = self.buckets.entry((g, bucket_key(&ts, g))).or_default();
                acc.edits += 1;
                if ev.kind == "new" {
                    acc.new_pages += 1;
                }
                if ev.bot {
                    acc.bot_edits += 1;
                }
                if let Some(d) = bytes_delta {
                    if d > 0 {
                        acc.bytes_added += d;
                    } else {
                        acc.bytes_removed -= d;
                    }
                }
                if !ev.user.is_empty() {
                    acc.sketch().insert(&ev.user);
                }
            }
        }
        Ok(())
    }
}

/// Batch-local per-wiki deltas for hour/day grain (counters only — per ADR
/// 0001, wiki rows never carry sketches).
type WikiDelta = (i64, i64, i64, i64, i64); // edits, new_pages, bot_edits, bytes_added, bytes_removed

fn wiki_deltas(buf: &Buffer, grain: u8) -> HashMap<(i64, String), WikiDelta> {
    let mut out: HashMap<(i64, String), WikiDelta> = HashMap::new();
    for r in &buf.raws {
        if r.kind != "edit" && r.kind != "new" {
            continue;
        }
        let e = out
            .entry((bucket_key(&r.ts, grain), r.server_name.clone()))
            .or_default();
        e.0 += 1;
        if r.kind == "new" {
            e.1 += 1;
        }
        if r.bot {
            e.2 += 1;
        }
        if let Some(d) = r.bytes_delta {
            if d > 0 {
                e.3 += d;
            } else {
                e.4 -= d;
            }
        }
    }
    out
}

// ── flush: one transaction per batch (ADR 0002) ───────────────────────────

async fn flush(
    pool: &PgPool,
    wiki_map: &mut HashMap<String, i16>,
    buf: &mut Buffer,
) -> anyhow::Result<()> {
    if buf.raws.is_empty() {
        return Ok(());
    }
    let t0 = Instant::now();
    let mut tx = pool.begin().await?;

    // dimension: resolve/insert wikis first (raws FK onto them)
    for name in buf.raws.iter().map(|r| &r.server_name).collect::<Vec<_>>() {
        if !wiki_map.contains_key(name) {
            let id: i16 = sqlx::query_scalar(
                "insert into wikis (server_name) values ($1)
                 on conflict (server_name) do update set server_name = excluded.server_name
                 returning id",
            )
            .bind(name)
            .fetch_one(&mut *tx)
            .await?;
            wiki_map.insert(name.clone(), id);
        }
    }

    // raw window (replay dedup via PK on meta.id, for free)
    let mut qb = sqlx::QueryBuilder::new(
        "insert into raw_events (event_id, ts, wiki_id, type, bot, bytes_delta, user_text) ",
    );
    qb.push_values(buf.raws.iter(), |mut b, r| {
        b.push_bind(&r.event_id)
            .push_bind(r.ts)
            .push_bind(wiki_map[&r.server_name])
            .push_bind(&r.kind)
            .push_bind(r.bot)
            .push_bind(r.bytes_delta)
            .push_bind(&r.user);
    });
    qb.push(" on conflict (event_id) do nothing");
    qb.build().execute(&mut *tx).await?;

    // totals rows at every grain (counters + sketch + estimate)
    for (g, table) in [
        (G_MINUTE, "rollup_minute"),
        (G_HOUR, "rollup_hour"),
        (G_DAY, "rollup_day"),
    ] {
        for ((_, key), acc) in buf.buckets.iter_mut().filter(|((grain, _), _)| *grain == g) {
            let est = acc.sketch.as_mut().map(|s| s.count() as i64);
            let sk: Vec<u8> = match acc.sketch.as_ref() {
                Some(s) => bincode::serialize(s)?,
                None => Vec::new(),
            };
            let bucket = DateTime::from_timestamp(*key, 0).expect("valid bucket key");
            if g == G_MINUTE {
                // minute grain is totals-only: no wiki_id column exists (ADR 0001)
                sqlx::query(
                    "insert into rollup_minute
                       (bucket_start, edits, new_pages, bot_edits, bytes_added, bytes_removed, editors_sketch, editors_est)
                     values ($1, $2, $3, $4, $5, $6, $7, $8)
                     on conflict (bucket_start) do update set
                       edits = rollup_minute.edits + excluded.edits,
                       new_pages = rollup_minute.new_pages + excluded.new_pages,
                       bot_edits = rollup_minute.bot_edits + excluded.bot_edits,
                       bytes_added = rollup_minute.bytes_added + excluded.bytes_added,
                       bytes_removed = rollup_minute.bytes_removed + excluded.bytes_removed,
                       editors_sketch = excluded.editors_sketch,
                       editors_est = excluded.editors_est",
                )
                .bind(bucket)
                .bind(acc.edits)
                .bind(acc.new_pages)
                .bind(acc.bot_edits)
                .bind(acc.bytes_added)
                .bind(acc.bytes_removed)
                .bind(sk)
                .bind(est)
                .execute(&mut *tx)
                .await?;
            } else {
                let sql = format!(
                    "insert into {table}
                       (bucket_start, wiki_id, edits, new_pages, bot_edits, bytes_added, bytes_removed, editors_sketch, editors_est)
                     values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                     on conflict (bucket_start, coalesce(wiki_id, 0)) do update set
                       edits = {table}.edits + excluded.edits,
                       new_pages = {table}.new_pages + excluded.new_pages,
                       bot_edits = {table}.bot_edits + excluded.bot_edits,
                       bytes_added = {table}.bytes_added + excluded.bytes_added,
                       bytes_removed = {table}.bytes_removed + excluded.bytes_removed,
                       editors_sketch = excluded.editors_sketch,
                       editors_est = excluded.editors_est"
                );
                sqlx::query(&sql)
                    .bind(bucket)
                    .bind(Option::<i16>::None) // totals row: owns the sketch
                    .bind(acc.edits)
                    .bind(acc.new_pages)
                    .bind(acc.bot_edits)
                    .bind(acc.bytes_added)
                    .bind(acc.bytes_removed)
                    .bind(sk)
                    .bind(est)
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }

    // per-wiki counter rows at hour/day grain (no sketches, per ADR 0001)
    for (g, table) in [(G_HOUR, "rollup_hour"), (G_DAY, "rollup_day")] {
        for ((key, server), (edits, new_pages, bot, ba, br)) in wiki_deltas(buf, g) {
            let bucket = DateTime::from_timestamp(key, 0).expect("valid bucket key");
            let sql = format!(
                "insert into {table}
                   (bucket_start, wiki_id, edits, new_pages, bot_edits, bytes_added, bytes_removed)
                 values ($1, $2, $3, $4, $5, $6, $7)
                 on conflict (bucket_start, coalesce(wiki_id, 0)) do update set
                   edits = {table}.edits + excluded.edits,
                   new_pages = {table}.new_pages + excluded.new_pages,
                   bot_edits = {table}.bot_edits + excluded.bot_edits,
                   bytes_added = {table}.bytes_added + excluded.bytes_added,
                   bytes_removed = {table}.bytes_removed + excluded.bytes_removed"
            );
            sqlx::query(&sql)
                .bind(bucket)
                .bind(wiki_map[&server])
                .bind(edits)
                .bind(new_pages)
                .bind(bot)
                .bind(ba)
                .bind(br)
                .execute(&mut *tx)
                .await?;
        }
    }

    // cursor + ledger ride the same transaction (ADR 0002: atomic batch+cursor)
    sqlx::query(
        "insert into ingest_cursor (id, last_event_id, last_event_ts, updated_at)
         values (true, $1, $2, now())
         on conflict (id) do update set
           last_event_id = excluded.last_event_id,
           last_event_ts = excluded.last_event_ts,
           updated_at = now()",
    )
    .bind(&buf.last_event_id)
    .bind(buf.last_event_ts)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "insert into ingest_batches (last_event_id, first_event_ts, last_event_ts, events)
         values ($1, $2, $3, $4)",
    )
    .bind(&buf.last_event_id)
    .bind(buf.raws.first().map(|r| r.ts).unwrap_or(buf.last_event_ts))
    .bind(buf.last_event_ts)
    .bind(buf.raws.len() as i32)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    let events = buf.raws.len();
    buf.raws.clear();
    buf.buckets.clear();
    log(&format!(
        "batch: {events} events, {} ms",
        t0.elapsed().as_millis()
    ));
    Ok(())
}

// ── gap ledger (ADR 0003: loss only) ──────────────────────────────────────

async fn record_gap(
    pool: &PgPool,
    gap_start: DateTime<Utc>,
    gap_end: DateTime<Utc>,
    cause: &str,
) -> anyhow::Result<()> {
    sqlx::query("insert into ingest_gaps (gap_start, gap_end, cause) values ($1, $2, $3)")
        .bind(gap_start)
        .bind(gap_end)
        .bind(cause)
        .execute(pool)
        .await?;
    log(&format!(
        "GAP recorded: {} -> {} ({cause})",
        gap_start.to_rfc3339_opts(SecondsFormat::Secs, true),
        gap_end.to_rfc3339_opts(SecondsFormat::Secs, true)
    ));
    Ok(())
}

// ── archive writer (ADR 0003: daemon-owned, hourly .ndjson.gz) ────────────

async fn archive_task(mut rx: mpsc::UnboundedReceiver<String>) {
    let dir = std::path::PathBuf::from(ARCHIVE_DIR);
    let _ = std::fs::create_dir_all(&dir);
    let mut cur_hour: Option<i64> = None;
    let mut writer: Option<flate2::write::GzEncoder<std::fs::File>> = None;
    loop {
        let Some(line) = rx.recv().await else { break };
        let hour = Utc::now().timestamp().div_euclid(3600) * 3600;
        if cur_hour != Some(hour) {
            writer.take(); // close previous hour's file
            let name = DateTime::from_timestamp(hour, 0)
                .expect("valid hour")
                .format("%Y-%m-%dT%H");
            match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join(format!("{name}.ndjson.gz")))
            {
                Ok(f) => {
                    writer = Some(flate2::write::GzEncoder::new(
                        f,
                        flate2::Compression::fast(),
                    ))
                }
                Err(e) => log(&format!("archive open error: {e}")),
            }
            cur_hour = Some(hour);
            // retention: lexical compare works on ISO-prefixed names
            let cutoff = (Utc::now() - chrono::Duration::days(ARCHIVE_RETENTION_DAYS))
                .format("%Y-%m-%dT%H")
                .to_string();
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if name < cutoff {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
        }
        if let Some(w) = writer.as_mut() {
            let _ = w
                .write_all(line.as_bytes())
                .and_then(|_| w.write_all(b"\n"));
        }
    }
}

// ── main ──────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL not set (systemd EnvironmentFile)");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(15))
        .connect(&db_url)
        .await
        .context("connect to Postgres")?;
    log("connected to Postgres");

    let mut wiki_map: HashMap<String, i16> = sqlx::query("select id, server_name from wikis")
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|r| (r.get::<String, _>("server_name"), r.get::<i16, _>("id")))
        .collect();
    log(&format!("wiki cache: {} known wikis", wiki_map.len()));

    // boot resume: continue from the durable cursor via `since=` (strictly newer)
    let mut url = reqwest::Url::parse(STREAM_URL)?;
    let boot_cursor: Option<(String, DateTime<Utc>)> =
        sqlx::query_as::<_, (String, DateTime<Utc>)>(
            "select last_event_id, last_event_ts from ingest_cursor where id = true",
        )
        .fetch_optional(&pool)
        .await?;
    if let Some((id, ts)) = &boot_cursor {
        log(&format!("resuming from cursor {id} @ {}", ts.to_rfc3339()));
        url.query_pairs_mut()
            .append_pair("since", &ts.to_rfc3339_opts(SecondsFormat::Nanos, true));
    }

    let (tx_ev, mut rx_ev) = mpsc::unbounded_channel::<RcEvent>();
    let (tx_flush, mut rx_flush) = mpsc::unbounded_channel::<()>();
    let (tx_arc, rx_arc) = mpsc::unbounded_channel::<String>();
    tokio::spawn(archive_task(rx_arc));

    // consumer/flusher task: owns the buffer so the SSE loop never blocks on Postgres
    let flusher_pool = pool.clone();
    tokio::spawn(async move {
        let mut buf = Buffer::default();
        let mut ticker = tokio::time::interval(Duration::from_secs(FLUSH_EVERY_SECS));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut cool_until = Instant::now(); // backoff after a failed flush
        loop {
            tokio::select! {
                ev = rx_ev.recv() => match ev {
                    Some(ev) => {
                        if let Err(e) = buf.push(ev) { log(&format!("event error: {e}")); }
                        if buf.raws.len() >= FLUSH_EVERY_EVENTS {
                            let _ = tx_flush.send(());
                        }
                    }
                    None => break,
                },
                _ = ticker.tick() => {
                    let _ = tx_flush.send(());
                }
                _ = rx_flush.recv() => {
                    if Instant::now() < cool_until {
                        continue; // still cooling down from a failed flush
                    }
                    if let Err(e) = flush(&flusher_pool, &mut wiki_map, &mut buf).await {
                        log(&format!("FLUSH ERROR (batch lost, cursor unmoved): {e:#}"));
                        cool_until = Instant::now() + Duration::from_secs(1);
                    }
                }
            }
        }
    });

    let client = reqwest::Client::builder().user_agent(UA).build()?;
    let mut es = reqwest_eventsource::EventSource::new(client.get(url))?;

    let mut window: VecDeque<Instant> = VecDeque::new();
    let mut stats = tokio::time::interval(Duration::from_secs(30));
    let mut first_event = true;

    log(&format!("connecting to {STREAM_URL}"));
    loop {
        tokio::select! {
            ev = es.next() => match ev {
                Some(Ok(reqwest_eventsource::Event::Open)) => log("stream connected"),
                Some(Ok(reqwest_eventsource::Event::Message(m))) => {
                    // canary filter BEFORE full parse (keepalives lack `type`)
                    let envelope: Envelope = match serde_json::from_str(&m.data) {
                        Ok(env) => env,
                        Err(e) => { log(&format!("json error: {e}")); continue; }
                    };
                    if envelope.meta.domain == "canary" {
                        continue;
                    }
                    if first_event {
                        first_event = false;
                        if let Some((_, ts)) = &boot_cursor {
                            let first_ts = DateTime::parse_from_rfc3339(&envelope.meta.dt)
                                .map(|t| t.with_timezone(&Utc))
                                .ok();
                            if let Some(first_ts) = first_ts {
                                let hole_days = (first_ts - *ts).num_days();
                                if hole_days > GAP_THRESHOLD_DAYS {
                                    let _ = record_gap(&pool, *ts, first_ts, "retention-exceeded").await;
                                }
                            }
                        }
                    }
                    window.push_back(Instant::now());
                    match serde_json::from_str::<RcEvent>(&m.data) {
                        Ok(ev) => {
                            let _ = tx_arc.send(m.data); // full-fidelity archive line
                            tx_ev.send(ev)?;
                        }
                        Err(e) => log(&format!("json error: {e}")),
                    }
                }
                Some(Err(e)) => log(&format!("stream error (auto-retry): {e}")),
                None => {
                    log("stream ended; exiting so systemd restarts us");
                    return Ok(());
                }
            },
            _ = stats.tick() => {
                while let Some(t) = window.front() {
                    if t.elapsed() > Duration::from_secs(60) { window.pop_front(); } else { break; }
                }
                log(&format!(
                    "stats: ~{} events/s (60 s window), {} in window, RSS {} KB",
                    window.len() / 60,
                    window.len(),
                    rss_kb()
                ));
            }
        }
    }
}
