# rust-1M-rps

A high-performance Rust web service for persisting messages, built with **Axum** + **Redis Streams** + **SQLx (PostgreSQL)** and structured around a clean, layered architecture. The project is a load-test target aimed at measuring throughput (up to ~1M requests/sec).

_This is the Rust implementation of [node-1m-rps](https://github.com/agile8118/node-1m-rps) by [@agile8118](https://github.com/agile8118)._

## Features

- Asynchronous HTTP server with [Axum](https://github.com/tokio-rs/axum) (HTTP/2, macros, WebSocket ready)
- **Redis Streams** as a high-throughput message broker between the API and the database
- **Background sync worker** that reads from Redis Streams and batch-inserts into PostgreSQL
- PostgreSQL persistence with [SQLx](https://github.com/launchbadge/sqlx) (compile-time-checked queries via `query_as`)
- Layered design: `handler -> service -> repository -> entity`
- Request validation with [validator](https://github.com/Keats/validator)
- Ergonomic error handling with [thiserror](https://github.com/dtolnay/thiserror), mapped to proper HTTP status codes
- UUID primary keys generated server-side
- SQLx CLI migrations (no in-code migration side effects)

## Tech Stack

| Concern        | Crate                                                          |
| -------------- | -------------------------------------------------------------- |
| Web framework  | `axum 0.8`                                                     |
| Runtime        | `tokio`                                                        |
| Database       | `sqlx 0.9` (Postgres, `time`, `uuid`)                          |
| Message broker | `redis 1.6` (async, connection manager, stream groups)         |
| Validation     | `validator 0.20`                                               |
| Errors         | `thiserror 2.0`                                                |
| IDs / time     | `uuid 1.24`, `time 0.3`                                        |
| Env loading    | `dotenv`                                                       |

## Architecture

The project uses a **decoupled write path**: the HTTP server writes messages to a Redis Stream, and a background `sync` worker consumes from that stream and persists to PostgreSQL in batches.

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
                                              | INSERT ...
                                              v
                                        +------------------+
                                        | PostgreSQL       |
                                        +------------------+
```

### Workspace Members

| Crate    | Purpose                                    |
| -------- | ------------------------------------------ |
| `server` | Axum HTTP API — validates and publishes    |
| `sync`   | Background consumer — batch-persists       |
| `shared` | Common config, errors, and message types   |

### Server Layout

```
Request -> routes -> handler -> service -> repository -> Redis Stream
                                    \- entity (domain) / dto (transport)
```

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

### Sync Worker Layout

```
sync/src/
├── consumer/
│   ├── mod.rs                 # Module declaration
│   └── message_consumer.rs    # Redis read -> parse -> batch insert -> ACK
└── main.rs                    # Entry point (Redis + DB init, consumer loop)
```

The consumer reads messages in batches (up to 500), deserializes them, inserts valid messages into PostgreSQL via `sqlx::QueryBuilder::push_values(...)` with `ON CONFLICT (id) DO NOTHING`, and ACKs the Redis entries only after a successful insert.

## Prerequisites

- Rust (stable) — https://rustup.rs
- Docker (for Postgres + Redis)
- `sqlx-cli` for migrations:
  ```bash
  cargo install sqlx-cli --no-default-features --features postgres
  ```

## Getting Started

### 1. Start Postgres

```bash
docker run -d --name message_db -p 5432:5432 \
  -e POSTGRES_PASSWORD=mysecretpassword \
  -e POSTGRES_USER=dbuser \
  -e POSTGRES_DB=message \
  postgres:17
```

### 2. Start Redis

```bash
docker run -d --name message_redis -p 6379:6379 redis:7
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

You can run multiple sync workers with different `CONSUMER_NAME` values for parallel consumption (Redis handles load-balancing across the consumer group).

## API Reference

### `POST /message`

Create a new message. The message is published to the Redis Stream `messages:stream` and later persisted to PostgreSQL by the sync worker.

**Request**

```http
POST /message HTTP/1.1
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
| `message` | string | length 1--1000 chars |

**Success response** -- `201 Created`

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

## Load Testing

A Postman collection (`postman_collection.json`) is included for manual checks. For throughput benchmarking, use [autocannon](https://github.com/mcollina/autocannon):

```bash
# install once
npm i -g autocannon

# standard run
autocannon -m POST \
  --connections 10 --duration 20 --pipelining 2 \
  -H "Content-Type: application/json" \
  -b '{"from":"alice","to":"bob","message":"Hello from Postman!"}' \
  "http://127.0.0.1:3000/message"

```

## Notes

- **Why Redis Streams?**
  Writing directly to PostgreSQL on every HTTP request creates lock contention and I/O bottlenecks under extreme load. By publishing to Redis Streams first, the HTTP path remains fast (in-memory, append-only log), while the `sync` worker absorbs database write pressure via efficient batch inserts.

- **Batch insert behavior.**
  The sync worker reads up to 500 messages per Redis `XREADGROUP` call, then inserts them into PostgreSQL in a single query via `sqlx::QueryBuilder::push_values(...)`. Invalid / poison messages (bad JSON, missing `data` field) are ACKed immediately so they don't block the stream. If a batch insert fails, none of the messages in that batch are ACKed -- Redis redelivers them on the next read.

- **Idempotency.**
  Messages use server-generated UUIDs. The batch insert query includes `ON CONFLICT (id) DO NOTHING`, so duplicate deliveries (e.g., after a crash before ACK) are silently safe.

- **Why this Rust port needs no PM2-style clustering (unlike the Node original).**
  Node.js is **single-threaded per process** -- one event loop runs on one OS thread, so a single Node process can only ever use **one CPU core**. The Node original therefore *must* run under PM2 with `instances: "max"` and `exec_mode: "cluster"` (`ecosystem.config.cjs`) to fork one worker process per core and share the listening port via the OS cluster module. That clustering is not an optimization there -- it is the only way Node touches more than one core.

  This Rust port does not need any of that. The runtime is configured at `src/main.rs` with the `#[tokio::main]` macro plus `tokio = { features = ["full"] }` in `Cargo.toml`. The `"full"` feature pulls in `rt-multi-thread`, so `#[tokio::main]` expands to `tokio::runtime::Builder::new_multi_thread().enable_all().build()` -- a **multi-thread work-stealing scheduler** with **one worker thread per logical CPU core** (default = `std::thread::available_parallelism()`). When `axum::serve` accepts a connection it becomes a tokio *task*, and tokio spreads tasks across all N worker threads (stealing work between them) **all inside a single process**. In other words: the parallelism the Node author obtains through N OS processes, Rust obtains through N OS threads in one process, automatically. That is why CPU utilization reaches ~99% (0--1% idle) here with **zero clustering code**.

  |                       | Node (`node-1m-rps`)              | Rust (this repo)                         |
  | --------------------- | --------------------------------- | ---------------------------------------- |
  | Threads per process   | 1 (single event loop)             | N (= logical CPU cores)                  |
  | Uses all cores        | only via PM2 `instances: "max"`    | automatic via tokio `rt-multi-thread`    |
  | Clustering needed?    | **yes** (process-per-core)        | **no** (thread-per-core in one process)  |

  Caveat: a single `TcpListener` + single accept loop means request *handling* is parallel across cores but connection *acceptance* is serialized on one task. With keep-alive + pipelining on a modest number of connections (as in the load test below) this is fine. PM2's N processes each get their own listener (via `SO_REUSEPORT`), so Node sidesteps the single-acceptor ceiling -- if that ever becomes the bottleneck here, `SO_REUSEPORT` + multiple processes would be the equivalent fix, but it is **not** required for basic core utilization.

- **Migrations are CLI-only.** The app does *not* run migrations on startup -- always run `sqlx migrate run` after starting a fresh database container.
- **Persist Postgres data across containers** by using a named volume, otherwise each new container starts empty:
  ```bash
  docker run -d --name message_db -p 5432:5432 \
    -e POSTGRES_PASSWORD=mysecretpassword \
    -e POSTGRES_USER=dbuser \
    -e POSTGRES_DB=message \
    -v message_pgdata:/var/lib/postgresql/data \
    postgres:17
  ```
  Then reuse the same container with `docker start message_db` / `docker stop message_db`.

- **Persist Redis data across containers** similarly:
  ```bash
  docker run -d --name message_redis -p 6379:6379 \
    -v message_redisdata:/data \
    redis:7 redis-server --appendonly yes
  ```
