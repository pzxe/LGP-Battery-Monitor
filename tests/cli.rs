//! Black-box tests of the built executable (no hardware required).

use std::process::Command;

fn exe() -> Command {
    Command::new(env!("CARGO_BIN_EXE_lgp-battery-monitor"))
}

#[test]
fn version_flag_prints_version() {
    let out = exe().arg("--version").output().expect("run exe");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("LGP Battery Monitor"), "got: {text}");
    assert!(text.contains(env!("CARGO_PKG_VERSION")), "got: {text}");
}

#[test]
fn help_lists_options() {
    let out = exe().arg("--help").output().expect("run exe");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("--probe") && text.contains("--show"),
        "got: {text}"
    );
}

#[test]
fn probe_never_crashes_without_hardware() {
    // With or without a mouse attached, --probe must exit cleanly and print an outcome.
    let out = exe().arg("--probe").output().expect("run exe");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        ["Ok(", "DeviceOffline", "NoReceiver", "Error("]
            .iter()
            .any(|k| text.contains(k)),
        "unexpected output: {text}"
    );
}
