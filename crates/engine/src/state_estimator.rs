use chrono::{DateTime, Utc};
use cylindersense_core::models::CylinderStatus;

/// Default cylinder parameters if no custom profile is provided.
pub const DEFAULT_TARE_GRAMS: i32 = 5500;
pub const DEFAULT_FILL_GRAMS: i32 = 12500;
pub const DEFAULT_OFFLINE_TIMEOUT_SECS: i64 = 900; // 15 minutes

/// Pure function to calculate remaining gas in grams and derived status.
///
/// Returns `(remaining_grams, CylinderStatus)`.
/// `remaining_grams` is clamped to `[0, fill_amount_grams]`.
pub fn compute_gas_remaining(
    raw_load_grams: i32,
    tare_grams: i32,
    fill_amount_grams: i32,
) -> (i32, CylinderStatus) {
    if fill_amount_grams <= 0 {
        return (0, CylinderStatus::Unknown);
    }

    let net_weight = raw_load_grams - tare_grams;
    let remaining_grams = net_weight.max(0).min(fill_amount_grams);

    let ratio = (remaining_grams as f64) / (fill_amount_grams as f64);

    let status = if remaining_grams == 0 || ratio <= 0.05 {
        CylinderStatus::Critical
    } else if ratio <= 0.20 {
        CylinderStatus::Low
    } else {
        CylinderStatus::Normal
    };

    (remaining_grams, status)
}

/// Outlier rejection and Simple Moving Average (SMA) smoothing.
///
/// Filters out impossible readings (negative or excessive spikes)
/// and averages valid readings in the window.
pub fn smooth_raw_readings(readings: &[i32], tare_grams: i32, fill_amount_grams: i32) -> i32 {
    if readings.is_empty() {
        return tare_grams;
    }

    // Maximum physically reasonable load = tare + fill + 2000g tolerance
    let max_allowed = tare_grams + fill_amount_grams + 2000;

    let valid_readings: Vec<i32> = readings
        .iter()
        .copied()
        .filter(|&r| r >= 0 && r <= max_allowed)
        .collect();

    if valid_readings.is_empty() {
        // Fallback to latest reading if all failed filter
        return readings[0];
    }

    let sum: i64 = valid_readings.iter().map(|&x| x as i64).sum();
    (sum / valid_readings.len() as i64) as i32
}

/// Evaluates whether a device should be marked Offline based on last seen timestamp.
pub fn eval_offline_status(
    current_status: CylinderStatus,
    last_seen_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    timeout_secs: i64,
) -> CylinderStatus {
    let Some(last_seen) = last_seen_at else {
        return CylinderStatus::Unknown;
    };

    let elapsed = (now - last_seen).num_seconds();
    if elapsed > timeout_secs {
        CylinderStatus::Offline
    } else {
        current_status
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_gas_remaining_normal() {
        // 5500g tare + 10000g gas = 15500g raw load (80% full)
        let (rem, status) = compute_gas_remaining(15500, 5500, 12500);
        assert_eq!(rem, 10000);
        assert_eq!(status, CylinderStatus::Normal);
    }

    #[test]
    fn test_compute_gas_remaining_low() {
        // 5500g tare + 2000g gas = 7500g raw load (16% full -> Low)
        let (rem, status) = compute_gas_remaining(7500, 5500, 12500);
        assert_eq!(rem, 2000);
        assert_eq!(status, CylinderStatus::Low);
    }

    #[test]
    fn test_compute_gas_remaining_critical() {
        // 5500g tare + 500g gas = 6000g raw load (4% full -> Critical)
        let (rem, status) = compute_gas_remaining(6000, 5500, 12500);
        assert_eq!(rem, 500);
        assert_eq!(status, CylinderStatus::Critical);
    }

    #[test]
    fn test_compute_gas_remaining_exactly_empty() {
        // 5500g tare + 0g gas = 5500g raw load -> Critical
        let (rem, status) = compute_gas_remaining(5500, 5500, 12500);
        assert_eq!(rem, 0);
        assert_eq!(status, CylinderStatus::Critical);
    }

    #[test]
    fn test_compute_gas_remaining_clamped_negative() {
        // Sensor drift below tare: 5000g raw load -> clamped to 0g -> Critical
        let (rem, status) = compute_gas_remaining(5000, 5500, 12500);
        assert_eq!(rem, 0);
        assert_eq!(status, CylinderStatus::Critical);
    }

    #[test]
    fn test_outlier_rejection_and_smoothing() {
        let tare = 5500;
        let fill = 12500;

        // Sequence of readings with a 5000g spike (e.g. pan placed on scale)
        // Normal readings ~15000g
        let readings = vec![15000, 15020, 24000, 14980, 15000]; // 24000 is spike > max_allowed (20000)

        let smoothed = smooth_raw_readings(&readings, tare, fill);
        // Spike 24000 rejected, average of [15000, 15020, 14980, 15000] = 15000
        assert_eq!(smoothed, 15000);
    }

    #[test]
    fn test_eval_offline_status() {
        let now = Utc::now();
        let recent = now - chrono::Duration::seconds(100);
        let old = now - chrono::Duration::seconds(1000); // > 900s timeout

        assert_eq!(
            eval_offline_status(CylinderStatus::Normal, Some(recent), now, 900),
            CylinderStatus::Normal
        );

        assert_eq!(
            eval_offline_status(CylinderStatus::Normal, Some(old), now, 900),
            CylinderStatus::Offline
        );

        assert_eq!(
            eval_offline_status(CylinderStatus::Normal, None, now, 900),
            CylinderStatus::Unknown
        );
    }
}
