use cylindersense_core::models::CylinderStatus;

/// Determines whether a status transition is notification-worthy and should trigger an alert event.
///
/// Rules:
/// - `from_state == to_state`: No alert (deduplicated).
/// - `to_state == Low`: Trigger alert (e.g. Normal -> Low).
/// - `to_state == Critical`: Trigger alert (e.g. Normal -> Critical, Low -> Critical).
/// - `to_state == Offline`: Trigger alert (Any -> Offline).
pub fn should_trigger_alert(from_state: CylinderStatus, to_state: CylinderStatus) -> bool {
    if from_state == to_state {
        return false;
    }

    matches!(
        to_state,
        CylinderStatus::Low | CylinderStatus::Critical | CylinderStatus::Offline
    )
}

/// Generates a human-readable description message for an alert event.
pub fn generate_alert_message(
    device_id: &str,
    from_state: CylinderStatus,
    to_state: CylinderStatus,
) -> String {
    match to_state {
        CylinderStatus::Critical => format!(
            "Device '{device_id}' reached CRITICAL gas level! Status changed from {} to {}.",
            from_state, to_state
        ),
        CylinderStatus::Low => format!(
            "Device '{device_id}' reached LOW gas level. Status changed from {} to {}.",
            from_state, to_state
        ),
        CylinderStatus::Offline => format!(
            "Device '{device_id}' went OFFLINE. No telemetry received within timeout.",
        ),
        _ => format!(
            "Device '{device_id}' status changed from {} to {}.",
            from_state, to_state
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_rules_normal_to_low() {
        assert!(should_trigger_alert(
            CylinderStatus::Normal,
            CylinderStatus::Low
        ));
    }

    #[test]
    fn test_alert_rules_low_to_critical() {
        assert!(should_trigger_alert(
            CylinderStatus::Low,
            CylinderStatus::Critical
        ));
    }

    #[test]
    fn test_alert_rules_normal_to_critical() {
        assert!(should_trigger_alert(
            CylinderStatus::Normal,
            CylinderStatus::Critical
        ));
    }

    #[test]
    fn test_alert_rules_any_to_offline() {
        assert!(should_trigger_alert(
            CylinderStatus::Normal,
            CylinderStatus::Offline
        ));
        assert!(should_trigger_alert(
            CylinderStatus::Low,
            CylinderStatus::Offline
        ));
    }

    #[test]
    fn test_alert_rules_deduplication_no_trigger() {
        // Same state -> no alert
        assert!(!should_trigger_alert(
            CylinderStatus::Low,
            CylinderStatus::Low
        ));
        assert!(!should_trigger_alert(
            CylinderStatus::Critical,
            CylinderStatus::Critical
        ));
        assert!(!should_trigger_alert(
            CylinderStatus::Normal,
            CylinderStatus::Normal
        ));
    }

    #[test]
    fn test_alert_rules_refill_recovery_no_trigger() {
        // Refill back to Normal -> no warning alert
        assert!(!should_trigger_alert(
            CylinderStatus::Low,
            CylinderStatus::Normal
        ));
        assert!(!should_trigger_alert(
            CylinderStatus::Critical,
            CylinderStatus::Normal
        ));
    }

    #[test]
    fn test_alert_message_formatting() {
        let msg = generate_alert_message("dev-001", CylinderStatus::Normal, CylinderStatus::Low);
        assert!(msg.contains("dev-001"));
        assert!(msg.contains("LOW gas level"));
    }
}
