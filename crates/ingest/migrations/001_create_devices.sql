-- Physical CylinderSense monitoring devices.
CREATE TABLE IF NOT EXISTS devices (
    id              UUID        PRIMARY KEY,
    device_id       TEXT        NOT NULL UNIQUE,  -- hardware-stamped identity
    model           TEXT,
    firmware_version TEXT,
    status          TEXT        NOT NULL DEFAULT 'uninitialized',
    site_id         TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_devices_device_id ON devices (device_id);
CREATE INDEX idx_devices_site_id   ON devices (site_id);
