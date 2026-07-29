use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The telemetry payload contract between the device and the backend.
///
/// This is the minimum required data the device must send on each reading.
/// Additional health fields (battery, RSSI) will be added in a separate
/// `DeviceHealthPayload` in a later milestone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryPayload {
    /// Hardware-stamped device identity string.
    pub device_id: String,
    /// Timestamp of the reading as reported by the device.
    pub timestamp: DateTime<Utc>,
    /// Raw load cell reading in grams.
    pub raw_load_grams: i32,
}
