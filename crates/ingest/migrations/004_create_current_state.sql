-- Current operational state per device (one row per device, overwritten).
--
-- This is the derived view shown to users. The state engine overwrites
-- this row each time it re-evaluates a device.
CREATE TABLE IF NOT EXISTS current_state (
    device_id         TEXT        PRIMARY KEY,
    remaining_grams   INTEGER,
    status            TEXT        NOT NULL DEFAULT 'unknown',
    last_seen_at      TIMESTAMPTZ,
    active_refill_id  UUID,
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
