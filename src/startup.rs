//! "Start with Windows" via the per-user Run key (no administrator rights needed),
//! plus toast AppUserModelID registration (also per-user).

use crate::settings::APP_NAME;
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const AUMID: &str = "LGPBatteryMonitor";

pub fn is_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_READ)
        .and_then(|k| k.get_value::<String, _>(APP_NAME))
        .is_ok()
}

pub fn set_enabled(enabled: bool) -> std::io::Result<()> {
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_WRITE)?;
    if enabled {
        let exe = std::env::current_exe()?;
        key.set_value(APP_NAME, &format!("\"{}\" --autostart", exe.display()))?;
    } else {
        match key.delete_value(APP_NAME) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Registers a display name so toast notifications are attributed to this app.
pub fn register_toast_identity() {
    let path = format!(r"Software\Classes\AppUserModelId\{AUMID}");
    if let Ok((key, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(path) {
        let _ = key.set_value("DisplayName", &APP_NAME);
    }
}
