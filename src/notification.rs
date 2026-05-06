use crate::model::MemoryPressure;
use crate::trend::LikelyCulprit;
use std::time::{Duration, Instant};

pub const HIGH_PRESSURE_NOTIFICATION_COOLDOWN: Duration = Duration::from_secs(15 * 60);

pub fn should_notify_high_pressure(
    previous: MemoryPressure,
    current: MemoryPressure,
    last_notification: Option<Instant>,
    now: Instant,
) -> bool {
    if !matches!(current, MemoryPressure::High) || matches!(previous, MemoryPressure::High) {
        return false;
    }

    last_notification
        .map(|last| now.duration_since(last) >= HIGH_PRESSURE_NOTIFICATION_COOLDOWN)
        .unwrap_or(true)
}

pub fn high_pressure_notification_text(culprit: Option<&LikelyCulprit>) -> String {
    match culprit {
        Some(culprit) => format!(
            "Top riser: {} +{} MB",
            culprit.name,
            culprit.delta_bytes / 1_000_000
        ),
        None => "Open rami to check top apps".to_string(),
    }
}

#[allow(deprecated)]
pub fn deliver_high_pressure_notification(body: &str) {
    use objc2_foundation::{NSString, NSUserNotification, NSUserNotificationCenter};

    let notification = NSUserNotification::new();
    notification.setTitle(Some(&NSString::from_str("RAM pressure high")));
    notification.setInformativeText(Some(&NSString::from_str(body)));
    let center = NSUserNotificationCenter::defaultUserNotificationCenter();
    center.deliverNotification(&notification);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifies_only_on_high_pressure_transition_after_cooldown() {
        let now = Instant::now();
        assert!(should_notify_high_pressure(
            MemoryPressure::Elevated,
            MemoryPressure::High,
            None,
            now
        ));
        assert!(!should_notify_high_pressure(
            MemoryPressure::High,
            MemoryPressure::High,
            None,
            now
        ));
        assert!(!should_notify_high_pressure(
            MemoryPressure::Normal,
            MemoryPressure::Elevated,
            None,
            now
        ));
        assert!(!should_notify_high_pressure(
            MemoryPressure::Normal,
            MemoryPressure::High,
            Some(now - Duration::from_secs(60)),
            now
        ));
        assert!(should_notify_high_pressure(
            MemoryPressure::Normal,
            MemoryPressure::High,
            Some(now - HIGH_PRESSURE_NOTIFICATION_COOLDOWN),
            now
        ));
    }

    #[test]
    fn notification_text_uses_top_riser_when_available() {
        let culprit = LikelyCulprit {
            name: "Zen".to_string(),
            delta_bytes: 420_000_000,
        };
        assert_eq!(
            high_pressure_notification_text(Some(&culprit)),
            "Top riser: Zen +420 MB"
        );
        assert_eq!(
            high_pressure_notification_text(None),
            "Open rami to check top apps"
        );
    }
}
