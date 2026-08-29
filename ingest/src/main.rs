//! wikistream-ingest — spike (ticket 06)
//!
//! Consumes Wikimedia EventStreams `recentchange` (SSE), batches events every
//! 5 s / 256 events, and commits each batch as ONE Postgres transaction:
//! raws (6 h window) + minute-rollup upserts (counters + p=12 HLL sketch +
//! daemon-computed estimate) + cursor advance + ledger row. Crash mid-batch
//! rolls back atomically — no double-counting by construction (ADR 0002).
//! Hour/day merges are deferred to the real daemon (ticket 07/08).
//!
//! Reconnects: reqwest-eventsource re-sends Last-Event-ID automatically
//! (the server kills every connection after 15 min — this is the normal
//! path, not an edge case). On boot, resumes from the durable cursor via
//! `since=`. systemd restarts us on exit (Restart=always).

use std::collections::{HashMap, VecDeque};
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
const UA: &str = "wikistream-ingest/0.1 (spike; portfolio project; contact golam.tamim94@gmail.com)";

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

#[derive(serde::Deserialize)]
struct Meta {
    id: String,
    dt: String,
    #[serde(default)]
    domain: String,
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
    buckets: HashMap<i64, BucketAcc>, // key: minute epoch (UTC-aligned)
    raws: Vec<RawRow>,
    last_event_id: String,
    last_event_ts: DateTime<Utc>,
    seen: u64, // events since boot (stats only)
}

impl Buffer {
    fn push(&mut self, ev: RcEvent) -> anyhow::Result<()> {
        let ts = DateTime::parse_from_rfc3339(&ev.meta.dt)
            .with_context(|| format!("bad meta.dt {:?}", ev.meta.dt))?
            .with_timezone(&Utc);
        self.last_event_id = ev.meta.id.clone();
        self.last_event_ts = ts;

        // raw window: every event type (audit + re-derivation), counting excluded
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
            let minute = ts.timestamp().div_euclid(60) * 60;
            let acc = self.buckets.entry(minute).or_default();
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
        self.seen += 1;
        Ok(())
    }
}

async fn flush(pool: &PgPool, wiki_map: &mut HashMap<String, i16>, buf: &mut Buffer) -> anyhow::Result<()> {
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

    // minute rollups: counters + merged sketch + estimate (totals rows only)
    for (minute, acc) in buf.buckets.iter_mut() {
        // count(&mut self): may merge sparse registers into normal form first
        let est = acc.sketch.as_mut().map(|s| s.count() as i64);
        let sk: Vec<u8> = match acc.sketch.as_ref() {
            Some(s) => bincode::serialize(s)?,
            None => Vec::new(),
        };
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
        .bind(DateTime::from_timestamp(*minute, 0).expect("valid minute"))
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
    let buckets = buf.buckets.len();
    buf.raws.clear();
    buf.buckets.clear();
    log(&format!(
        "batch: {events} events, {buckets} bucket(s), {} ms",
        t0.elapsed().as_millis()
    ));
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set (systemd EnvironmentFile)");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(15))
        .connect(&db_url)
        .await
        .context("connect to Postgres")?;
    log("connected to Postgres");

    // wiki id cache: warm from DB, extend on first sight
    let mut wiki_map: HashMap<String, i16> =
        sqlx::query("select id, server_name from wikis")
            .fetch_all(&pool)
            .await?
            .into_iter()
            .map(|r| (r.get::<String, _>("server_name"), r.get::<i16, _>("id")))
            .collect();
    log(&format!("wiki cache: {} known wikis", wiki_map.len()));

    // boot resume: continue from the durable cursor via `since=` (ticket 07 refines)
    let mut url = reqwest::Url::parse(STREAM_URL)?;
    if let Some((id, ts)) = sqlx::query_as::<_, (String, DateTime<Utc>)>(
        "select last_event_id, last_event_ts from ingest_cursor where id = true",
    )
    .fetch_optional(&pool)
    .await?
    {
        log(&format!("resuming from cursor {id} @ {}", ts.to_rfc3339()));
        url.query_pairs_mut()
            .append_pair("since", &ts.to_rfc3339_opts(SecondsFormat::Nanos, true));
    }

    let client = reqwest::Client::builder().user_agent(UA).build()?;
    let mut es = reqwest_eventsource::EventSource::new(client.get(url))?;

    let (tx_ev, mut rx_ev) = mpsc::unbounded_channel::<RcEvent>();
    let (tx_flush, mut rx_flush) = mpsc::unbounded_channel::<()>();

    // consumer/flusher task: owns the buffer so the SSE loop never blocks on Postgres
    let flusher_pool = pool.clone();
    tokio::spawn(async move {
        let mut buf = Buffer::default();
        let mut ticker = tokio::time::interval(Duration::from_secs(FLUSH_EVERY_SECS));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                ev = rx_ev.recv() => match ev {
                    Some(ev) => {
                        if let Err(e) = buf.push(ev) { log(&format!("event parse error: {e}")); }
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
                    if let Err(e) = flush(&flusher_pool, &mut wiki_map, &mut buf).await {
                        log(&format!("FLUSH ERROR (batch lost, cursor unmoved): {e:#}"));
                    }
                }
            }
        }
    });

    // stats: events/s over trailing 60 s + RSS, to journald every 30 s
    let mut window: VecDeque<Instant> = VecDeque::new();
    let mut stats = tokio::time::interval(Duration::from_secs(30));

    log(&format!("connecting to {STREAM_URL}"));
    loop {
        tokio::select! {
            ev = es.next() => match ev {
                Some(Ok(reqwest_eventsource::Event::Open)) => log("stream connected"),
                Some(Ok(reqwest_eventsource::Event::Message(m))) => {
                    window.push_back(Instant::now());
                    match serde_json::from_str::<RcEvent>(&m.data) {
                        Ok(ev) => {
                            if ev.meta.domain != "canary" {
                                tx_ev.send(ev)?;
                            }
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
