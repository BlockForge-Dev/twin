# Contributing to CylinderSense

Thank you for contributing to **CylinderSense**! This document provides guidelines and standards for developing, testing, and submitting code to the repository.

---

## 🏛️ Repository Architecture

The project is organized as a Cargo workspace with decoupled crates:

| Crate / Path | Description |
| :--- | :--- |
| `crates/core` | Shared domain models (`Device`, `TelemetryRaw`, `CurrentState`, `AlertEvent`), payloads, and common error types (`AppError`). |
| `crates/engine` | Pure mathematical state estimator (`compute_gas_remaining`), outlier rejection, moving average smoothing, and alert transition rules (`should_trigger_alert`). |
| `crates/ingest` | Axum HTTP server handling telemetry ingestion, REST APIs, database migrations (`migrations/`), and static web dashboard serving (`web/`). |
| `crates/simulator` | Rust CLI binary generating synthetic LPG depletion telemetry with Gaussian noise, weight spikes, and auto-refills. |
| `web/` | Vanilla HTML5/CSS3/JS frontend served directly by Axum at `http://localhost:3001`. |

---

## 🛠️ Local Development & Quickstart

1. **Start PostgreSQL**:
   ```bash
   docker compose up -d
   ```

2. **Run Backend Service**:
   ```bash
   cargo run -p cylindersense-ingest
   ```

3. **Run Simulator**:
   ```bash
   cargo run -p cylindersense-simulator
   ```

4. **Access Web Dashboard**:
   Open `http://localhost:3001` in your web browser.

---

## 📏 Code Standards & Linting

Before pushing code or opening a pull request, ensure your code satisfies formatting and linting requirements:

1. **Format Code**:
   ```bash
   cargo fmt --check
   ```
   To auto-format code, run `cargo fmt`.

2. **Run Clippy Linter**:
   ```bash
   cargo clippy --workspace -- -D warnings
   ```
   No clippy warnings are permitted in `main`.

3. **Run All Tests**:
   ```bash
   cargo test --workspace
   ```
   All unit and database integration tests must pass cleanly.

---

## 📝 Commit Conventions

We follow clear, descriptive commit messages:

- `feat(ingest): add GET /api/v1/devices/{id}/state endpoint`
- `fix(engine): clamp remaining gas calculation to non-negative values`
- `docs: add pilot deployment runbook`

---

## 🧪 Testing Guidelines

- **Pure Functions**: Write unit tests inside `#[cfg(test)] mod tests` in the relevant module (e.g., `crates/engine/src/state_estimator.rs`).
- **API Endpoints**: Add integration tests in `crates/ingest/tests/ingest_api_tests.rs`. Use `tower::ServiceExt` to test route responses and database interactions.
