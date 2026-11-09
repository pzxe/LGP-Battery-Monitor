# Security Policy

## Supported versions

Only the latest release receives fixes.

## Reporting a vulnerability

Please do **not** open a public issue for security problems. Use GitHub's
[private vulnerability reporting](https://github.com/pzxe/LGP-Battery-Monitor/security/advisories/new)
for this repository. You should get a first response within a few days.

## What the application does

- Opens only Logitech (VID `0x046D`) HID++ vendor interfaces and sends read-only
  battery queries. It never writes configuration, firmware, or macros to the device.
- Makes network requests **only** when you choose "Check for Updates"
  (HTTPS request to the public GitHub Releases API). No telemetry.
- Stores settings in `%APPDATA%\LGP Battery Monitor\settings.json`.
- Writes only to per-user registry keys (`HKCU`): the optional autostart entry and
  the toast notification display name. It never needs administrator rights.

## Release integrity

Release assets are published with SHA256 checksums. Builds are currently **unsigned**;
see the README section "Code signing".
