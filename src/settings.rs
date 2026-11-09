use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const APP_NAME: &str = "LGP Battery Monitor";
pub const REFRESH_CHOICES: [u64; 4] = [30, 60, 120, 300];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub refresh_secs: u64,
    pub notifications: bool,
    pub start_with_windows: bool,
    pub start_minimized: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            refresh_secs: 60,
            notifications: true,
            start_with_windows: false,
            start_minimized: true,
        }
    }
}

impl Settings {
    /// Clamp values loaded from disk to sane ranges.
    pub fn sanitized(mut self) -> Self {
        self.refresh_secs = self.refresh_secs.clamp(15, 3600);
        self
    }

    pub fn path() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(APP_NAME).join("settings.json"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .unwrap_or_default()
            .sanitized()
    }

    pub fn save(&self) {
        let Some(p) = Self::path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_partial_json() {
        let s: Settings = serde_json::from_str(r#"{"refresh_secs": 120}"#).unwrap();
        assert_eq!(s.refresh_secs, 120);
        assert!(s.notifications);
        assert!(s.start_minimized);
        assert!(!s.start_with_windows);
    }

    #[test]
    fn sanitize_clamps() {
        let s = Settings {
            refresh_secs: 1,
            ..Default::default()
        }
        .sanitized();
        assert_eq!(s.refresh_secs, 15);
        let s = Settings {
            refresh_secs: 999_999,
            ..Default::default()
        }
        .sanitized();
        assert_eq!(s.refresh_secs, 3600);
    }
}
