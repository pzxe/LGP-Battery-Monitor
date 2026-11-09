//! Low / critical battery notification logic with de-duplication.

use crate::hidpp::Charging;

pub const LOW_THRESHOLD: u8 = 20;
pub const CRITICAL_THRESHOLD: u8 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    Low(u8),
    Critical(u8),
}

impl Notice {
    pub fn title(&self) -> &'static str {
        match self {
            Notice::Low(_) => "Mouse battery low",
            Notice::Critical(_) => "Mouse battery critical",
        }
    }
    pub fn body(&self) -> String {
        match self {
            Notice::Low(p) => format!("Battery is at {p}%. Consider charging soon."),
            Notice::Critical(p) => format!("Battery is at {p}%. Plug in the charging cable now."),
        }
    }
}

#[derive(Debug, Default)]
pub struct Notifier {
    low_sent: bool,
    critical_sent: bool,
}

impl Notifier {
    /// Returns a notice only the first time each threshold is crossed. The latch
    /// re-arms once the battery is charging or has recovered above the threshold.
    pub fn evaluate(&mut self, percent: u8, charging: Charging) -> Option<Notice> {
        let is_charging = matches!(
            charging,
            Charging::Charging | Charging::ChargingSlow | Charging::Full
        );
        if is_charging || percent > LOW_THRESHOLD {
            self.low_sent = false;
        }
        if is_charging || percent > CRITICAL_THRESHOLD {
            self.critical_sent = false;
        }
        if is_charging {
            return None;
        }
        if percent <= CRITICAL_THRESHOLD && !self.critical_sent {
            self.critical_sent = true;
            self.low_sent = true; // do not follow a critical alert with a "low" one
            return Some(Notice::Critical(percent));
        }
        if percent <= LOW_THRESHOLD && percent > CRITICAL_THRESHOLD && !self.low_sent {
            self.low_sent = true;
            return Some(Notice::Low(percent));
        }
        None
    }
}

pub fn show(notice: Notice) {
    let _ = tauri_winrt_notification::Toast::new(crate::startup::AUMID)
        .title(notice.title())
        .text1(&notice.body())
        .show();
}

#[cfg(test)]
mod tests {
    use super::*;
    use Charging as C;

    #[test]
    fn low_fires_once() {
        let mut n = Notifier::default();
        assert_eq!(n.evaluate(50, C::Discharging), None);
        assert_eq!(n.evaluate(20, C::Discharging), Some(Notice::Low(20)));
        assert_eq!(n.evaluate(19, C::Discharging), None);
        assert_eq!(n.evaluate(15, C::Discharging), None);
    }

    #[test]
    fn critical_fires_once_and_suppresses_low() {
        let mut n = Notifier::default();
        assert_eq!(n.evaluate(10, C::Discharging), Some(Notice::Critical(10)));
        assert_eq!(n.evaluate(9, C::Discharging), None);
        assert_eq!(n.evaluate(15, C::Discharging), None);
    }

    #[test]
    fn low_then_critical() {
        let mut n = Notifier::default();
        assert_eq!(n.evaluate(18, C::Discharging), Some(Notice::Low(18)));
        assert_eq!(n.evaluate(9, C::Discharging), Some(Notice::Critical(9)));
    }

    #[test]
    fn rearms_after_charge() {
        let mut n = Notifier::default();
        assert!(n.evaluate(19, C::Discharging).is_some());
        assert_eq!(n.evaluate(40, C::Charging), None);
        assert!(n.evaluate(19, C::Discharging).is_some());
    }

    #[test]
    fn no_alert_while_charging() {
        let mut n = Notifier::default();
        assert_eq!(n.evaluate(5, C::Charging), None);
        assert_eq!(n.evaluate(5, C::ChargingSlow), None);
    }
}
