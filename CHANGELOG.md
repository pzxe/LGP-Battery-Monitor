# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [1.0.0] - 2026-11-09

First stable release. Functionally identical to 0.1.0-beta.1; the version number now reflects that the feature set is complete. Verification is unchanged: only a Superlight 2 behind a LIGHTSPEED receiver on Windows 11 has been tested, and builds remain unsigned.

## [0.1.0-beta.1] - 2026-11-09

First public beta.

### Added
- HID++ 2.0 battery reading (Unified Battery `0x1004`, fallback Battery Status `0x1000`) over the LIGHTSPEED receiver or USB cable, with no dependency on Logitech G HUB.
- System tray icon that renders the current percentage, tooltip, and a context menu (Show Battery, Settings, Start with Windows, Check for Updates, About, Exit).
- Compact dark popup with percentage, progress bar, connection state, charging state, and last-updated time.
- Settings: refresh interval, notifications, start with Windows, launch minimized to tray.
- Low (20%) and critical (10%) notifications with de-duplication.
- Single-instance behavior (a second launch opens the popup of the running instance).
- Manual update check against GitHub Releases.
- Portable executable and Inno Setup installer; unsigned (see README, "Code signing").
- Opt-in `--debug` log and `--probe` command-line diagnostic.
