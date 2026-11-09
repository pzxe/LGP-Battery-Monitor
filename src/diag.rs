//! Opt-in diagnostic log (`--debug`), written next to the settings file.

use std::io::Write;
use std::sync::OnceLock;

static ENABLED: OnceLock<bool> = OnceLock::new();

pub fn init(enabled: bool) {
    let _ = ENABLED.set(enabled);
}

pub fn log(msg: impl AsRef<str>) {
    if !ENABLED.get().copied().unwrap_or(false) {
        return;
    }
    let Some(dir) =
        crate::settings::Settings::path().and_then(|p| p.parent().map(|d| d.to_path_buf()))
    else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("debug.log"))
    {
        let _ = writeln!(
            f,
            "{} {}",
            chrono::Local::now().format("%H:%M:%S%.3f"),
            msg.as_ref()
        );
    }
}
