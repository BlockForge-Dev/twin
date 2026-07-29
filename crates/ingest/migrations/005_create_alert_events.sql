-- Alert events — notification-worthy state transitions.
CREATE TABLE IF NOT EXISTS alert_events (
    id              UUID        PRIMARY KEY,
    device_id       TEXT        NOT NULL,
    state_from      TEXT        NOT NULL,
    state_to        TEXT        NOT NULL,
    triggered_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged_at TIMESTAMPTZ,
    message         TEXT
);

CREATE INDEX idx_alert_events_device_id    ON alert_events (device_id);
CREATE INDEX idx_alert_events_triggered_at ON alert_events (triggered_at DESC);
