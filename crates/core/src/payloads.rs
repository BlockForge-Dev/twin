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

/// Payload to register a new physical device in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDevicePayload {
    /// Hardware-stamped unique identity string (e.g. MAC address or serial number).
    pub device_id: String,
    /// Optional hardware model (e.g. "basic_v1").
    pub model: Option<String>,
    /// Optional initial firmware version string.
    pub firmware_version: Option<String>,
}

/// Payload to assign a device to a site/location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignDevicePayload {
    /// Site or location ID to assign the device to.
    pub site_id: String,
}

/// Payload for recording a cylinder refill event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRefillPayload {
    /// Refill amount in kilograms (e.g. 12.5). Converted to grams internally.
    pub fill_amount_kg: f64,
    /// Optional cylinder label or name (e.g. "Kitchen Tank #1").
    pub cylinder_name: Option<String>,
    /// Optional cylinder profile (e.g. "12.5kg", "6kg").
    pub cylinder_profile: Option<String>,
    /// Operator ID or username who recorded the refill.
    pub edited_by: Option<String>,
    /// Optional notes or comments.
    pub notes: Option<String>,
}

/// Payload for editing an existing refill record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRefillPayload {
    /// Updated fill amount in kilograms.
    pub fill_amount_kg: Option<f64>,
    pub cylinder_name: Option<String>,
    pub cylinder_profile: Option<String>,
    pub edited_by: Option<String>,
    pub notes: Option<String>,
}

/// Payload for reassigning device site/cylinder context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReassignDevicePayload {
    pub site_id: Option<String>,
    pub cylinder_name: Option<String>,
    pub cylinder_profile: Option<String>,
    pub edited_by: Option<String>,
}
