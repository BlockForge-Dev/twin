# CylinderSense

**Smart LPG depletion monitoring for commercial operators.**

CylinderSense is a connected monitoring system that tells commercial kitchens,
restaurants, and other LPG-dependent businesses how much gas remains in their
active cylinder — before operations are disrupted.

The system consists of a hardware sensing device (ESP32 + load cell), a
backend service (Rust/Axum + PostgreSQL), and a thin app interface. The
backend is the brain: it ingests telemetry, estimates remaining gas, and
triggers low/critical alerts.

> **Current status**: Milestone 0 — Domain model frozen, project skeleton
> built, health endpoint live.

## Architecture

See [DESIGN.md](DESIGN.md) for the full architecture overview, data model
diagram, design decisions, and v1 non-goals.

## Project Structure

```
twin/
├── Cargo.toml                  # Workspace root
├── DESIGN.md                   # Architecture & design decisions
├── docker-compose.yml          # Local Postgres for development
├── .env.example                # Environment variable template
├── crates/
│   ├── core/                   # Shared domain types (no I/O)
│   │   └── src/
│   │       ├── models.rs       # Device, TelemetryRaw, RefillRecord, CurrentState, AlertEvent
│   │       ├── payloads.rs     # TelemetryPayload (device→backend contract)
│   │       └── error.rs        # AppError enum
│   ├── ingest/                 # Axum HTTP service
│   │   ├── src/
│   │   │   ├── main.rs         # Server entrypoint
│   │   │   ├── config.rs       # AppConfig (env-based)
│   │   │   ├── routes/         # HTTP handlers
│   │   │   └── db/             # Database pool setup
│   │   └── migrations/         # SQL migration files (5 tables)
│   ├── engine/                 # State estimation & alert rules (stub)
│   └── simulator/              # Device telemetry simulator (stub)
└── web/                        # Future UI app
```

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Docker](https://docs.docker.com/get-docker/) and Docker Compose
- [sqlx-cli](https://crates.io/crates/sqlx-cli) (optional, for manual migration commands)

  ```bash
  cargo install sqlx-cli --no-default-features --features postgres
  ```

## Setup

### 1. Start PostgreSQL

```bash
docker compose up -d
```

This starts a Postgres 16 instance on `localhost:5432` with database
`cylindersense`, user `cs_dev`, password `cs_dev_pass`.

### 2. Set Environment Variables

Copy the example env file and adjust if needed:

```bash
cp .env.example .env
```

Or export directly:

```bash
export DATABASE_URL=postgres://cs_dev:cs_dev_pass@localhost:5432/cylindersense
```

On PowerShell:

```powershell
$env:DATABASE_URL = "postgres://cs_dev:cs_dev_pass@localhost:5432/cylindersense"
```

### 3. Build the Workspace

```bash
cargo build --workspace
```

### 4. Run the Ingest Service

```bash
cargo run -p cylindersense-ingest
```

The server starts on `http://localhost:3000`. Database migrations run
automatically on startup.

### 5. Verify

```bash
curl http://localhost:3000/health
# → {"status":"ok"}
```

## Database Tables

The following tables are created by the migrations:

| Table | Purpose |
|-------|---------|
| `devices` | Registered physical monitoring units |
| `telemetry_raw` | Append-only raw sensor readings |
| `refill_records` | Refill context per depletion cycle |
| `current_state` | Derived operational state (one row per device) |
| `alert_events` | Notification-worthy state transitions |

## License

Proprietary. All rights reserved.
