# CylinderSense Pilot Deployment Runbook

This runbook provides step-by-step procedures for deploying CylinderSense in a pilot environment, onboarding hardware devices, troubleshooting offline sensors, managing backups, and responding to system alerts.

---

## 🚀 1. Production / Pilot Deployment

### Prerequisites
- Docker & Docker Compose
- Rust 1.75+ (if running binaries natively)

### Step 1: External Environment Configuration
Create a `.env` file or export production environment variables:

```bash
DATABASE_URL=postgres://cs_dev:cs_dev_pass@localhost:5432/cylindersense
HOST=0.0.0.0
PORT=3000
LOG_LEVEL=info
LOG_FORMAT=json
```

### Step 2: Launch Services
```bash
# 1. Start PostgreSQL database container
docker compose up -d

# 2. Run backend ingest service (automatically applies SQL migrations)
cargo run -p cylindersense-ingest --release
```

### Step 3: Verify Health Probe
```bash
curl http://localhost:3000/health
```
**Expected Response**:
```json
{
  "status": "healthy",
  "database": "connected",
  "version": "0.1.0"
}
```

---

## 📲 2. Device Onboarding Workflow

When deploying a new CylinderSense sensor unit at a pilot site:

### Step 1: Register Hardware Identity
Register the hardware-stamped serial/MAC address via the API or Web Dashboard:

```bash
curl -X POST http://localhost:3000/api/v1/devices \
  -H "Content-Type: application/json" \
  -d '{
    "device_id": "CS-PILOT-101",
    "model": "basic_v1",
    "firmware_version": "1.0.0"
  }'
```

### Step 2: Assign to Commercial Site
Assign the device to a customer location (e.g. `bakery-kitchen-1`):

```bash
curl -X POST http://localhost:3000/api/v1/devices/CS-PILOT-101/assign \
  -H "Content-Type: application/json" \
  -d '{ "site_id": "bakery-kitchen-1" }'
```

### Step 3: Record Initial Cylinder Refill / Baseline
Anchor the remaining gas calculation with an initial refill record (e.g. 12.5 kg fill):

```bash
curl -X POST http://localhost:3000/api/v1/devices/CS-PILOT-101/refill \
  -H "Content-Type: application/json" \
  -d '{
    "fill_amount_kg": 12.5,
    "cylinder_name": "Main Kitchen Tank",
    "notes": "Initial pilot installation"
  }'
```

---

## 🛠️ 3. Troubleshooting Offline Devices

If a device displays status `Offline` on the dashboard or triggers an `Offline` alert event:

### Diagnostic Flow:
1. **Check Backend Connectivity**:
   Verify if the backend server is reachable from the hardware device's network:
   `curl http://<SERVER_IP>:3000/health`

2. **Inspect Simulator / Sensor Logs**:
   Look for HTTP connection errors in device logs:
   `warn: connection failed (is ingest running?)`

3. **Check Device Power & Wi-Fi RSSI**:
   Ensure the load-cell unit is powered and RSSI is stronger than `-85 dBm`.

4. **Verify Database Last Telemetry**:
   Query recent telemetry rows for the device directly in PostgreSQL:
   ```sql
   SELECT * FROM telemetry_raw WHERE device_id = 'CS-PILOT-101' ORDER BY timestamp DESC LIMIT 5;
   ```

---

## 💾 4. Database Backup & Restore Procedures

### Database Backup (pg_dump)
To take an automated logical backup of raw telemetry, devices, refill audit records, and states:

```bash
docker exec -t cylindersense-db pg_dump -U cs_dev -d cylindersense -F c -b -v -f /var/lib/postgresql/data/backup_$(date +%Y%m%d_%H%M%S).dump
```

### Database Restore (pg_restore)
To restore a backup into a fresh Postgres database:

```bash
docker exec -i cylindersense-db pg_restore -U cs_dev -d cylindersense -v /var/lib/postgresql/data/backup_filename.dump
```

---

## 🚨 5. Alert Response Protocol

| Alert Level | Condition | Operator Action |
| :--- | :--- | :--- |
| **Low** | Gas $\le 20\%$ ($\sim 2.5\text{kg}$ remaining) | Schedule cylinder replacement with supplier within 24 hours. |
| **Critical** | Gas $\le 5\%$ ($\sim 0.6\text{kg}$ remaining) | Immediate dispatch required. Prepare backup cylinder. |
| **Offline** | No telemetry for $> 15\text{min}$ | Contact site manager to check device power and router status. |

Operators can acknowledge alerts on the Web Dashboard (`http://localhost:3000`) or via API:
```bash
curl -X POST http://localhost:3000/api/v1/alerts/<ALERT_UUID>/acknowledge
```
