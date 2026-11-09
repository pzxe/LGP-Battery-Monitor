# Contributing

Thanks for helping! Bug reports, hardware reports, and pull requests are welcome.

## Hardware reports

The app is verified only on a G Pro X Superlight 2 behind a LIGHTSPEED receiver. If you try
another Logitech mouse (or the USB cable mode), please open an issue using the
"Hardware report" template and include the output of `lgp-battery-monitor.exe --probe`.

## Development setup

1. Install Rust (stable, MSVC): `winget install Rustlang.Rustup`
2. Install Visual Studio Build Tools with the "Desktop development with C++" workload.
3. `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
4. Run with `cargo run -- --show --no-autohide --debug` (debug log: `%APPDATA%\LGP Battery Monitor\debug.log`).

## Pull requests

- Keep changes focused; add or update tests for logic changes (`src/hidpp.rs`, `src/notify.rs`, ...).
- Run `cargo fmt` and `cargo clippy` before pushing.
- Do not copy code from GPL projects (e.g. Solaar); the protocol must be implemented from public documentation.
- Never claim hardware support you have not verified.
