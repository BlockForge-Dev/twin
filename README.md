# CylinderSense (twin)

> **Commercial LPG Depletion & State Estimation Infrastructure**

CylinderSense is a Rust-based commercial LPG cylinder monitoring platform.
Physical sensors attached to load cells stream continuous telemetry to the ingest service.
The backend applies outlier rejection, smoothing, and domain rules to estimate remaining gas, detect refills, and generate real-time alerts.

> **Current status**: Milestone 7 — Production Hardening & Documentation complete (Externalized config defaults, DB health check probes, HTTP request tracing, structured JSON/text logging, GitHub Actions CI pipeline, `CONTRIBUTING.md`, `PILOT_RUNBOOK.md`, 19 tests passing).

---

## ⚡ 10-Minute Quickstart

Get the entire CylinderSense stack running locally in 3 steps:

### Step 1: Start PostgreSQL
```bash
docker compose up -d
```

### Step 2: Start Backend Service
```bash
cargo run -p cylindersense-ingest
```
> The backend server will automatically apply database migrations and start listening at `http://localhost:3001`.

### Step 3: Run Device Simulator
In a separate terminal:
```bash
cargo run -p cylindersense-simulator
```
> The simulator will stream realistic gas depletion telemetry (with noise, spikes, and auto-refills) to the backend.

### Open Web Dashboard
Open **`http://localhost:3001`** in your browser to view live cylinder gas levels, alert feeds, and record refills!

---

## 🏗️ Workspace Architecture

```text
c:\Users\hp\twin
├── Cargo.toml                # Workspace manifest
├── docker-compose.yml        # PostgreSQL container setup
├── DESIGN.md                 # System architecture design & domain model diagram
├── CONTRIBUTING.md           # Developer guidelines & code standards
├── PILOT_RUNBOOK.md          # Pilot deployment, onboarding & backup procedures
├── .github/workflows/ci.yml  # GitHub Actions CI pipeline
├── web/                      # Responsive HTML5/CSS3/JS Web Dashboard
└── crates/
    ├── core/                 # Shared domain models (Device, TelemetryRaw, CurrentState, AlertEvent)
    ├── engine/               # Pure state estimator (compute_gas_remaining, smoothing, alert rules)
    ├── ingest/               # Axum HTTP ingestion server, REST APIs, SQL migrations, static web serving
    └── simulator/            # Synthetic LPG telemetry simulator CLI
```

---

## 🔌 API Endpoints Summary

| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/health` | Liveness & PostgreSQL connectivity health probe |
| `POST` | `/api/v1/telemetry` | Ingest raw telemetry, smooth load, recompute state & alerts |
| `POST` | `/api/v1/devices` | Register new physical monitoring device |
| `GET` | `/api/v1/devices` | List registered devices |
| `POST` | `/api/v1/devices/{id}/assign` | Assign device to a site/location |
| `POST` | `/api/v1/devices/{id}/reassign` | Reassign site/cylinder context |
| `GET` | `/api/v1/devices/{id}/state` | Get latest derived operational state |
| `POST` | `/api/v1/devices/{id}/refill` | Record cylinder refill (gas level jumps to full) |
| `PUT` | `/api/v1/refills/{id}` | Edit refill record (audited) & recalculate state |
| `GET` | `/api/v1/devices/{id}/refills` | Query refill audit history |
| `GET` | `/api/v1/alerts` | Get system alert events (optional `?device_id=`) |
| `POST` | `/api/v1/alerts/{id}/acknowledge` | Mark alert event as acknowledged |

---

## ⚙️ Environment Configuration

Configuration is externalized with sensible defaults:

| Environment Variable | Default Value | Description |
| :--- | :--- | :--- |
| `DATABASE_URL` | `postgres://cs_dev:cs_dev_pass@localhost:5433/cylindersense` | PostgreSQL connection URL |
| `HOST` | `0.0.0.0` | Host interface to bind server |
| `PORT` | `3000` | HTTP port |
| `LOG_LEVEL` | `info` | Tracing log filter (`info`, `debug`, `trace`) |
| `LOG_FORMAT` | `text` | Logging output format (`text` or `json`) |

---

## 🧪 Testing & Quality Assurance

Run code format, linting, and workspace test suite:

```bash
# Check formatting
cargo fmt --check

# Run linter
cargo clippy --workspace -- -D warnings

# Run all tests
cargo test --workspace
```

---

## 📖 Documentation Links

- [System Architecture (DESIGN.md)](file:///c:/Users/hp/twin/DESIGN.md)
- [Contributing Guidelines (CONTRIBUTING.md)](file:///c:/Users/hp/twin/CONTRIBUTING.md)
- [Pilot Deployment Runbook (PILOT_RUNBOOK.md)](file:///c:/Users/hp/twin/PILOT_RUNBOOK.md)
