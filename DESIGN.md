# CylinderSense — Architecture & Design

## 1. Overview

CylinderSense is a connected LPG cylinder monitoring system for small commercial
operators. The core promise: **know how much gas remains before operations are
disrupted**.

The architecture follows a strict layered principle:

```
 ┌──────────────┐       ┌──────────────────┐       ┌──────────────┐
 │   Device     │──────▶│   Backend        │◀──────│   App / UI   │
 │  (ESP32 +    │ HTTP  │  (Axum + Postgres)│  API  │  (thin web/  │
 │  load cell)  │ POST  │                  │  GET  │   mobile)    │
 └──────────────┘       └──────────────────┘       └──────────────┘
       │                        │
  Measures &              Interprets &
  Displays kg             Decides (source
  locally                 of truth)
```

**Hardware measures. Backend interprets. App presents.**

The backend is the product brain. It ingests raw telemetry, combines it with
refill context, derives operational state, and triggers alerts. The app is a
thin window into that truth.

---

## 2. Data Model

### 2.1 Entity Relationship Diagram

```
┌─────────────────┐
│     devices      │
├─────────────────┤
│ id         UUID  │◄──PK
│ device_id  TEXT  │◄──UNIQUE ──────────────────────────────────┐
│ model      TEXT  │                                            │
│ firmware   TEXT  │                                            │
│ status     TEXT  │  (active / inactive / uninitialized)       │
│ site_id    TEXT  │                                            │
│ created_at TSTZ  │                                            │
│ updated_at TSTZ  │                                            │
└─────────────────┘                                            │
       │                                                        │
       │ FK (device_id)                                         │
       ▼                                                        │
┌──────────────────┐                                            │
│  refill_records   │                                           │
├──────────────────┤                                            │
│ id           UUID │◄──PK                                      │
│ device_id    TEXT  │──FK──▶ devices.device_id                  │
│ fill_amount  INT   │  (grams)                                  │
│ cylinder_name TEXT │                                           │
│ cylinder_profile   │                                           │
│ refill_date  TSTZ  │                                           │
│ edited_by    TEXT  │                                           │
│ notes        TEXT  │                                           │
│ created_at   TSTZ  │                                           │
│ updated_at   TSTZ  │                                           │
└──────────────────┘                                            │
                                                                │
┌──────────────────┐     (no FK — intentional)                  │
│  telemetry_raw    │─ ─ ─ ─ ─ ─ ─ logical ref ─ ─ ─ ─ ─ ─ ─ ┘
├──────────────────┤
│ id            UUID│◄──PK
│ device_id     TEXT│  (logical key, not FK)
│ timestamp     TSTZ│
│ raw_load_grams INT│
│ battery_pct   I16 │
│ rssi          I16 │
│ created_at    TSTZ│
└──────────────────┘   ◄── APPEND-ONLY

┌──────────────────┐
│  current_state    │
├──────────────────┤
│ device_id    TEXT  │◄──PK  (one row per device)
│ remaining_grams   │
│ status       TEXT  │  (normal / low / critical / offline / unknown)
│ last_seen_at TSTZ  │
│ active_refill_id  │  (UUID ref to refill_records)
│ updated_at   TSTZ  │
└──────────────────┘   ◄── OVERWRITTEN by state engine

┌──────────────────┐
│  alert_events     │
├──────────────────┤
│ id           UUID │◄──PK
│ device_id    TEXT  │
│ state_from   TEXT  │
│ state_to     TEXT  │
│ triggered_at TSTZ  │
│ acknowledged_at   │
│ message      TEXT  │
└──────────────────┘   ◄── APPEND-ONLY (state transitions)
```

### 2.2 Entity Summary

| Entity | Purpose | Mutability |
|--------|---------|-----------|
| `devices` | Registered physical monitoring units | Mutable (status, firmware updates) |
| `telemetry_raw` | Raw sensor readings from hardware | **Append-only**, never mutated |
| `refill_records` | Refill context per depletion cycle | Mutable (operators correct mistakes) |
| `current_state` | Derived operational truth per device | **Overwritten** on each state re-evaluation |
| `alert_events` | Notification-worthy state transitions | **Append-only**, acknowledged_at updated |

