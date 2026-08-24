# From 5.7k to 56k RPS: How I Made Rust Say Yes to a Million Requests

> A story about one endpoint, two designs, and a CPU graph that refused to lie to me.

This is a Rust web service that accepts messages over HTTP and persists them. Simple, right? That's what I thought too. But somewhere between "just write it to Postgres" and serving **1.12 million requests in a 20-second load test**, I learned a lot about where throughput actually goes to die.

This project is my Rust implementation of [node-1m-rps](https://github.com/agile8118/node-1m-rps) by [@agile8118](https://github.com/agile8118) — rebuilt with **Axum + Redis Streams + SQLx (PostgreSQL)**, and benchmarked every step of the way with [autocannon](https://github.com/mcollina/autocannon).

If you just want to run it, jump to [Run it yourself](#run-it-yourself). Otherwise, grab a coffee — this is how it went.

---

## The Setup

The requirements were honest and small:

- `POST` a message (`from`, `to`, `message`)
- Persist it to PostgreSQL
- Return a `201` with the created message

The stack:

| Concern        | Choice                                        |
| -------------- | --------------------------------------------- |
| Web framework  | `axum 0.8`                                    |
| Runtime        | `tokio` (multi-thread, work-stealing)         |
| Database       | `sqlx 0.9` + PostgreSQL 17                    |
| Message broker | `redis 1.6` (Streams + consumer groups)       |
| Validation     | `validator 0.20`                              |
| Errors         | `thiserror 2.0`                               |

A layered structure, because future-me deserves clean code too:

```
Request -> routes -> handler -> service -> repository -> ???
                              \- entity (domain) / dto (transport)
```

That `???` at the end of the pipeline is where this whole story lives.

---

## Act 1: The Naive Version — Write Straight to Postgres

First version: HTTP request comes in, we validate it, run an `INSERT`, return. One database, zero moving parts.

```rust
pub async fn create_message_direct(
    &self,
    payload: MessageCreateDto,
) -> Result<MessageReadDto, ApiError> {
    let message = Message {
        id: Uuid::new_v4(),
        from: payload.from_,
        to: payload.to,
        message: payload.message,
        created_at: OffsetDateTime::now_utc(),
    };

    sqlx::query(
        r#"INSERT INTO messages (id, "from", "to", message, created_at)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(message.id)
    .bind(&message.from)
    .bind(&message.to)
    .bind(&message.message)
    .bind(message.created_at)
    .execute(&self.pg_pool)
    .await?;

    Ok(MessageReadDto::from(message))
}
```

Clean. Readable. Correct. And — spoiler — **10x slower than what I ended up with**.

### Enter autocannon

I fired up [autocannon](https://github.com/mcollina/autocannon) to see what this thing could actually do:

```bash
autocannon -m POST \
  --connections 100 --duration 20 --pipelining 20 \
  -H "Content-Type: application/json" \
  -b '{"from":"alice","to":"bob","message":"Hello from Postman!"}' \
  "http://127.0.0.1:3000/message"
```

100 connections, 20x pipelining, 20 seconds. Here's what came back:

![Autocannon results — direct Postgres writes](public/before.png)

| Metric           | Value                          |
| ---------------- | ------------------------------ |
| Avg latency      | **345.77 ms** (p50: 342 ms)    |
| p99 latency      | 470 ms                         |
| Avg throughput   | **~5,727 req/sec**             |
| Total requests   | 117k in 20s                    |

~5.7k RPS and a third of a second of latency per request. For a Rust server on a modern machine, that's... suspicious. Rust isn't slow, so something else was.

---

## Act 2: The CPU Graph That Refused to Lie

I opened the core utilization graph while the test was running, and this is what I saw:

![CPU core usage — direct Postgres writes](public/cpu_before.png)

**50–55% idle. On every core. Under full load.**

Half my CPU was sitting there doing absolutely nothing while my server was allegedly getting hammered.

And that was the clue. Tokio's multi-thread runtime already spreads work across one worker thread per logical core — the machinery was there. The problem was what those threads were *doing*: each request handler `await`s a Postgres `INSERT`. While it waits on the database (disk flush, WAL, connection pool checkout, network round-trip), the request is parked. Work is *available* — 100 connections × 20 pipelined requests deep — but every single one of them funnels through synchronous DB writes, one at a time, per request.

The server wasn't CPU-bound. It was **database-round-trip-bound**. Postgres is a fantastic database, but it is not an in-memory buffer, and asking it to durably store 100+ individual rows per second *per connection* turns your fancy async runtime into a very expensive waiting room.

So the question became: **how do I keep the CPU fed and stop making every request pay the disk-flush tax?**

---

## Act 3: Put Redis in the Hot Path

The insight: an HTTP `201` doesn't require the row to be *in Postgres* yet. It requires the message to be *safely accepted*. So I split the write path:

1. **Hot path:** HTTP handler → `XADD` to a Redis Stream → return `201`. Redis is in-memory and append-only; this is microseconds, not milliseconds.
2. **Cold path:** a background worker consumes the stream and batch-inserts into Postgres at its own pace.

```
HTTP Request
    |
    v
+--------+     XADD (Redis Stream)     +------------------+
| Server | ----------------------------> | Redis: messages  |
|        |                               | :stream          |
+--------+                               +------------------+
                                               |
                                               | XREADGROUP
                                               v
                                         +------------------+
                                         | Sync Worker      |
                                         | (batch insert)   |
                                         +------------------+
                                               |
                                               | INSERT ... 500 rows
                                               v
                                         +------------------+
                                         | PostgreSQL       |
                                         +------------------+
```

### The hot path: one XADD, done

The repository layer changed from "talk to Postgres" to "append to a stream":

```rust
async fn create(&self, payload: MessageCreateDto) -> Result<Message, DbError> {
    let message = Message {
        id: Uuid::new_v4(),
        from: payload.from_,
        to: payload.to,
        message: payload.message,
        created_at: OffsetDateTime::now_utc(),
    };

    let data = serde_json::to_string(&message)
        .map_err(|e| DbError::SomethingWentWrong(e.to_string()))?;

    let mut conn = self.db_conn.get_connection();

    let _ = conn
        .xgroup_create_mkstream(Message::STREAM, Message::GROUP, "$")
        .await;

    let _: Option<String> = conn
        .xadd(Message::STREAM, "*", &[("data", data.as_str())])
        .await
        .map_err(|e| DbError::SomethingWentWrong(e.to_string()))?;

    Ok(message)
}
```

(That `xgroup_create_mkstream` call on every write looks odd, but it's idempotent — it just guarantees the consumer group exists before the worker ever reads. Errors are ignored on purpose.)

I kept the old direct-write endpoint at `POST /message` and exposed the new path as `POST /message-fast` — same validation, same response shape, so I could A/B them against each other:

```rust
pub fn route() -> Router<MessageState> {
    Router::new()
        .route("/message-fast", post(message_fast_handler))
        .route("/message", post(message_direct_handler))
}
```

### The cold path: batch like you mean it

The `sync` worker is a separate binary that loops forever:

- **`XREADGROUP`** with `count(500)` and `block(300ms)` — pull up to 500 messages per read
- **one bulk `INSERT`** for the whole batch via `sqlx::QueryBuilder::push_values`
- **`XACK`** only *after* a successful insert

```rust
async fn insert_batch(&self, entries: &[StreamEntry]) -> Result<(), sqlx::Error> {
    let mut qb =
        QueryBuilder::new(r#"INSERT INTO messages (id, "from", "to", message, created_at) "#);

    qb.push_values(entries, |mut b, entry| {
        b.push_bind(entry.message.id)
            .push_bind(&entry.message.from)
            .push_bind(&entry.message.to)
            .push_bind(&entry.message.message)
            .push_bind(entry.message.created_at);
    });

    qb.push(" ON CONFLICT (id) DO NOTHING");

    qb.build().execute(&self.db_pool).await?;
    Ok(())
}
```

Instead of 500 round-trips, Postgres now handles 500 rows in **one** statement. The database gets durable writes at a pace it likes, and the message stays safe in Redis the entire time — if the batch insert fails, the worker doesn't ACK, and Redis simply redelivers those entries on the next read. Poison messages (bad JSON, missing fields) get ACKed immediately so one malformed entry can't clog the stream.

And because every message carries a server-generated UUID plus `ON CONFLICT (id) DO NOTHING`, a crash-between-insert-and-ACK scenario — the classic at-least-once duplication — is silently absorbed. At-least-once delivery becomes effectively exactly-once persistence.

---

## Act 4: The Numbers

Same machine. Same autocannon flags. New endpoint:

```bash
autocannon -m POST \
  --connections 100 --duration 20 --pipelining 20 \
  -H "Content-Type: application/json" \
  -b '{"from":"alice","to":"bob","message":"Hello from Postman!"}' \
  "http://127.0.0.1:3000/message-fast"
```

![Autocannon results — Redis Streams write path](public/after.png)

| Metric           | Before (direct PG)             | After (Redis stream)           | Δ            |
| ---------------- | ------------------------------ | ------------------------------ | ------------ |
| Avg latency      | 345.77 ms                      | **35.25 ms**                   | **~10x**     |
| p99 latency      | 470 ms                         | 41 ms                          | ~11x         |
| Avg throughput   | ~5,727 req/sec                 | **~55,899 req/sec**            | **~10x**     |
| Total requests   | 117k in 20s                    | **1.12M in 20s**               | ~10x         |
| Data transferred | 29.8 MB                        | 291 MB                         | ~10x         |

And the part I was most curious about — the CPU graph:

![CPU core usage — Redis Streams write path](public/cpu_after.png)

**Idle dropped from 50–55% to 6–7%.** The cores that were previously napping while handlers waited on Postgres are now actually processing requests. Same runtime, same thread count, same machine — the only change was removing the disk round-trip from the request path. The throughput gain didn't come from working *harder*; it came from stopping the CPU from *waiting*.

### Why no PM2-style clustering?

The original Node project needs PM2 with `instances: "max"` and `exec_mode: "cluster"` — because Node is single-threaded per process, and forking one process per core is the *only* way it can use more than one core.

Rust needs none of that here. `#[tokio::main]` with `features = ["full"]` gives you a multi-thread work-stealing scheduler with one worker thread per logical core, inside a single process. Connections become tokio tasks and get spread across all workers automatically.

|                       | Node (`node-1m-rps`)              | Rust (this repo)                         |
| --------------------- | --------------------------------- | ---------------------------------------- |
| Threads per process  | 1 (single event loop)             | N (= logical CPU cores)                  |
| Uses all cores       | only via PM2 `instances: "max"`   | automatic via tokio `rt-multi-thread`    |
| Clustering needed?   | **yes** (process-per-core)        | **no** (thread-per-core in one process)  |

Honest caveat: with a single `TcpListener`, connection *acceptance* is serialized on one task even though request *handling* is parallel. With keep-alive + pipelining on a modest number of connections, that's a non-issue. If it ever becomes the ceiling, `SO_REUSEPORT` + multiple processes is the equivalent fix — PM2 just does that trick by default.

---

## What I Took Away From This

1. **Measure before you optimize.** I assumed the bottleneck was "Rust + JSON parsing" or something glamorous. The CPU graph said: nope, you're I/O-bound, half the machine is idle.
2. **"Slow" is often just "waiting."** The 10x didn't come from making anything faster — it came from taking a disk-flip out of the request's critical path.
3. **Buffering writes isn't cheating durability.** Redis Streams + consumer groups + ACK-after-insert + idempotent batch inserts means nothing is lost, and a crash is a redelivery, not data loss.
4. **Boring architecture, fast numbers.** The winning design is a textbook decoupled write path — it just took me a load test and an idle CPU graph to actually respect it.

## What's Next

The natural follow-ups, in the order I'll probably attack them:

- **Dead-letter stream** — the `messages:dead` constant already exists; the worker just doesn't publish to it yet
- **`SO_REUSEPORT` + multiple acceptors** if the single-acceptor ceiling ever shows up in a profile
- Tuning `XREADGROUP` batch size / block time against different Postgres configs
- Running the whole thing on real hardware instead of my laptop, where "1M rps" stops being "1M requests per 20-second run" and starts being per second 🙂

---

## Run It Yourself

### Prerequisites

- Rust (stable) — https://rustup.rs
- Docker (for Postgres + Redis)
- `sqlx-cli` for migrations:

  ```bash
  cargo install sqlx-cli --no-default-features --features postgres
  ```

### 1. Start Postgres

```bash
docker run -d --name message_db -p 5432:5432 \
  -e POSTGRES_PASSWORD=mysecretpassword \
  -e POSTGRES_USER=dbuser \
  -e POSTGRES_DB=message \
  -v message_pgdata:/var/lib/postgresql/data \
  postgres:17
```

### 2. Start Redis

```bash
docker run -d --name message_redis -p 6379:6379 \
  -v message_redisdata:/data \
  redis:7 redis-server --appendonly yes
```

### 3. Configure environment

Create a `.env` in the project root (already gitignored):

```env
DATABASE_URL=postgres://dbuser:mysecretpassword@localhost:5432/message
REDIS_URL=redis://127.0.0.1:6379/
APP_URL=127.0.0.1
APP_PORT=3000
CONSUMER_NAME=sync-worker-1
```

> `CONSUMER_NAME` must be unique per running sync instance (used as the Redis consumer group member ID).

### 4. Run migrations (CLI only)

```bash
sqlx migrate run      # apply pending migrations
sqlx migrate info     # inspect applied/pending state
sqlx migrate revert   # undo the last migration
```

Migration applied here (`migrations/20260820104454_messages.up.sql`):

```sql
create table if not exists messages (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "from"   TEXT NOT NULL,
  "to"     TEXT NOT NULL,
  message  TEXT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT now()
);
```

### 5. Build & run

**Start the HTTP server:**

```bash
cargo run -p server
# Server starting at 127.0.0.1:3000
```

**Start the sync worker (in another terminal):**

```bash
cargo run -p sync
# Consumer 'sync-worker-1' started on stream 'messages:stream'
```

Run multiple sync workers with different `CONSUMER_NAME` values for parallel consumption — Redis load-balances across the consumer group.

### 6. Benchmark it

```bash
# install once
npm i -g autocannon

# the fast path (Redis Streams + sync worker)
autocannon -m POST \
  --connections 100 --duration 20 --pipelining 20 \
  -H "Content-Type: application/json" \
  -b '{"from":"alice","to":"bob","message":"Hello from Postman!"}' \
  "http://127.0.0.1:3000/message-fast"

# the old direct-write path, for comparison
autocannon -m POST \
  --connections 100 --duration 20 --pipelining 20 \
  -H "Content-Type: application/json" \
  -b '{"from":"alice","to":"bob","message":"Hello from Postman!"}' \
  "http://127.0.0.1:3000/message"
```

---

## API Reference

### `POST /message-fast`

Create a message via the fast path — published to the Redis Stream `messages:stream`, persisted to PostgreSQL asynchronously by the sync worker.

### `POST /message`

Create a message via the direct path — synchronous `INSERT` into PostgreSQL, kept around for A/B comparison.

**Request**

```http
POST /message-fast HTTP/1.1
Content-Type: application/json

{
  "from": "alice",
  "to": "bob",
  "message": "Hello from Postman!"
}
```

| Field     | Type   | Rule                  |
| --------- | ------ | --------------------- |
| `from`    | string | required              |
| `to`      | string | required              |
| `message` | string | length 1--1000 chars  |

**Success response** — `201 Created`

```json
{
  "id": "0f1c2a7e-4b3d-4f9a-9c1b-2d3e4f5a6b7c",
  "from": "alice",
  "to": "bob",
  "message": "Hello from Postman!",
  "created_at": "2026-08-21T10:00:00Z"
}
```

**Error responses**

```json
{ "message": "<details>", "code": 400 }
```

| Status | Cause                       |
| ------ | --------------------------- |
| 400    | Validation failed           |
| 409    | Unique constraint violation |
| 500    | Other database error        |

---

## Workspace Layout

| Crate    | Purpose                                  |
| -------- | ---------------------------------------- |
| `server` | Axum HTTP API — validates and publishes  |
| `sync`   | Background consumer — batch-persists     |
| `shared` | Common config, errors, and message types |

```
server/src/
├── config/        # DB connection + env parameters
├── dto/           # Request/Response DTOs (MessageCreateDto, MessageReadDto)
├── entity/        # Domain entity (Message)
├── error/         # ApiError, DbError -> HTTP status mapping
├── handler/       # Axum handlers (validation -> service call)
├── repository/    # Redis data access (MessageRepository)
├── response/      # JSON error envelope (ApiErrorResponse)
├── routes/        # Router + state wiring
├── service/       # Business logic (MessageService)
└── state/         # Per-feature app state (MessageState)
```

---

_This is the Rust implementation of [node-1m-rps](https://github.com/agile8118/node-1m-rps) by [@agile8118](https://github.com/agile8118). Benchmarks run with [autocannon](https://github.com/mcollina/autocannon). Thanks for reading — now go check your CPU idle graph._
