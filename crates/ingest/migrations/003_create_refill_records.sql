-- Refill records capturing fill context for each depletion cycle.
CREATE TABLE IF NOT EXISTS refill_records (
    id                UUID        PRIMARY KEY,
    device_id         TEXT        NOT NULL REFERENCES devices(device_id),
    fill_amount_grams INTEGER     NOT NULL,
    cylinder_name     TEXT,
    cylinder_profile  TEXT,
    refill_date       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    edited_by         TEXT,
    notes             TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_refill_records_device_id ON refill_records (device_id);
