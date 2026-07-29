use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Operational status of a physical device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Active,
    Inactive,
    Uninitialized,
}

impl std::fmt::Display for DeviceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Inactive => write!(f, "inactive"),
            Self::Uninitialized => write!(f, "uninitialized"),
        }
    }
}

/// Derived cylinder status computed by the state engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CylinderStatus {
    Normal,
    Low,
    Critical,
    Offline,
    Unknown,
}

impl std::fmt::Display for CylinderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "normal"),
            Self::Low => write!(f, "low"),
            Self::Critical => write!(f, "critical"),
            Self::Offline => write!(f, "offline"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

// ---------------------------------------------------------------------------
// Domain structs
// ---------------------------------------------------------------------------

/// A physical CylinderSense monitoring device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// Internal database primary key.
    pub id: Uuid,
    /// Hardware-stamped unique identity string (e.g. MAC or serial).
    pub device_id: String,
    /// Hardware model identifier (e.g. "basic_v1").
    pub model: Option<String>,
    /// Current firmware version running on the device.
    pub firmware_version: Option<String>,
    /// Operational status of the device.
    pub status: DeviceStatus,
    /// Optional site/location this device is assigned to.
    pub site_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single raw telemetry reading from a device.
///
/// This is an **append-only** record. Raw telemetry is never mutated.
/// It is kept separate from derived state for auditability and replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRaw {
    pub id: Uuid,
    /// Hardware device identity (logical key, not FK-enforced).
    pub device_id: String,
    /// Timestamp reported by the device.
    pub timestamp: DateTime<Utc>,
    /// Raw load sensor reading in grams.
    pub raw_load_grams: i32,
    /// Battery percentage (0-100), if reported.
    pub battery_pct: Option<i16>,
    /// Wi-Fi RSSI in dBm, if reported.
    pub rssi: Option<i16>,
    /// When the backend received this reading.
    pub created_at: DateTime<Utc>,
}

/// Captures the refill context for a depletion cycle.
///
/// When a cylinder is filled or replaced, the operator records the fill
/// amount. This value anchors the state engine's remaining-gas estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefillRecord {
    pub id: Uuid,
    pub device_id: String,
    /// Amount filled in grams.
    pub fill_amount_grams: i32,
    /// Human-readable cylinder name or label.
    pub cylinder_name: Option<String>,
    /// Cylinder size profile (e.g. "12.5kg", "6kg").
    pub cylinder_profile: Option<String>,
    pub refill_date: DateTime<Utc>,
    /// Who recorded or last edited this refill.
    pub edited_by: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The current operational truth for a device, shown to users.
///
/// Exactly **one row per device**. Overwritten each time the state engine
/// re-evaluates. This is the derived view — never the source of truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentState {
    /// The device this state belongs to (primary key in DB).
    pub device_id: String,
    /// Estimated remaining gas in grams.
    pub remaining_grams: Option<i32>,
    /// Derived cylinder status.
    pub status: CylinderStatus,
    /// Last time a reading was received from this device.
    pub last_seen_at: Option<DateTime<Utc>>,
    /// The active refill record anchoring this estimate.
    pub active_refill_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

/// A notification-worthy state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: Uuid,
    pub device_id: String,
    /// Status before the transition.
    pub state_from: CylinderStatus,
    /// Status after the transition.
    pub state_to: CylinderStatus,
    /// When the alert was generated.
    pub triggered_at: DateTime<Utc>,
    /// When an operator acknowledged the alert (if ever).
    pub acknowledged_at: Option<DateTime<Utc>>,
    /// Human-readable alert message.
    pub message: Option<String>,
}