---

## 3. Design Decisions

### 3.1 Raw Telemetry Separated from Derived State

Raw telemetry (`telemetry_raw`) is never modified and exists for:
- Debugging and audit trail
- Historical replay if estimation logic improves
- Proving calibration accuracy during pilots

Derived state (`current_state`) is what users see. It is recomputed by the
state engine each time new data arrives. This separation means:
- The app never reads raw telemetry directly
- The state engine can be improved without touching historical data
- Bad readings don't corrupt the operational view (the engine filters them)

### 3.2 Backend as Source of Truth

All product logic lives in the backend, not the app or device:
- Status computation (Normal → Low → Critical)
- Threshold evaluation and alert generation
- Refill record validation

The device is a "dumb sensor" that reads and transmits. The app is a "thin
window" that displays. This means the product can improve without replacing
hardware or shipping app updates.

### 3.3 No Foreign Key on telemetry_raw

`telemetry_raw.device_id` does **not** have a foreign key to `devices`.
This is intentional:

- Telemetry may arrive before a device is formally registered
- The ingestion path must not fail on an unregistered device_id
- The relationship is enforced at the application layer during state computation

### 3.4 Status Enums

**DeviceStatus**: `active`, `inactive`, `uninitialized`
- Tracks whether a device is operational, offline, or never set up

**CylinderStatus**: `normal`, `low`, `critical`, `offline`, `unknown`
- Derived by the state engine based on remaining gas and connectivity
- Drives alert generation and user-facing display
- `unknown` is the initial state before any refill context exists

### 3.5 Integer Grams for Load Values

Load values are stored as `i32` grams (not floats). This gives:
- Range up to ~2,147,483 kg — far beyond any cylinder
- No floating-point precision issues
- Simple arithmetic for threshold comparison
- Sub-gram precision is unnecessary for the load cell resolution

### 3.6 Workspace Crate Layout

| Crate | Type | Purpose |
|-------|------|---------|
| `cylindersense-core` | Library | Domain types, enums, error types — no I/O |
| `cylindersense-ingest` | Binary | Axum HTTP service, telemetry ingestion, migrations |
| `cylindersense-engine` | Library | State estimation and alert rule logic |
| `cylindersense-simulator` | Binary | Device simulator for testing |

---

## 4. Explicit Non-Goals (v1)

These are **out of scope** for the MVP. They may become goals in future
product generations but must not be attempted in v1:

- **Gas leak detection**: Requires additional hardware sensors and safety
  certification. v1 is a depletion monitoring product, not a safety device.
- **Automatic shutoff**: No physical actuator or safety-critical control path.
- **AI/ML predictions**: No "days remaining" predictions until enough usage
  history exists. v1 shows remaining grams, not forecasts.
- **Analytics dashboards**: No charts, graphs, or data exploration UI. The app
  shows the answer directly.
- **Multi-SKU hardware**: One device model (Basic) at launch. Premium safety
  hardware (CylinderSense Safe/Pro) is Gen 2.
- **Consumer/household rollout**: First users are B2B commercial kitchens.
- **Multi-cylinder-type support**: One cylinder profile at launch.
- **Partner/third-party API**: No external API surface in v1.

---

## 5. Technology Stack

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Language | Rust | Type safety, performance, reliable concurrency |
| Web framework | Axum | Async, tower-based, ergonomic Rust HTTP |
| Database | PostgreSQL 16 | Mature, relational, excellent for time-series + relational data |
| Migrations | sqlx migrate | Compile-time checked queries (future), embedded migrations |
| Serialization | serde + serde_json | Standard Rust ecosystem |
| Observability | tracing + tracing-subscriber | Structured logging with env-filter |
| Containerization | Docker Compose | Local development database |
