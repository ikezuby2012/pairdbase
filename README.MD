# PairDbase

> naming may change before public launch.

A web-based, collaborative, multi-database developer workspace with
AI-assisted querying, real-time collaboration, and cloud-native tooling.
Think **GitHub Codespaces for database engineering teams**.

No install required — pairdbase runs entirely in the browser, and
credentials never touch the client.

---

## Core Features

- **Monaco-based SQL/query editor** — syntax highlighting, schema-aware
  autocomplete, formatting, linting, multi-tab, split panes
- **Query Execution Engine** — streaming results, cancellation, timeouts,
  background jobs, EXPLAIN plan visualization
- **Schema browser** — tables, views, procedures, functions, packages,
  jobs, and indexes, normalized across every supported database into one
  consistent tree
- **AI Query Assistant** — NL→SQL, query explanation, optimization, index
  recommendations, dialect conversion, error debugging; schema-aware with
  a validation pipeline so hallucinated table/column references never
  reach the user
- **Real-time collaboration** — live cursors, shared tabs, presence,
  inline comments, pair debugging, CRDT-based conflict resolution
- **Query history** — snapshots, bookmarks, rollback, diffing, full
  execution logs
- **Security** — RBAC (org → workspace → connection → query), envelope-
  encrypted credential vault, append-only audit logs, query restriction
  rules, tenant isolation via PostgreSQL RLS
- **Analytics** — slow query dashboard, query volume trends, connection
  health, AI usage metrics

## Supported Databases

| Phase | Databases |
|---|---|
| 1 | PostgreSQL |
| 2 | MySQL, MongoDB, Redis, SQLite |
| 3 | SQL Server |
| Later | Oracle |

Each database is added by implementing one `DatabaseDriver` trait impl —
nothing else in the app changes to support a new database type.

---

## Tech Stack

**Frontend** — Next.js, React, TypeScript, Monaco Editor, TanStack Query/Table,
Zustand/Jotai, Automerge (CRDT)

**Backend** — Rust, Axum + Tokio, SQLx (Postgres/MySQL/SQLite),
official MongoDB driver, redis-rs, tiberius (SQL Server) — all pure-Rust
wire-protocol implementations; no native client libraries required except
for Oracle, which needs Oracle Instant Client on the host

**Infrastructure** — PostgreSQL (app DB + pgvector for AI semantic cache),
Redis (sessions, WS pub/sub fan-out, job queue, rate limiting),
S3-compatible object storage, Docker + Kubernetes (Helm)

**AI Layer** — tiered LLM routing (fast → balanced → powerful), schema-aware
prompting with truncation for large schemas, SQL validation pipeline
(parse check → schema existence → DML safety → injection pattern),
semantic caching via pgvector

---

## Architecture

Feature-based clean architecture. Each feature is self-contained:

```
domain.rs      ← entities + repo traits (pure, no I/O, no framework)
use_cases.rs   ← business logic (depends only on repo traits — testable without a DB)
repository.rs  ← SQLx/driver implementation of the repo trait
handler.rs     ← Axum handlers (call use_cases, never repository directly)
mod.rs         ← wires the feature together, exports router()
```

**Dependency rule:** `handler → use_cases → domain trait ← repository`.
Axum never appears outside `handler.rs`; SQLx never appears outside
`repository.rs`.

### Workspace layout

```
pairdbase/
├── Cargo.toml                 ← workspace root, pins shared dep versions
├── crates/
│   ├── shared/                ← zero framework deps: IDs, DomainError
│   │   └── src/
│   │       ├── ids.rs
│   │       ├── errors.rs
│   │       └── lib.rs
│   │
│   └── app/
│       └── src/
│           ├── main.rs        ← loads AppConfig, builds AppState, starts axum::serve
│           ├── config.rs      ← AppConfig — the only place std::env::var appears
│           ├── state.rs       ← AppState, shared across every feature
│           ├── router.rs      ← merges each feature's router(), stays thin
│           │
│           └── features/
│               ├── auth/
│               ├── connections/   ← create(), test(), get_schema()
│               ├── query/         ← execute(), cancel(), history()
│               │   └── drivers/   ← DatabaseDriver trait + one impl per DB
│               ├── workspace/
│               ├── ai/
│               └── analytics/
```

`connections` borrows the `DatabaseDriver` capability from `query::drivers`
and `query::engine` (one-directional — `query` never depends back on
`connections`). Everything else is feature-isolated.

---

## Getting Started

### Prerequisites

- Rust (stable, 2021 edition)
- PostgreSQL (for the app database — separate from any database a user connects to)
- Node.js + npm/pnpm (frontend, not included in this backend workspace)

### Environment variables

`AppConfig::from_env()` reads the following at startup:

| Variable | Required | Default |
|---|---|---|
| `APP_DATABASE_URL` | yes | — |
| `BIND_ADDR` | no | `0.0.0.0:8080` |
| `JWT_SECRET` | yes | — |
| `JWT_EXPIRY_MINUTES` | no | `60` |
| `VAULT_KMS_KEY_ID` | yes | — |
| `AI_PROVIDER_API_KEY` | yes | — |
| `AI_TIER1_MODEL` | no | `claude-haiku-4-5` |
| `AI_TIER2_MODEL` | no | `claude-sonnet-5` |
| `AI_TIER3_MODEL` | no | `claude-opus-5` |
| `CORS_ALLOWED_ORIGINS` | no | *(empty)* — comma-separated |

### Run locally

```bash
# from the workspace root
export APP_DATABASE_URL="postgres://user:pass@localhost:5432/pairdbase"
export JWT_SECRET="dev-secret-change-me"
export VAULT_KMS_KEY_ID="dev-kms-key"
export AI_PROVIDER_API_KEY="sk-..."

cargo run --bin pairdbase
```

### Run checks

```bash
cargo check --workspace
cargo test --workspace
```

---

## Roadmap

| Phase | Timeline | Scope |
|---|---|---|
| 1 | 0–3 mo | PostgreSQL only, Monaco editor, query execution + history, schema browser, email/password auth, basic RBAC, Docker Compose |
| 2 | 3–6 mo | MySQL, MongoDB, Redis, SQLite; real-time collaboration; query snapshots; OAuth2; enhanced RBAC; CSV export; Kubernetes Helm chart |
| 3 | 6–9 mo | Full AI assistant pipeline; EXPLAIN visualizer; analytics dashboard; credential vault + KMS; audit logs; background jobs |
| 4 | 9–12 mo | SSO/SAML; SOC 2 prep; SQL Server; query scheduling; ER diagram; webhooks; on-prem deployment guide |

---

## Key Engineering Decisions

- **Rust for the entire backend** — no GC pauses (matters for streaming
  results), memory-safe concurrency
- **Feature-based clean architecture** — adding a feature touches one
  folder, not five crates; use cases are unit-testable without a database
- **Browser-native only** — no Electron; credentials never reach the client
- **Mandatory AI validation pipeline** — hallucinated table/column
  references are caught and corrected before reaching the user
- **Redis pub/sub for WebSocket fan-out** — horizontal scaling without
  sticky sessions
- **Append-only audit log** — DELETE/UPDATE revoked at the database role
  level, not just the application level