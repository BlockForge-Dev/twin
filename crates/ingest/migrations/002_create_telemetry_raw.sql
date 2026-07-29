-- Raw telemetry readings from devices (append-only).
--
-- No foreign key to devices: telemetry may arrive before a device is
-- formally registered. The relationship is enforced at the application
-- layer during state computation.
CREATE TABLE IF NOT EXISTS telemetry_raw (
    id              UUID        PRIMARY KEY,
    device_id       TEXT        NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL,
    raw_load_grams  INTEGER     NOT NULL,
    battery_pct     SMALLINT,
    rssi            SMALLINT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_telemetry_raw_device_id  ON telemetry_raw (device_id);
CREATE INDEX idx_telemetry_raw_timestamp  ON telemetry_raw (device_id, timestamp DESC);
